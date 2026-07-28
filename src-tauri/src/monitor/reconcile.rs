use std::collections::{HashMap, HashSet};

use super::model::{
    AgentObservation, AgentStatus, HealthLevel, ModelSource, MonitorSnapshot, ObserverStatus,
    ReconcileInput, SpawnFact, SummaryCounts, ThreadFact,
};

pub fn reconcile(input: ReconcileInput, now_ms: i64) -> MonitorSnapshot {
    let spawns: HashMap<&str, &SpawnFact> = input
        .spawns
        .iter()
        .map(|spawn| (spawn.child_thread_id.as_str(), spawn))
        .collect();
    let parents: HashMap<&str, Option<&str>> = input
        .threads
        .iter()
        .map(|thread| {
            (
                thread.thread_id.as_str(),
                thread.parent_thread_id.as_deref(),
            )
        })
        .collect();

    let mut agents: Vec<AgentObservation> = input
        .threads
        .iter()
        .map(|thread| {
            observe_thread(
                thread,
                spawns.get(thread.thread_id.as_str()).copied(),
                &parents,
                input.codex_running,
                input.session_started_at_ms,
                now_ms,
            )
        })
        .collect();

    agents.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| right.updated_at_ms.cmp(&left.updated_at_ms))
            .then_with(|| left.display_name.cmp(&right.display_name))
    });

    let counts = count_agents(&agents);
    let observer_status = if input.health.state_database.level == HealthLevel::Error
        || input.health.rollout_observer.level == HealthLevel::Error
    {
        ObserverStatus::Error
    } else if counts.uncertain > 0 || counts.tracking_errors > 0 {
        ObserverStatus::Uncertain
    } else if input.health.state_database.level == HealthLevel::Degraded
        || input.health.rollout_observer.level == HealthLevel::Degraded
    {
        ObserverStatus::Delayed
    } else {
        ObserverStatus::Live
    };
    MonitorSnapshot {
        generated_at_ms: now_ms,
        codex_running: input.codex_running,
        session_started_at_ms: input.session_started_at_ms,
        observer_status,
        agents,
        counts,
        health: input.health,
    }
}

fn observe_thread(
    thread: &ThreadFact,
    spawn: Option<&SpawnFact>,
    parents: &HashMap<&str, Option<&str>>,
    codex_running: bool,
    session_started_at_ms: Option<i64>,
    now_ms: i64,
) -> AgentObservation {
    let requested_model = spawn.and_then(|fact| fact.requested_model.clone());
    let (effective_model, model_source) = if let Some(model) = thread.rollout_model.clone() {
        (Some(model), ModelSource::TurnContext)
    } else if let Some(model) = thread.database_model.clone() {
        (Some(model), ModelSource::StateDatabase)
    } else if requested_model.is_some() {
        (None, ModelSource::RequestedOnly)
    } else {
        (None, ModelSource::Unknown)
    };
    let reasoning_effort = thread
        .rollout_effort
        .clone()
        .or_else(|| thread.database_effort.clone())
        .or_else(|| spawn.and_then(|fact| fact.requested_effort.clone()));
    let is_subagent = thread.parent_thread_id.is_some();
    let lifecycle_status = infer_status(thread, is_subagent);
    let updated_at_ms = [
        thread.updated_at_ms,
        thread.latest_task_started_ms,
        thread.latest_task_completed_ms,
        thread.interrupted_at_ms,
        spawn.and_then(|fact| fact.occurred_at_ms),
    ]
    .into_iter()
    .flatten()
    .max();
    let freshness_ms = updated_at_ms.and_then(|updated| {
        (updated >= 0 && updated <= now_ms.saturating_add(5 * 60 * 1_000))
            .then(|| now_ms.saturating_sub(updated).max(0))
    });
    let current_session_evidence = session_started_at_ms
        .zip(updated_at_ms)
        .is_some_and(|(session_start, updated)| updated >= session_start);
    let status = match lifecycle_status {
        AgentStatus::Starting | AgentStatus::Running if !codex_running => {
            AgentStatus::HistoricalUnclosed
        }
        AgentStatus::Starting | AgentStatus::Running if current_session_evidence => {
            lifecycle_status
        }
        AgentStatus::Starting | AgentStatus::Running if session_started_at_ms.is_some() => {
            AgentStatus::HistoricalUnclosed
        }
        AgentStatus::Starting | AgentStatus::Running => AgentStatus::Uncertain,
        other => other,
    };
    let model_drift = requested_model
        .as_deref()
        .zip(effective_model.as_deref())
        .is_some_and(|(requested, effective)| requested != effective);

    AgentObservation {
        thread_id: thread.thread_id.clone(),
        parent_thread_id: thread.parent_thread_id.clone(),
        display_name: display_name(thread),
        role: thread.role.clone(),
        project: thread.project.clone(),
        originator: thread.originator.clone(),
        requested_model,
        effective_model,
        model_source,
        reasoning_effort,
        status,
        model_drift,
        is_subagent,
        depth: depth_for(&thread.thread_id, parents),
        started_at_ms: thread
            .created_at_ms
            .or_else(|| spawn.and_then(|fact| fact.occurred_at_ms)),
        updated_at_ms,
        freshness_ms,
    }
}

