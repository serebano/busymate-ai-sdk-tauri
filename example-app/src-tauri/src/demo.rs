//! The example's identity settings and its call to YOUR authenticated backend.
//! There is no backend, login shortcut, signing key or fabricated identity here:
//! identified mode needs your own HTTPS mint endpoint.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Url;
use tauri_plugin_busymate_ai::{MintError, MintRequest, MintToken};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct DemoIdentity {
    pub signed_in: bool,
    pub account_id: String,
    pub mint_url: String,
    /// Your existing tenant backend session credential, memory only.
    pub session_token: String,
    pub handle_close_action: bool,
}

pub fn secure_url(raw: &str) -> Option<Url> {
    let url = Url::parse(raw.trim()).ok()?;
    (url.scheme() == "https" && url.host_str().is_some() && url.username().is_empty() && url.password().is_none()).then_some(url)
}

impl DemoIdentity {
    /// The account id the SDK's `account` callback returns, or `None` (signed out).
    pub fn account(&self) -> Option<String> {
        let id = self.account_id.trim();
        (self.signed_in && !id.is_empty()).then(|| id.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.session_token.contains('\n') || self.session_token.contains('\r') {
            return Err("The backend session token cannot contain line breaks.".into());
        }
        if self.signed_in {
            if self.account_id.trim().is_empty() {
                return Err("Identified mode requires your actual signed-in account ID. This is not a login.".into());
            }
            if secure_url(&self.mint_url).is_none() {
                return Err("Identified mode requires your authenticated HTTPS mint endpoint.".into());
            }
        } else if !self.mint_url.trim().is_empty() && secure_url(&self.mint_url).is_none() {
            return Err("The mint endpoint must be an HTTPS URL.".into());
        }
        Ok(())
    }
}

/// The JSON the example POSTs: the chat's nonce, optional aud and reason, plus
/// the bridge-supplied assistant and origin your backend may bind the token to.
pub fn mint_body(request: &MintRequest) -> Value {
    let mut body = json!({ "nonce": request.nonce, "assistant": request.assistant, "origin": request.origin });
    if let Some(aud) = &request.aud {
        body["aud"] = json!(aud);
    }
    if let Some(reason) = &request.reason {
        body["reason"] = json!(reason);
    }
    body
}

/// 401/403 = signed out. Anything else that is not a fresh, correlated
/// `{"token":"…","nonce":"<same nonce>"}` fails closed.
pub fn decode(status: u16, body: &[u8], nonce: &str) -> Result<Option<MintToken>, MintError> {
    if status == 401 || status == 403 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        return Err(MintError::Failed(format!("backend status {status}")));
    }
    let value: Value = serde_json::from_slice(body).map_err(|_| MintError::Failed("not JSON".into()))?;
    let token = value.get("token").and_then(Value::as_str).filter(|t| !t.is_empty());
    let reply_nonce = value.get("nonce").and_then(Value::as_str);
    match (token, reply_nonce) {
        (Some(token), Some(reply_nonce)) if reply_nonce == nonce => {
            Ok(Some(MintToken { token: token.to_string(), nonce: reply_nonce.to_string() }))
        }
        _ => Err(MintError::Failed("uncorrelated reply".into())),
    }
}

/// A client that never follows a redirect, so authentication never travels to
/// another endpoint or scheme.
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(12))
        .build()
        .expect("http client")
}

/// POST the mint request to `url` (validated as https by the caller).
pub async fn post(client: &reqwest::Client, url: Url, session_token: &str, request: &MintRequest) -> Result<Option<MintToken>, MintError> {
    let mut call = client
        .post(url)
        .header("Cache-Control", "no-store")
        .json(&mint_body(request));
    if !session_token.is_empty() {
        call = call.bearer_auth(session_token);
    }
    let response = call.send().await.map_err(|_| MintError::Failed("backend unreachable".into()))?;
    let status = response.status().as_u16();
    let body = response.bytes().await.map_err(|_| MintError::Failed("backend body".into()))?;
    decode(status, &body, &request.nonce)
}

