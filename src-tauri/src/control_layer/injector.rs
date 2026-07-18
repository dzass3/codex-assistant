pub const CONTROL_BINDING_NAME: &str = "codexAssistant";
const MAX_INJECTION_SCRIPT_BYTES: usize = 262_144;

use serde_json::{json, Value};

use super::cdp::{CdpClient, CdpClientError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionPrimitive {
    RuntimeEnable,
    PageEnable,
    RuntimeAddBinding,
    PageAddScriptOnNewDocument,
    RuntimeEvaluate,
}

pub const REQUIRED_PRIMITIVES: [InjectionPrimitive; 5] = [
    InjectionPrimitive::RuntimeEnable,
    InjectionPrimitive::PageEnable,
    InjectionPrimitive::RuntimeAddBinding,
    InjectionPrimitive::PageAddScriptOnNewDocument,
    InjectionPrimitive::RuntimeEvaluate,
];

#[derive(Debug, Clone, PartialEq)]
pub struct InjectionCommand {
    pub method: &'static str,
    pub params: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionPlanError {
    InvalidScript,
}

pub fn injection_plan(script: &str) -> Result<Vec<InjectionCommand>, InjectionPlanError> {
    if script.is_empty()
        || script.len() > MAX_INJECTION_SCRIPT_BYTES
        || script.as_bytes().contains(&0)
    {
        return Err(InjectionPlanError::InvalidScript);
    }
    Ok(vec![
        InjectionCommand {
            method: "Runtime.enable",
            params: json!({}),
        },
        InjectionCommand {
            method: "Page.enable",
            params: json!({}),
        },
        InjectionCommand {
            method: "Runtime.addBinding",
            params: json!({ "name": CONTROL_BINDING_NAME }),
        },
        InjectionCommand {
            method: "Page.addScriptToEvaluateOnNewDocument",
            params: json!({ "source": script }),
        },
        InjectionCommand {
            method: "Runtime.evaluate",
            params: json!({
                "expression": script,
                "awaitPromise": false,
                "returnByValue": false,
            }),
        },
    ])
}

pub async fn apply_injection(
    client: &mut CdpClient,
    script: &str,
) -> Result<(), ApplyInjectionError> {
    let plan = injection_plan(script).map_err(|_| ApplyInjectionError::InvalidScript)?;
    for command in plan {
        client
            .call(command.method, command.params)
            .await
            .map_err(ApplyInjectionError::Cdp)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyInjectionError {
    InvalidScript,
    Cdp(CdpClientError),
}
