//! Busymate AI — kit v2 host core for Tauri (`busymate.bridge/2`, build 2.0.0).
//!
//! A PIPE, like the frozen iOS, Android and JavaScript hosts: the same five
//! operations over the same JSON envelope, answered by YOUR callbacks. It only
//! moves envelopes; every message name, timing, retry and fallback lives in the
//! chat served from busymate.ai. Contract: `CONTRACT-v2.md` in this repository
//! (`https://busymate.ai/sdk/v2/2.0.0/CONTRACT.md`).
//!
//! This module is pure (no Tauri types), so every branch is unit-tested.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::origin::{allowed_origins, normalized_origin};

pub const VERSION: i64 = 2;
pub const BUILD: &str = "2.0.0";
pub const MAX_ENVELOPE: usize = 64 * 1024;
/// The bridge's own mint ceiling; the chat owns every real budget.
pub const MINT_CEILING_MS: u64 = 15_000;
pub const PLATFORM: &str = "tauri";
pub const TRANSPORT: &str = "tauri-ipc";
pub const CAPS: [&str; 8] = [
    "hello",
    "state",
    "mint",
    "open",
    "action",
    "ev:resume",
    "ev:state",
    "ev:restored",
];
pub const ERRORS: [&str; 6] = ["unsupported", "denied", "not_signed_in", "mint_failed", "timeout", "bad_request"];

/// Your application, as `hello` reports it.
#[derive(Debug, Clone, Default)]
pub struct AppInfo {
    pub id: String,
    pub version: Option<String>,
    pub build: Option<String>,
}

/// The configured bridge for one chat webview.
#[derive(Debug, Clone)]
pub struct BridgeCore {
    pub assistant: String,
    pub origins: Vec<String>,
    pub app: AppInfo,
    pub webview: Option<String>,
}

/// What the chat asked, after the envelope and the sender's origin were checked.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Not ours (unparseable, too big, wrong version, no id): no reply at all.
    Ignore,
    /// Answer immediately with this reply envelope.
    Reply { op: String, reply: String },
    /// Call your mint with `request` (the chat's fields plus `assistant` and `origin`).
    Mint { id: Value, request: Map<String, Value> },
    /// Open an https URL outside the app.
    Open { id: Value, url: String, mode: String },
    /// Give a named action to your app.
    Action { id: Value, name: String, params: Map<String, Value> },
}

impl BridgeCore {
    pub fn new(assistant: &str, origins: &[String], app: AppInfo, webview: Option<String>) -> Self {
        Self {
            assistant: assistant.to_string(),
            origins: allowed_origins(Some(assistant), origins),
            app,
            webview,
        }
    }

    pub fn origin_allowed(&self, origin: Option<&str>) -> bool {
        match origin.and_then(normalized_origin) {
            Some(value) => self.origins.contains(&value),
            None => false,
        }
    }

