//! BusymateMicrophone adapter — wire 1, adapter 1.0.0 — for Tauri.
//!
//! The chat sends `{"type":"busymate.microphone.v1.request","id":…,"source":"dictation"|"voice"}`
//! ONLY when the user taps its microphone or voice control. The adapter checks
//! the sender's exact origin, asks the operating system (see `os_permission`)
//! and replies `{"id":…,"granted":bool}`. The webview's own media request is a
//! second decision (`crate::permission_handler`): microphone only, allowed
//! origin only, and only while the OS permission is granted. Pure parsing lives
//! here so it is unit-tested.

use serde_json::{json, Value};

use crate::origin::normalized_origin;

pub const WIRE: u32 = 1;
pub const ADAPTER: &str = "1.0.0";
pub const REQUEST_TYPE: &str = "busymate.microphone.v1.request";
pub const MAX_REQUEST: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrophoneRequest {
    pub id: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The sender is not one of the exact allowed origins.
    Origin,
    /// Too big, not JSON, wrong type or no string id.
    Malformed,
}

/// Validate one request string from the chat document at `origin`.
pub fn parse_request(raw: &str, origin: Option<&str>, allowed: &[String]) -> Result<MicrophoneRequest, Refusal> {
    let origin_ok = origin
        .and_then(normalized_origin)
        .map(|value| allowed.contains(&value))
        .unwrap_or(false);
    if !origin_ok {
        return Err(Refusal::Origin);
    }
    if raw.len() > MAX_REQUEST {
        return Err(Refusal::Malformed);
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| Refusal::Malformed)?;
    if value.get("type").and_then(Value::as_str) != Some(REQUEST_TYPE) {
        return Err(Refusal::Malformed);
    }
    let id = value.get("id").and_then(Value::as_str).ok_or(Refusal::Malformed)?;
    Ok(MicrophoneRequest {
        id: id.to_string(),
        source: value.get("source").and_then(Value::as_str).map(str::to_string),
    })
}

/// `{"id":…,"granted":bool}` — the same id, a boolean answer.
pub fn reply(id: &str, granted: bool) -> String {
    json!({ "id": id, "granted": granted }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> Vec<String> {
        vec!["https://busymate.ai".to_string(), "https://acme.busymate.ai".to_string()]
    }

    #[test]
    fn accepts_a_tap_request_from_an_allowed_origin() {
        let raw = r#"{"type":"busymate.microphone.v1.request","id":"mic-1","source":"voice"}"#;
        assert_eq!(
            parse_request(raw, Some("https://busymate.ai"), &allowed()),
            Ok(MicrophoneRequest { id: "mic-1".into(), source: Some("voice".into()) })
        );
        assert_eq!(parse_request(raw, Some("https://Busymate.ai:443"), &allowed()).unwrap().id, "mic-1");
    }

    #[test]
    fn refuses_other_origins_first() {
        let raw = r#"{"type":"busymate.microphone.v1.request","id":"mic-1","source":"dictation"}"#;
        for origin in [Some("https://evil.com"), Some("https://other.busymate.ai"), Some("http://busymate.ai"), Some("null"), None] {
            assert_eq!(parse_request(raw, origin, &allowed()), Err(Refusal::Origin), "{origin:?}");
        }
    }

    #[test]
    fn refuses_malformed_requests() {
        let o = Some("https://busymate.ai");
        for raw in [
            "",
            "not json",
            r#"{"type":"busymate.microphone.v2.request","id":"x"}"#,
            r#"{"type":"busymate.microphone.v1.request"}"#,
            r#"{"type":"busymate.microphone.v1.request","id":7}"#,
        ] {
            assert_eq!(parse_request(raw, o, &allowed()), Err(Refusal::Malformed), "{raw}");
        }
        let big = format!(r#"{{"type":"busymate.microphone.v1.request","id":"{}"}}"#, "x".repeat(MAX_REQUEST));
        assert_eq!(parse_request(&big, o, &allowed()), Err(Refusal::Malformed));
    }

    #[test]
    fn reply_preserves_the_id() {
        let value: Value = serde_json::from_str(&reply("mic-9", false)).unwrap();
        assert_eq!(value, json!({"id": "mic-9", "granted": false}));
    }
}