/// The example's implementation of the SDK `mint` callback.
pub async fn mint(identity: DemoIdentity, request: MintRequest) -> Result<Option<MintToken>, MintError> {
    if identity.account().is_none() {
        return Ok(None);
    }
    let url = secure_url(&identity.mint_url).ok_or_else(|| MintError::Failed("no https mint endpoint".into()))?;
    post(&client(), url, &identity.session_token, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn request(nonce: &str) -> MintRequest {
        MintRequest {
            nonce: nonce.into(),
            aud: Some("busymate-ai".into()),
            reason: Some("launch".into()),
            assistant: "busyproxy".into(),
            origin: "https://busymate.ai".into(),
            raw: Map::new(),
        }
    }

    #[test]
    fn guest_defaults_are_valid_and_signed_out() {
        let identity = DemoIdentity::default();
        assert_eq!(identity.validate(), Ok(()));
        assert_eq!(identity.account(), None);
    }

    #[test]
    fn identified_mode_needs_an_account_and_an_https_endpoint() {
        let mut identity = DemoIdentity { signed_in: true, ..Default::default() };
        assert!(identity.validate().is_err());
        identity.account_id = "user-42".into();
        assert!(identity.validate().is_err());
        identity.mint_url = "http://api.example.com/mint".into();
        assert!(identity.validate().is_err());
        identity.mint_url = "https://user:pw@api.example.com/mint".into();
        assert!(identity.validate().is_err());
        identity.mint_url = "https://api.example.com/mint".into();
        assert_eq!(identity.validate(), Ok(()));
        assert_eq!(identity.account().as_deref(), Some("user-42"));
        identity.session_token = "a\nb".into();
        assert!(identity.validate().is_err());
    }

    #[test]
    fn body_carries_bridge_bound_fields() {
        assert_eq!(
            mint_body(&request("n1")),
            json!({"nonce": "n1", "aud": "busymate-ai", "reason": "launch", "assistant": "busyproxy", "origin": "https://busymate.ai"})
        );
    }

    #[test]
    fn decode_fails_closed() {
        assert!(matches!(decode(401, b"", "n"), Ok(None)));
        assert!(matches!(decode(403, b"{}", "n"), Ok(None)));
        assert!(matches!(decode(500, b"{}", "n"), Err(MintError::Failed(_))));
        assert!(matches!(decode(200, b"nope", "n"), Err(MintError::Failed(_))));
        assert!(decode(200, br#"{"token":"","nonce":"n"}"#, "n").is_err());
        assert!(decode(200, br#"{"token":"t","nonce":"other"}"#, "n").is_err());
        let ok = decode(200, br#"{"token":"t","nonce":"n"}"#, "n").unwrap().unwrap();
        assert_eq!((ok.token.as_str(), ok.nonce.as_str()), ("t", "n"));
    }

    #[tokio::test]
    async fn signed_out_makes_no_request() {
        let result = mint(DemoIdentity { mint_url: "https://127.0.0.1:9/never".into(), ..Default::default() }, request("n")).await;
        assert!(matches!(result, Ok(None)));
    }

    /// One loopback server answers 302 to a second; the redirect is refused and
    /// the second server is never reached.
    #[tokio::test]
    async fn redirects_are_refused() {
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let target_hits = hits.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = target.accept().await {
                target_hits.fetch_add(1, Ordering::SeqCst);
                let _ = socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 26\r\n\r\n{\"token\":\"t\",\"nonce\":\"n1\"}").await;
            }
        });
        let redirector = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let redirector_addr = redirector.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = redirector.accept().await {
                let mut buffer = [0u8; 4096];
                let _ = socket.read(&mut buffer).await;
                let reply = format!("HTTP/1.1 302 Found\r\nlocation: http://{target_addr}/mint\r\ncontent-length: 0\r\n\r\n");
                let _ = socket.write_all(reply.as_bytes()).await;
            }
        });
        let url = Url::parse(&format!("http://{redirector_addr}/mint")).unwrap();
        let result = post(&client(), url, "session", &request("n1")).await;
        assert!(matches!(result, Err(MintError::Failed(_))), "{result:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