    /// Handle ONE envelope string from the chat. `origin` is the sender's
    /// origin as the host knows it (here: the chat webview's top document).
    pub fn handle(&self, raw: &str, origin: Option<&str>, account: Option<String>) -> Outcome {
        if raw.len() > MAX_ENVELOPE {
            return Outcome::Ignore;
        }
        let envelope: Map<String, Value> = match serde_json::from_str::<Value>(raw) {
            Ok(Value::Object(map)) => map,
            _ => return Outcome::Ignore,
        };
        let version_ok = match envelope.get("bm") {
            Some(Value::Number(n)) => n.as_i64() == Some(VERSION) || n.as_f64() == Some(VERSION as f64),
            _ => false,
        };
        let id = match envelope.get("id") {
            Some(Value::Null) | None => return Outcome::Ignore,
            Some(id) => id.clone(),
        };
        if !version_ok {
            return Outcome::Ignore;
        }
        let op = envelope.get("op").and_then(Value::as_str).unwrap_or("").to_string();
        if !self.origin_allowed(origin) {
            return Outcome::Reply { op, reply: error(&id, "denied") };
        }
        let from = origin.and_then(normalized_origin).unwrap_or_default();
        let p = match envelope.get("p") {
            Some(Value::Object(map)) => map.clone(),
            _ => Map::new(),
        };
        match op.as_str() {
            "hello" => Outcome::Reply { reply: ok(&id, self.hello(&from)), op },
            "state" => Outcome::Reply { reply: ok(&id, state(&self.app.id, account.as_deref())), op },
            "mint" => {
                if account.is_none() {
                    return Outcome::Reply { reply: error(&id, "not_signed_in"), op };
                }
                // Who asked is stated by this file, never by the page.
                let mut request = p;
                request.insert("assistant".into(), Value::String(self.assistant.clone()));
                request.insert("origin".into(), Value::String(from));
                Outcome::Mint { id, request }
            }
            "open" => {
                let url = p.get("url").and_then(Value::as_str).unwrap_or("");
                if !is_https_url(url) {
                    return Outcome::Reply { reply: error(&id, "bad_request"), op };
                }
                let mode = if p.get("mode").and_then(Value::as_str) == Some("auth") { "auth" } else { "external" };
                Outcome::Open { id, url: url.to_string(), mode: mode.to_string() }
            }
            "action" => {
                let name = p.get("name").and_then(Value::as_str).unwrap_or("");
                if name.is_empty() || name.chars().count() > 64 {
                    return Outcome::Reply { reply: error(&id, "bad_request"), op };
                }
                let params = match p.get("params") {
                    Some(Value::Object(map)) => map.clone(),
                    _ => Map::new(),
                };
                Outcome::Action { id, name: name.to_string(), params }
            }
            _ => Outcome::Reply { reply: error(&id, "unsupported"), op },
        }
    }

    /// The `hello` reply body for the asking origin.
    pub fn hello(&self, origin: &str) -> Value {
        json!({
            "bridge": "busymate",
            "v": VERSION,
            "build": BUILD,
            "platform": PLATFORM,
            "transport": TRANSPORT,
            "assistant": self.assistant,
            "origin": origin,
            "origins": self.origins,
            "caps": CAPS,
            "webview": self.webview,
            "app": { "id": self.app.id, "version": self.app.version, "build": self.app.build },
        })
    }
}

/// `{signedIn, account}`; `account` is SHA-256(`appId:accountId`) truncated to
/// 128 bits (hex). The raw id never leaves the device.
pub fn state(app_id: &str, account: Option<&str>) -> Value {
    match account {
        Some(account) => json!({ "signedIn": true, "account": account_key(app_id, account) }),
        None => json!({ "signedIn": false, "account": Value::Null }),
    }
}

