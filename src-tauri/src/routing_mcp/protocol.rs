use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionPhase {
    New,
    AwaitingInitialized,
    Ready,
}

pub(crate) struct Request {
    pub(crate) id: Option<Value>,
    pub(crate) method: String,
    pub(crate) params: Option<Value>,
}

pub(crate) struct ProtocolError {
    pub(crate) id: Value,
    pub(crate) code: i32,
    pub(crate) message: &'static str,
    pub(crate) diagnostic_code: &'static str,
}

pub(crate) fn request(line: &str) -> Result<Request, ProtocolError> {
    let value: Value = serde_json::from_str(line).map_err(|_| ProtocolError {
        id: Value::Null,
        code: -32700,
        message: "Parse error",
        diagnostic_code: "parse_error",
    })?;
    let Some(object) = value.as_object() else {
        return Err(invalid_request());
    };
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "jsonrpc" | "id" | "method" | "params"))
        || object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
    {
        return Err(invalid_request());
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Err(invalid_request());
    };
    if object
        .get("params")
        .is_some_and(|params| !params.is_object())
    {
        return Err(invalid_request());
    }
    let id = object.get("id").cloned();
    if id
        .as_ref()
        .is_some_and(|id| !(id.is_string() || id.as_i64().is_some() || id.as_u64().is_some()))
    {
        return Err(invalid_request());
    }
    match method {
        "initialize" | "tools/list" | "tools/call" if id.is_none() => {
            return Err(invalid_request());
        }
        "notifications/initialized" if id.is_some() => {
            return Err(invalid_request());
        }
        _ => {}
    }
    Ok(Request {
        id,
        method: method.to_owned(),
        params: object.get("params").cloned(),
    })
}

pub(crate) fn success(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

pub(crate) fn error(id: Value, code: i32, message: &'static str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

pub(crate) fn contains_forbidden_field(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            matches!(
                key.to_ascii_lowercase().as_str(),
                "prompt"
                    | "task"
                    | "response"
                    | "reasoning"
                    | "tool_arguments"
                    | "tool_output"
                    | "patch"
                    | "command"
                    | "cwd"
                    | "file_path"
                    | "auth"
                    | "cookie"
                    | "secret"
            ) || contains_forbidden_field(value)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_field),
        _ => false,
    }
}

pub(crate) fn valid_params(method: &str, params: Option<&Value>) -> bool {
    match method {
        "initialize" => valid_initialize(params),
        "notifications/initialized" | "tools/list" => {
            params.is_none_or(|value| value.as_object().is_some_and(|object| object.is_empty()))
        }
        "tools/call" => params.and_then(Value::as_object).is_some_and(|object| {
            exact_keys(object, &["name", "arguments"])
                && object.get("name").is_some_and(Value::is_string)
                && object.get("arguments").is_some_and(Value::is_object)
        }),
        _ => true,
    }
}

fn valid_initialize(params: Option<&Value>) -> bool {
    let Some(params) = params.and_then(Value::as_object) else {
        return false;
    };
    if !exact_keys(params, &["protocolVersion", "capabilities", "clientInfo"])
        || !params.get("protocolVersion").is_some_and(Value::is_string)
    {
        return false;
    }
    let Some(capabilities) = params.get("capabilities").and_then(Value::as_object) else {
        return false;
    };
    if !valid_capabilities(capabilities) {
        return false;
    }
    let Some(client) = params.get("clientInfo").and_then(Value::as_object) else {
        return false;
    };
    client.keys().all(|key| {
        matches!(
            key.as_str(),
            "name" | "title" | "version" | "description" | "icons" | "websiteUrl"
        )
    }) && client.get("name").is_some_and(Value::is_string)
        && client.get("version").is_some_and(Value::is_string)
        && ["title", "description", "websiteUrl"]
            .iter()
            .all(|key| client.get(*key).is_none_or(Value::is_string))
        && client.get("icons").is_none_or(valid_icons)
}

fn valid_capabilities(capabilities: &serde_json::Map<String, Value>) -> bool {
    capabilities
        .iter()
        .all(|(name, value)| match name.as_str() {
            "experimental" => value.as_object().is_some_and(|features| {
                features
                    .values()
                    .all(|feature| feature.as_object().is_some_and(serde_json::Map::is_empty))
            }),
            "roots" => value.as_object().is_some_and(|roots| {
                roots.keys().all(|key| key == "listChanged")
                    && roots.get("listChanged").is_none_or(Value::is_boolean)
            }),
            "sampling" => value.as_object().is_some_and(|sampling| {
                sampling
                    .keys()
                    .all(|key| matches!(key.as_str(), "context" | "tools"))
                    && sampling.values().all(empty_object)
            }),
            "elicitation" => value.as_object().is_some_and(|elicitation| {
                elicitation
                    .keys()
                    .all(|key| matches!(key.as_str(), "form" | "url"))
                    && elicitation.values().all(empty_object)
            }),
            "tasks" => valid_task_capability(value),
            _ => false,
        })
}

fn valid_task_capability(value: &Value) -> bool {
    let Some(tasks) = value.as_object() else {
        return false;
    };
    if !tasks
        .keys()
        .all(|key| matches!(key.as_str(), "list" | "cancel" | "requests"))
        || ["list", "cancel"]
            .iter()
            .any(|key| tasks.get(*key).is_some_and(|value| !empty_object(value)))
    {
        return false;
    }
    tasks.get("requests").is_none_or(|requests| {
        requests.as_object().is_some_and(|requests| {
            requests
                .keys()
                .all(|key| matches!(key.as_str(), "sampling" | "elicitation"))
                && requests.iter().all(|(name, value)| {
                    let expected = if name == "sampling" {
                        "createMessage"
                    } else {
                        "create"
                    };
                    value.as_object().is_some_and(|request| {
                        request.keys().all(|key| key == expected)
                            && request.get(expected).is_none_or(empty_object)
                    })
                })
        })
    })
}

fn empty_object(value: &Value) -> bool {
    value.as_object().is_some_and(serde_json::Map::is_empty)
}

fn valid_icons(value: &Value) -> bool {
    value.as_array().is_some_and(|icons| {
        icons.iter().all(|icon| {
            let Some(icon) = icon.as_object() else {
                return false;
            };
            icon.keys()
                .all(|key| matches!(key.as_str(), "src" | "mimeType" | "sizes" | "theme"))
                && icon.get("src").is_some_and(Value::is_string)
                && icon.get("mimeType").is_none_or(Value::is_string)
                && icon.get("theme").is_none_or(Value::is_string)
                && icon.get("sizes").is_none_or(|sizes| {
                    sizes
                        .as_array()
                        .is_some_and(|sizes| sizes.iter().all(Value::is_string))
                })
        })
    })
}

fn exact_keys(object: &serde_json::Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && object.keys().all(|key| expected.contains(&key.as_str()))
}

fn invalid_request() -> ProtocolError {
    ProtocolError {
        id: Value::Null,
        code: -32600,
        message: "Invalid Request",
        diagnostic_code: "invalid_request",
    }
}
