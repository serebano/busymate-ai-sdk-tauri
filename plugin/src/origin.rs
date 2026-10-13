//! Exact-origin rules shared by the identity bridge, the microphone adapter,
//! navigation and the runtime IPC capability. Same rules as the iOS, Android
//! and JavaScript kit v2 hosts: `https://host[:port]`, lower-case, default port
//! dropped, no wildcards, paths, credentials, queries or fragments.

use tauri::Url;

const MAX_HOST: usize = 253;

fn host_ok(host: &str) -> bool {
    if host.is_empty() || host.len() > MAX_HOST {
        return false;
    }
    let bytes = host.as_bytes();
    let edge = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    edge(bytes[0])
        && edge(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'.' || *b == b'-')
}

/// `"https://host[:port]"`, lower-case, default port dropped; `None` for anything
/// else (wildcards, paths, `http:`, credentials, `"null"`).
pub fn normalized_origin(raw: &str) -> Option<String> {
    let mut value = raw.trim().to_ascii_lowercase();
    if value.ends_with('/') {
        value.pop();
    }
    if value.ends_with(":443") {
        value.truncate(value.len() - 4);
    }
    let rest = value.strip_prefix("https://")?;
    let (host, port) = match rest.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (rest, None),
    };
    if !host_ok(host) {
        return None;
    }
    if let Some(port) = port {
        if port.is_empty() || port.len() > 5 || !port.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    Some(value)
}

/// True for a valid assistant slug (`[a-z0-9]` with inner hyphens, at most 63).
pub fn valid_assistant(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.is_empty() || bytes.len() > 63 {
        return false;
    }
    let edge = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    edge(bytes[0])
        && edge(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Who may ask: EXACTLY `https://<assistant>.busymate.ai` plus the exact https
/// origins you list. Invalid entries are ignored, never widened.
pub fn allowed_origins(assistant: Option<&str>, origins: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(slug) = assistant {
        if valid_assistant(slug) {
            out.push(format!("https://{slug}.busymate.ai"));
        }
    }
    for origin in origins {
        if let Some(value) = normalized_origin(origin) {
            if !out.contains(&value) {
                out.push(value);
            }
        }
    }
    out
}

/// The origin of a document URL, or `None` unless it is https with a host.
/// (`url` serialises the origin without the default port already.)
pub fn origin_of_url(url: &Url) -> Option<String> {
    if url.scheme() != "https" {
        return None;
    }
    url.host_str()?;
    normalized_origin(&url.origin().ascii_serialization())
}

/// Validates an origin list entry strictly, as the settings of an app should:
/// `Err(entry)` for the first entry that is not an exact https origin.
pub fn validate_origins(origins: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in origins {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match normalized_origin(trimmed) {
            Some(value) => {
                if !out.contains(&value) {
                    out.push(value)
                }
            }
            None => return Err(trimmed.to_string()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_exact_https_origins() {
        assert_eq!(normalized_origin("https://Support.Acme.com/").as_deref(), Some("https://support.acme.com"));
        assert_eq!(normalized_origin("https://acme.com:443").as_deref(), Some("https://acme.com"));
        assert_eq!(normalized_origin(" https://acme.com:8443 ").as_deref(), Some("https://acme.com:8443"));
    }

    #[test]
    fn refuses_wildcards_paths_http_and_opaque() {
        for bad in [
            "https://*.busymate.ai",
            "http://acme.com",
            "https://acme.com/path",
            "https://acme.com?x=1",
            "https://acme.com#f",
            "https://user@acme.com",
            "null",
            "",
            "https://",
            "https://-acme.com",
            "https://acme.com:",
            "https://acme.com:123456",
            "https://acme.com:80a",
        ] {
            assert_eq!(normalized_origin(bad), None, "{bad}");
        }
    }

    #[test]
    fn assistant_slug_rules() {
        assert!(valid_assistant("acme"));
        assert!(valid_assistant("busy-proxy2"));
        assert!(!valid_assistant("-acme"));
        assert!(!valid_assistant("acme-"));
        assert!(!valid_assistant("Acme"));
        assert!(!valid_assistant("a.b"));
        assert!(!valid_assistant(&"a".repeat(64)));
    }

    #[test]
    fn allowed_origins_are_exact_and_deduplicated() {
        let extra = vec![
            "https://support.acme.com".to_string(),
            "https://SUPPORT.acme.com/".to_string(),
            "https://*.acme.com".to_string(),
            "https://acme.busymate.ai".to_string(),
        ];
        assert_eq!(
            allowed_origins(Some("acme"), &extra),
            vec!["https://acme.busymate.ai".to_string(), "https://support.acme.com".to_string()]
        );
        assert_eq!(allowed_origins(Some("Not A Slug"), &[]), Vec::<String>::new());
    }

    #[test]
    fn origin_of_document_urls() {
        let url = Url::parse("https://busymate.ai/support/busyproxy?channel=embed&locale=en").unwrap();
        assert_eq!(origin_of_url(&url).as_deref(), Some("https://busymate.ai"));
        let port = Url::parse("https://acme.com:8443/x").unwrap();
        assert_eq!(origin_of_url(&port).as_deref(), Some("https://acme.com:8443"));
        assert_eq!(origin_of_url(&Url::parse("http://busymate.ai/").unwrap()), None);
        assert_eq!(origin_of_url(&Url::parse("tauri://localhost/").unwrap()), None);
        assert_eq!(origin_of_url(&Url::parse("about:blank").unwrap()), None);
    }

    #[test]
    fn strict_validation_reports_the_bad_entry() {
        let ok = validate_origins(&["https://busymate.ai".into(), " ".into()]).unwrap();
        assert_eq!(ok, vec!["https://busymate.ai".to_string()]);
        assert_eq!(validate_origins(&["https://busymate.ai/x".into()]), Err("https://busymate.ai/x".to_string()));
    }
}