fn infer_status(thread: &ThreadFact, is_subagent: bool) -> AgentStatus {
    if thread.tracking_error {
        return AgentStatus::TrackingError;
    }

    let latest_boundary = thread
        .latest_task_started_ms
        .max(thread.latest_task_completed_ms);
    if thread
        .interrupted_at_ms
        .is_some_and(|interrupted| latest_boundary.is_none_or(|boundary| interrupted > boundary))
    {
        return AgentStatus::Interrupted;
    }
    if thread.latest_task_started_ms > thread.latest_task_completed_ms {
        return AgentStatus::Running;
    }
    if thread.latest_task_completed_ms.is_some() {
        return AgentStatus::Idle;
    }
    if is_subagent && thread.rollout_model.is_none() {
        return AgentStatus::Starting;
    }
    AgentStatus::Idle
}

fn display_name(thread: &ThreadFact) -> String {
    let opaque = thread
        .thread_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>();
    let opaque = if opaque.is_empty() {
        "unknown"
    } else {
        &opaque
    };
    if thread.parent_thread_id.is_none() {
        return bounded_label(thread.project.as_deref()).map_or_else(
            || format!("根任务 {opaque}"),
            |project| format!("{project} · {opaque}"),
        );
    }
    bounded_label(thread.nickname.as_deref())
        .or_else(|| bounded_label(thread.role.as_deref()))
        .map_or_else(
            || format!("子代理 {opaque}"),
            |label| format!("{label} · {opaque}"),
        )
}

fn bounded_label(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.chars().take(48).collect())
}

fn depth_for(thread_id: &str, parents: &HashMap<&str, Option<&str>>) -> u32 {
    let mut depth = 0_u32;
    let mut current = thread_id;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        let Some(Some(parent)) = parents.get(current) else {
            break;
        };
        depth = depth.saturating_add(1);
        current = parent;
    }
    depth
}