pub fn account_key(app_id: &str, account: &str) -> String {
    let digest = Sha256::digest(format!("{app_id}:{account}").as_bytes());
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

fn envelope(id: &Value, mut body: Map<String, Value>) -> String {
    body.insert("bm".into(), json!(VERSION));
    body.insert("id".into(), id.clone());
    Value::Object(body).to_string()
}

/// `{"bm":2,"id":…,"ok":true,"r":{…}}`
pub fn ok(id: &Value, r: Value) -> String {
    let mut body = Map::new();
    body.insert("ok".into(), Value::Bool(true));
    body.insert("r".into(), r);
    envelope(id, body)
}

/// `{"bm":2,"id":…,"ok":false,"e":"…"}`
pub fn error(id: &Value, e: &str) -> String {
    debug_assert!(ERRORS.contains(&e));
    let mut body = Map::new();
    body.insert("ok".into(), Value::Bool(false));
    body.insert("e".into(), Value::String(e.to_string()));
    envelope(id, body)
}

/// An event envelope: `{"bm":2,"ev":"…","r":{…}}`.
pub fn event(ev: &str, r: Value) -> String {
    json!({ "bm": VERSION, "ev": ev, "r": r }).to_string()
}

/// The page-side event delivery the host evaluates in the chat document.
pub fn deliver_script(envelope: &str) -> String {
    format!(
        "window.dispatchEvent(new CustomEvent('busymate-bridge',{{detail:{}}}));",
        Value::String(envelope.to_string())
    )
}

/// `https://` with a non-empty host, as the other hosts accept.
pub fn is_https_url(url: &str) -> bool {
    match tauri::Url::parse(url) {
        Ok(parsed) => parsed.scheme() == "https" && parsed.host_str().is_some_and(|h| !h.is_empty()),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> BridgeCore {
        BridgeCore::new(
            "acme",
            &["https://support.acme.com".to_string(), "https://*.acme.com".to_string()],
            AppInfo { id: "com.acme.desktop".into(), version: Some("1.2.3".into()), build: Some("45".into()) },
            Some("WebView2/140".into()),
        )
    }

    const OWN: Option<&str> = Some("https://acme.busymate.ai");

    fn parse(reply: &str) -> Value {
        serde_json::from_str(reply).unwrap()
    }

    fn reply_of(outcome: Outcome) -> Value {
        match outcome {
            Outcome::Reply { reply, .. } => parse(&reply),
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn ignores_what_is_not_an_envelope() {
        let c = core();
        for raw in ["", "nope", "[]", "{}", r#"{"bm":1,"id":"a","op":"hello"}"#, r#"{"bm":2,"op":"hello"}"#, r#"{"bm":2,"id":null,"op":"hello"}"#, r#"{"bm":"2","id":"a"}"#] {
            assert_eq!(c.handle(raw, OWN, None), Outcome::Ignore, "{raw}");
        }
        let big = format!(r#"{{"bm":2,"id":"a","op":"hello","p":{{"x":"{}"}}}}"#, "x".repeat(MAX_ENVELOPE));
        assert_eq!(c.handle(&big, OWN, None), Outcome::Ignore);
    }

    #[test]
    fn hello_reports_the_exact_allow_list() {
        let r = reply_of(core().handle(r#"{"bm":2,"id":"h1","op":"hello"}"#, Some("https://ACME.busymate.ai"), None));
        assert_eq!(r["bm"], 2);
        assert_eq!(r["id"], "h1");
        assert_eq!(r["ok"], true);
        let hello = &r["r"];
        assert_eq!(hello["bridge"], "busymate");
        assert_eq!(hello["v"], 2);
        assert_eq!(hello["build"], "2.0.0");
        assert_eq!(hello["platform"], "tauri");
        assert_eq!(hello["assistant"], "acme");
        assert_eq!(hello["origin"], "https://acme.busymate.ai");
        assert_eq!(hello["origins"], json!(["https://acme.busymate.ai", "https://support.acme.com"]));
        assert_eq!(hello["caps"], json!(CAPS));
        assert_eq!(hello["webview"], "WebView2/140");
        assert_eq!(hello["app"], json!({"id": "com.acme.desktop", "version": "1.2.3", "build": "45"}));
    }

    #[test]
    fn foreign_origins_are_denied_and_never_reach_mint() {
        let c = core();
        for origin in [
            Some("https://other.busymate.ai"),
            Some("https://busymate.ai"),
            Some("https://evil.acme.com"),
            Some("http://acme.busymate.ai"),
            Some("null"),
            None,
        ] {
            let r = reply_of(c.handle(r#"{"bm":2,"id":7,"op":"mint","p":{"nonce":"n"}}"#, origin, Some("u1".into())));
            assert_eq!(r, json!({"bm": 2, "id": 7, "ok": false, "e": "denied"}), "{origin:?}");
        }
    }

    #[test]
    fn state_hashes_the_account_and_never_sends_it() {
        let c = core();
        let signed_out = reply_of(c.handle(r#"{"bm":2,"id":"s","op":"state"}"#, OWN, None));
        assert_eq!(signed_out["r"], json!({"signedIn": false, "account": null}));
        let signed_in = reply_of(c.handle(r#"{"bm":2,"id":"s","op":"state"}"#, OWN, Some("user-42".into())));
        let key = signed_in["r"]["account"].as_str().unwrap();
        assert_eq!(key.len(), 32);
        assert!(!signed_in.to_string().contains("user-42"));
        // SHA-256("com.acme.desktop:user-42"), first 16 bytes; same formula as the JS core.
        assert_eq!(key, account_key("com.acme.desktop", "user-42"));
        assert_eq!(account_key("a", "b"), "6783a31eabf68ccc0660f935c0826282");
    }

    #[test]
    fn mint_is_bound_by_the_bridge_not_the_page() {
        let c = core();
        let signed_out = reply_of(c.handle(r#"{"bm":2,"id":"m","op":"mint","p":{"nonce":"n"}}"#, OWN, None));
        assert_eq!(signed_out["e"], "not_signed_in");
        let raw = r#"{"bm":2,"id":"m","op":"mint","p":{"nonce":"n1","aud":"busymate-ai","reason":"launch","assistant":"evil","origin":"https://evil.com"}}"#;
        match c.handle(raw, Some("https://support.acme.com:443"), Some("u".into())) {
            Outcome::Mint { id, request } => {
                assert_eq!(id, json!("m"));
                assert_eq!(request["nonce"], "n1");
                assert_eq!(request["aud"], "busymate-ai");
                assert_eq!(request["reason"], "launch");
                assert_eq!(request["assistant"], "acme");
                assert_eq!(request["origin"], "https://support.acme.com");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn open_accepts_https_only() {
        let c = core();
        match c.handle(r#"{"bm":2,"id":1,"op":"open","p":{"url":"https://acme.com/help","mode":"auth"}}"#, OWN, None) {
            Outcome::Open { url, mode, .. } => {
                assert_eq!(url, "https://acme.com/help");
                assert_eq!(mode, "auth");
            }
            other => panic!("{other:?}"),
        }
        match c.handle(r#"{"bm":2,"id":1,"op":"open","p":{"url":"https://acme.com","mode":"weird"}}"#, OWN, None) {
            Outcome::Open { mode, .. } => assert_eq!(mode, "external"),
            other => panic!("{other:?}"),
        }
        for url in ["http://acme.com", "file:///etc/passwd", "javascript:alert(1)", "https://", ""] {
            let raw = json!({"bm": 2, "id": 1, "op": "open", "p": {"url": url}}).to_string();
            assert_eq!(reply_of(c.handle(&raw, OWN, None))["e"], "bad_request", "{url}");
        }
    }

    #[test]
    fn action_names_are_bounded() {
        let c = core();
        match c.handle(r#"{"bm":2,"id":1,"op":"action","p":{"name":"close","params":{"a":1}}}"#, OWN, None) {
            Outcome::Action { name, params, .. } => {
                assert_eq!(name, "close");
                assert_eq!(params["a"], 1);
            }
            other => panic!("{other:?}"),
        }
        let long = json!({"bm": 2, "id": 1, "op": "action", "p": {"name": "x".repeat(65)}}).to_string();
        assert_eq!(reply_of(c.handle(&long, OWN, None))["e"], "bad_request");
        assert_eq!(reply_of(c.handle(r#"{"bm":2,"id":1,"op":"action","p":{}}"#, OWN, None))["e"], "bad_request");
    }

    #[test]
    fn unknown_ops_are_unsupported_and_unknown_fields_ignored() {
        let r = reply_of(core().handle(r#"{"bm":2,"id":"u","op":"teleport","future":true}"#, OWN, None));
        assert_eq!(r, json!({"bm": 2, "id": "u", "ok": false, "e": "unsupported"}));
    }

    #[test]
    fn events_and_delivery_script() {
        assert_eq!(serde_json::from_str::<Value>(&event("resume", json!({}))).unwrap(), json!({"bm": 2, "ev": "resume", "r": {}}));
        let script = deliver_script(&event("restored", json!({})));
        assert!(script.starts_with("window.dispatchEvent(new CustomEvent('busymate-bridge',{detail:\""));
        assert!(script.contains(r#"\"ev\":\"restored\""#));
    }
}