fn count_agents(agents: &[AgentObservation]) -> SummaryCounts {
    let mut counts = SummaryCounts::default();
    for agent in agents {
        if agent.is_subagent {
            counts.subagents += 1;
        } else {
            counts.roots += 1;
        }
        match agent.status {
            AgentStatus::Starting => counts.starting += 1,
            AgentStatus::Running => counts.running += 1,
            AgentStatus::Uncertain => counts.uncertain += 1,
            AgentStatus::HistoricalUnclosed => counts.historical_unclosed += 1,
            AgentStatus::Idle => counts.idle += 1,
            AgentStatus::Interrupted => counts.interrupted += 1,
            AgentStatus::TrackingError => counts.tracking_errors += 1,
        }
        if agent.model_drift {
            counts.model_drifts += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::model::{
        HealthEntry, HealthLevel, ReconcileInput, SourceHealth, SpawnFact, ThreadFact,
    };

    fn health() -> SourceHealth {
        SourceHealth {
            state_database: HealthEntry::healthy("State database ready", 10_000),
            rollout_observer: HealthEntry::healthy("Rollout observer ready", 10_000),
        }
    }

    fn root_and_child() -> Vec<ThreadFact> {
        vec![
            ThreadFact {
                thread_id: "root".into(),
                nickname: Some("Root task".into()),
                database_model: Some("gpt-5.6-sol".into()),
                ..ThreadFact::default()
            },
            ThreadFact {
                thread_id: "child".into(),
                parent_thread_id: Some("root".into()),
                nickname: Some("Locke".into()),
                database_model: Some("gpt-5.6-sol".into()),
                rollout_model: Some("gpt-5.6-terra".into()),
                rollout_effort: Some("high".into()),
                created_at_ms: Some(1_000),
                updated_at_ms: Some(9_000),
                latest_task_started_ms: Some(8_000),
                ..ThreadFact::default()
            },
        ]
    }

    #[test]
    fn rollout_model_wins_and_drift_is_visible() {
        let snapshot = reconcile(
            ReconcileInput {
                threads: root_and_child(),
                spawns: vec![SpawnFact {
                    child_thread_id: "child".into(),
                    requested_model: Some("gpt-5.6-sol".into()),
                    requested_effort: Some("xhigh".into()),
                    occurred_at_ms: Some(900),
                }],
                health: health(),
                codex_running: true,
                session_started_at_ms: Some(7_000),
            },
            10_000,
        );
        let child = snapshot
            .agents
            .iter()
            .find(|agent| agent.thread_id == "child")
            .expect("child observation");
        assert_eq!(child.requested_model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(child.effective_model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(child.model_source, ModelSource::TurnContext);
        assert_eq!(child.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(child.status, AgentStatus::Running);
        assert_eq!(child.depth, 1);
        assert!(child.model_drift);
        assert_eq!(snapshot.counts.running, 1);
        assert_eq!(snapshot.counts.model_drifts, 1);
    }

    #[test]
    fn lifecycle_prefers_newest_authoritative_boundary() {
        let cases = [
            (Some(100), None, None, AgentStatus::Running),
            (Some(100), Some(200), None, AgentStatus::Idle),
            (Some(300), Some(200), Some(250), AgentStatus::Running),
            (Some(100), Some(200), Some(300), AgentStatus::Interrupted),
        ];
        for (started, completed, interrupted, expected) in cases {
            let thread = ThreadFact {
                thread_id: "child".into(),
                parent_thread_id: Some("root".into()),
                rollout_model: Some("gpt".into()),
                latest_task_started_ms: started,
                latest_task_completed_ms: completed,
                interrupted_at_ms: interrupted,
                ..ThreadFact::default()
            };
            assert_eq!(infer_status(&thread, true), expected);
        }
    }

    #[test]
    fn pending_and_tracking_error_are_explicit() {
        let pending = ThreadFact {
            thread_id: "pending".into(),
            parent_thread_id: Some("root".into()),
            ..ThreadFact::default()
        };
        assert_eq!(infer_status(&pending, true), AgentStatus::Starting);

        let broken = ThreadFact {
            tracking_error: true,
            ..pending
        };
        assert_eq!(infer_status(&broken, true), AgentStatus::TrackingError);
    }

    #[test]
    fn session_boundary_prevents_historical_unclosed_work_from_being_live() {
        let old_unclosed = ThreadFact {
            thread_id: "old".into(),
            latest_task_started_ms: Some(1_000),
            updated_at_ms: Some(1_000),
            ..ThreadFact::default()
        };
        let historical = reconcile(
            ReconcileInput {
                threads: vec![old_unclosed.clone()],
                spawns: vec![],
                health: health(),
                codex_running: true,
                session_started_at_ms: Some(9_000),
            },
            10_000,
        );
        assert_eq!(historical.agents[0].status, AgentStatus::HistoricalUnclosed);
        assert_eq!(historical.counts.running, 0);
        assert_eq!(historical.counts.historical_unclosed, 1);

        let uncertain = reconcile(
            ReconcileInput {
                threads: vec![old_unclosed],
                spawns: vec![],
                health: health(),
                codex_running: true,
                session_started_at_ms: None,
            },
            10_000,
        );
        assert_eq!(uncertain.agents[0].status, AgentStatus::Uncertain);
        assert_eq!(uncertain.counts.uncertain, 1);
    }

    #[test]
    fn current_session_activity_restores_running_and_future_time_fails_safe() {
        let snapshot = reconcile(
            ReconcileInput {
                threads: vec![ThreadFact {
                    thread_id: "current".into(),
                    latest_task_started_ms: Some(9_500),
                    updated_at_ms: Some(10_000 + 10 * 60 * 1_000),
                    ..ThreadFact::default()
                }],
                spawns: vec![],
                health: health(),
                codex_running: true,
                session_started_at_ms: Some(9_000),
            },
            10_000,
        );
        assert_eq!(snapshot.agents[0].status, AgentStatus::Running);
        assert_eq!(snapshot.agents[0].freshness_ms, None);
    }

    #[test]
    fn nested_depth_is_cycle_safe() {
        let parents = HashMap::from([
            ("root", None),
            ("child", Some("root")),
            ("grandchild", Some("child")),
            ("cycle-a", Some("cycle-b")),
            ("cycle-b", Some("cycle-a")),
        ]);
        assert_eq!(depth_for("grandchild", &parents), 2);
        assert_eq!(depth_for("cycle-a", &parents), 2);
    }

    #[test]
    fn health_fixture_is_healthy() {
        assert_eq!(health().state_database.level, HealthLevel::Healthy);
    }
}
