//! Local-only AI host policy
//!
//! `OLLAMA_HOST` and `ANTHROPIC_COMPATIBLE_HOST` accept any URL. "No cloud APIs"
//! is a product rule; this module makes it a code gate.
//!
//! Local (always allowed):
//! - loopback IPs (`127.0.0.0/8`, `::1`)
//! - RFC1918 IPv4 (`10/8`, `172.16/12`, `192.168/16`)
//! - IPv6 unique-local (`fc00::/7`)
//! - `localhost`
//! - `*.local` (mDNS)
//! - single-label names (Docker Compose `ollama`, LAN short names)
//! - `*.docker.internal` (Docker Desktop)
//!
//! Anything else is refused unless `HONE_ALLOW_REMOTE_AI` is `1`/`true`/`yes`.
//!
//! A name that passes the string check is still resolved. Every address must
//! be loopback, RFC1918, or IPv6 unique-local. AI HTTP clients do not follow
//! redirects, so a 307 cannot move the prompt onto a host this gate never saw.

use std::net::{IpAddr, ToSocketAddrs};

use crate::error::{Error, Result};

/// Env vars that point at an AI HTTP endpoint.
pub const AI_HOST_ENV_VARS: &[&str] = &["OLLAMA_HOST", "ANTHROPIC_COMPATIBLE_HOST"];

/// Opt-in that allows a non-local AI host. Any remote URL is then accepted.
pub const ALLOW_REMOTE_AI_ENV: &str = "HONE_ALLOW_REMOTE_AI";

/// Whether `HONE_ALLOW_REMOTE_AI` is an explicit opt-in.
pub fn remote_ai_is_allowed() -> bool {
    remote_ai_opt_in_value(std::env::var(ALLOW_REMOTE_AI_ENV).ok().as_deref())
}

/// Parse an opt-in flag (`1`, `true`, `yes`; case-insensitive).
fn remote_ai_opt_in_value(value: Option<&str>) -> bool {
    matches!(
        value
            .map(str::trim)
            .map(|s| s.to_ascii_lowercase())
            .as_deref(),
        Some("1" | "true" | "yes")
    )
}

/// True when `url` resolves to a local / LAN / Docker-internal host.
pub fn is_local_ai_host(url: &str) -> bool {
    match hostname_from_url(url) {
        Some(host) => is_local_hostname(&host),
        None => false,
    }
}

/// Refuse a public AI host unless the caller already opted in.
///
/// When the hostname string passes [`is_local_ai_host`], each resolved address
/// must be loopback, RFC1918, or IPv6 unique-local. The remote opt-in covers
/// hostnames that fail that string check.
pub fn ensure_ai_host_allowed(url: &str) -> Result<()> {
    ensure_ai_host_allowed_with(url, remote_ai_is_allowed())
}

/// Same as [`ensure_ai_host_allowed`] with an explicit opt-in flag (for tests).
pub fn ensure_ai_host_allowed_with(url: &str, allow_remote: bool) -> Result<()> {
    ensure_named_ai_host(url, "AI host", allow_remote, &mut resolve_host_ips)
}

/// Refuse configured `OLLAMA_HOST` / `ANTHROPIC_COMPATIBLE_HOST` when they are
/// public and `HONE_ALLOW_REMOTE_AI` is not set.
pub fn ensure_configured_ai_hosts() -> Result<()> {
    let allow_remote = remote_ai_is_allowed();
    for var in AI_HOST_ENV_VARS {
        let Ok(url) = std::env::var(var) else {
            continue;
        };
        let url = url.trim();
        if url.is_empty() {
            continue;
        }
        ensure_named_ai_host(url, var, allow_remote, &mut resolve_host_ips)?;
    }
    Ok(())
}

fn ensure_named_ai_host(
    url: &str,
    name: &str,
    allow_remote: bool,
    resolve: &mut dyn FnMut(&str) -> std::io::Result<Vec<IpAddr>>,
) -> Result<()> {
    if is_local_ai_host(url) {
        // The string check already passed. Confirm the addresses we would
        // actually dial, including when the remote opt-in is set.
        return ensure_resolved_addresses_local(name, url, resolve);
    }
    if allow_remote {
        return Ok(());
    }
    Err(remote_ai_host_error(name, url))
}

/// Addresses a local-looking AI hostname is allowed to dial.
fn ensure_resolved_addresses_local(
    name: &str,
    url: &str,
    resolve: &mut dyn FnMut(&str) -> std::io::Result<Vec<IpAddr>>,
) -> Result<()> {
    let Some(host) = hostname_from_url(url) else {
        return Err(Error::InvalidData(format!(
            "{name} '{url}' has no hostname to resolve"
        )));
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();

    // Literals were classified by `is_local_ai_host`. Re-check the parsed
    // address so a literal does not depend on the resolver.
    let ips = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![ip]
    } else {
        resolve(&host).map_err(|err| {
            Error::InvalidData(format!(
                "{name} '{url}' looks local but could not be resolved to a loopback, \
                 RFC1918, or IPv6 unique-local address: {err}"
            ))
        })?
    };

    if ips.is_empty() {
        return Err(Error::InvalidData(format!(
            "{name} '{url}' looks local but resolved to no addresses"
        )));
    }

    // Any public address is enough to refuse: the HTTP client may dial any
    // record it got back.
    if let Some(ip) = ips.iter().find(|ip| !is_local_ip(**ip)) {
        return Err(resolved_address_error(name, url, *ip));
    }
    Ok(())
}

fn resolved_address_error(name: &str, url: &str, ip: IpAddr) -> Error {
    Error::InvalidData(format!(
        "{name} '{url}' resolves to {ip}, which is not loopback, RFC1918, or IPv6 \
         unique-local. Hone refuses to send financial data to that address. To use a \
         public endpoint, set the host to that URL and {ALLOW_REMOTE_AI_ENV}=1."
    ))
}

fn resolve_host_ips(host: &str) -> std::io::Result<Vec<IpAddr>> {
    let mut ips = Vec::new();
    for addr in (host, 0u16).to_socket_addrs()? {
        let ip = addr.ip();
        if !ips.contains(&ip) {
            ips.push(ip);
        }
    }
    Ok(ips)
}

/// HTTP client for Ollama, Anthropic-compatible, and OpenAI-compatible calls.
///
/// Redirects are disabled. A local endpoint can answer `307` to a public host,
/// and `HONE_ALLOW_REMOTE_AI` is applied only to the configured URL.
pub(crate) fn ai_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to build AI HTTP client")
}

fn remote_ai_host_error(name: &str, url: &str) -> Error {
    Error::InvalidData(format!(
        "{name} '{url}' is not a local AI host. Hone refuses public AI endpoints \
         unless {ALLOW_REMOTE_AI_ENV}=1. Local hosts: loopback, RFC1918, localhost, \
         *.local, single-label names (e.g. ollama), and *.docker.internal."
    ))
}

/// Extract the hostname from a URL or host:port string.
fn hostname_from_url(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let rest = rest.rsplit_once('@').map(|(_, host)| host).unwrap_or(rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if authority.is_empty() {
        return None;
    }

    let host = if let Some(inner) = authority.strip_prefix('[') {
        inner.split(']').next()?.to_string()
    } else if let Some((h, port)) = authority.rsplit_once(':') {
        if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            h.to_string()
        } else {
            authority.to_string()
        }
    } else {
        authority.to_string()
    };

    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn is_local_hostname(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    // Parse IPs first so `[::]` / `0.0.0.0` are not treated as single-label names.
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_local_ip(ip);
    }
    if host == "localhost" {
        return true;
    }
    if host.ends_with(".local") && host.len() > ".local".len() {
        return true;
    }
    if host.ends_with(".docker.internal") {
        return true;
    }
    // Docker Compose / short LAN names (`ollama`, `mac`, `ollama_gpu`).
    // Not all-numeric, hex IPv4 (`0x8080808`), or colon remnants from
    // unbracketed IPv6 — those fail IpAddr parse but WHATWG/reqwest
    // still treat them as IPs.
    is_single_label_local_name(&host)
}

/// Short LAN / Compose name that is safe to treat as local.
///
/// Underscores are allowed (Compose service names). Decimal IPv4
/// (`134744072`), hex IPv4 (`0x8080808`), and colon remnants are not.
fn is_single_label_local_name(host: &str) -> bool {
    if host.contains(':') || looks_like_integer_ipv4(host) {
        return false;
    }
    is_local_short_label(host)
}

fn is_local_short_label(host: &str) -> bool {
    let bytes = host.as_bytes();
    if bytes.is_empty() || bytes.len() > 63 {
        return false;
    }
    let first = bytes[0];
    let last = bytes[bytes.len() - 1];
    if !first.is_ascii_alphanumeric() || !last.is_ascii_alphanumeric() {
        return false;
    }
    // DNS letters/digits/hyphen plus `_` for Compose service names.
    bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_')
}

fn looks_like_integer_ipv4(host: &str) -> bool {
    if host.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    host.strip_prefix("0x")
        .or_else(|| host.strip_prefix("0X"))
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn is_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private(),
        IpAddr::V6(ip) => {
            // fc00::/7. Ipv6Addr::is_unique_local() needs rustc 1.84+.
            ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_loopback_and_localhost_accepted() {
        for url in [
            "http://localhost:11434",
            "http://LOCALHOST:11434",
            "http://127.0.0.1:11434",
            "http://127.0.0.53",
            "http://[::1]:11434",
            "localhost",
        ] {
            assert!(is_local_ai_host(url), "expected local: {url}");
            ensure_ai_host_allowed_with(url, false).unwrap_or_else(|e| {
                panic!("local host should be accepted without opt-in ({url}): {e}")
            });
        }
    }

    #[test]
    fn rfc1918_and_lan_names_accepted() {
        for url in [
            "http://192.168.1.100:11434",
            "http://10.0.0.5:11434",
            "http://172.16.0.8:11434",
            "http://172.31.255.1",
        ] {
            assert!(is_local_ai_host(url), "expected local: {url}");
            ensure_ai_host_allowed_with(url, false)
                .unwrap_or_else(|e| panic!("LAN address should be accepted ({url}): {e}"));
        }
        // Names stay local as strings. Acceptance after DNS uses a private
        // address so the test does not depend on this machine's resolver.
        for url in [
            "http://mac.local:11434",
            "http://ollama.local",
            "http://ollama:11434",
            "http://mac:11434",
            "http://host.docker.internal:11434",
            "http://gateway.docker.internal",
        ] {
            assert!(is_local_ai_host(url), "expected local: {url}");
            ensure_named_ai_host(url, "AI host", false, &mut |_| {
                Ok(vec![IpAddr::from([192, 168, 1, 20])])
            })
            .unwrap_or_else(|e| {
                panic!("LAN name resolving to a private address should be accepted ({url}): {e}")
            });
        }
    }

    #[test]
    fn ipv6_unique_local_accepted() {
        assert!(is_local_ai_host("http://[fd12:3456:789a::1]:11434"));
        ensure_ai_host_allowed_with("http://[fd12:3456:789a::1]:11434", false).unwrap();
    }

    #[test]
    fn public_hosts_refused_without_opt_in() {
        for url in [
            "https://api.openai.com",
            "https://api.anthropic.com/v1",
            "http://ollama.com:11434",
            "https://8.8.8.8:11434",
            "http://1.1.1.1",
        ] {
            assert!(!is_local_ai_host(url), "expected public: {url}");
            let err = ensure_ai_host_allowed_with(url, false)
                .expect_err(&format!("public host must be refused: {url}"));
            let msg = err.to_string();
            assert!(msg.contains(url), "error should name the URL: {msg}");
            assert!(
                msg.contains(ALLOW_REMOTE_AI_ENV),
                "error should name the opt-in: {msg}"
            );
        }
    }

    #[test]
    fn public_host_allowed_with_opt_in() {
        ensure_ai_host_allowed_with("https://api.openai.com", true)
            .expect("opt-in should allow a public host");
    }

    #[test]
    fn empty_or_unparseable_is_not_local() {
        assert!(!is_local_ai_host(""));
        assert!(!is_local_ai_host("   "));
        assert!(!is_local_ai_host("http://"));
        ensure_ai_host_allowed_with("", false).expect_err("empty URL is not local");
    }

    #[test]
    fn unspecified_bind_addresses_are_not_local_clients() {
        // 0.0.0.0 / :: are bind addresses, not a destination you talk to.
        assert!(!is_local_ai_host("http://0.0.0.0:11434"));
        assert!(!is_local_ai_host("http://[::]:11434"));
    }

    #[test]
    fn integer_ipv4_and_unbracketed_ipv6_refused_without_opt_in() {
        // WHATWG/reqwest parse 134744072 / 0x8080808 as 8.8.8.8.
        for url in [
            "http://134744072",
            "http://134744072:11434",
            "http://0x8080808",
            "http://0X8080808:11434",
            "http://2001:4860:4860::8888",
            "http://fe80::1",
        ] {
            assert!(!is_local_ai_host(url), "expected refused: {url}");
            let err = ensure_ai_host_allowed_with(url, false)
                .expect_err(&format!("must be refused without opt-in: {url}"));
            let msg = err.to_string();
            assert!(
                msg.contains(ALLOW_REMOTE_AI_ENV),
                "error should name the opt-in: {msg}"
            );
        }
    }

    #[test]
    fn single_label_lan_and_docker_internal_still_local() {
        for url in [
            "http://ollama:11434",
            "http://mac",
            "http://hone-ai",
            "http://ollama_gpu:11434",
            "http://ollama_server",
            "http://host.docker.internal:11434",
            "http://gateway.docker.internal",
        ] {
            assert!(is_local_ai_host(url), "expected local: {url}");
            ensure_named_ai_host(url, "AI host", false, &mut |_| {
                Ok(vec![IpAddr::from([10, 0, 0, 5])])
            })
            .unwrap_or_else(|e| panic!("should stay local when resolved private ({url}): {e}"));
        }
    }

    #[test]
    fn opt_in_flag_parsing() {
        assert!(remote_ai_opt_in_value(Some("1")));
        assert!(remote_ai_opt_in_value(Some("true")));
        assert!(remote_ai_opt_in_value(Some("YES")));
        assert!(remote_ai_opt_in_value(Some(" True ")));
        assert!(!remote_ai_opt_in_value(Some("0")));
        assert!(!remote_ai_opt_in_value(Some("false")));
        assert!(!remote_ai_opt_in_value(Some("")));
        assert!(!remote_ai_opt_in_value(None));
    }

    #[test]
    fn hostname_extraction() {
        assert_eq!(
            hostname_from_url("http://192.168.1.100:11434"),
            Some("192.168.1.100".into())
        );
        assert_eq!(
            hostname_from_url("https://api.openai.com/v1"),
            Some("api.openai.com".into())
        );
        assert_eq!(hostname_from_url("http://[::1]:11434"), Some("::1".into()));
        assert_eq!(hostname_from_url("ollama:11434"), Some("ollama".into()));
    }

    #[test]
    fn resolved_public_address_refused_after_local_name() {
        assert!(is_local_ai_host("http://ollama:11434"));
        let err = ensure_named_ai_host("http://Ollama:11434", "OLLAMA_HOST", false, &mut |host| {
            assert_eq!(host, "ollama");
            Ok(vec![IpAddr::from([8, 8, 8, 8])])
        })
        .expect_err("a local-looking name that resolves public must be refused");
        let msg = err.to_string();
        assert!(msg.contains("8.8.8.8"), "{msg}");
        assert!(msg.contains("OLLAMA_HOST"), "{msg}");
        assert!(msg.contains("http://Ollama:11434"), "{msg}");
    }

    #[test]
    fn resolved_loopback_rfc1918_and_ula_accepted() {
        let accepted = [
            IpAddr::from([127, 0, 0, 1]),
            IpAddr::from([10, 1, 2, 3]),
            IpAddr::from([172, 16, 0, 1]),
            IpAddr::from([192, 168, 0, 9]),
            "::1".parse().unwrap(),
            "fd00::1".parse().unwrap(),
        ];
        for ip in accepted {
            ensure_named_ai_host(
                "http://ollama.local:11434",
                "AI host",
                false,
                &mut move |_| Ok(vec![ip]),
            )
            .unwrap_or_else(|e| panic!("{ip} should be accepted: {e}"));
        }
    }

    #[test]
    fn resolved_non_local_addresses_refused() {
        for raw in [
            "169.254.1.1",
            "100.64.0.1",
            "203.0.113.5",
            "fe80::1",
            "2001:db8::1",
        ] {
            let ip: IpAddr = raw.parse().unwrap();
            let err = ensure_named_ai_host("http://mac.local", "AI host", false, &mut move |_| {
                Ok(vec![ip])
            })
            .expect_err(raw);
            assert!(
                err.to_string().contains(raw) || err.to_string().contains(&ip.to_string()),
                "{err}"
            );
        }
    }

    #[test]
    fn mixed_local_and_public_resolution_refused() {
        let err = ensure_named_ai_host("http://ollama:11434", "AI host", false, &mut |_| {
            Ok(vec![
                IpAddr::from([127, 0, 0, 1]),
                IpAddr::from([1, 1, 1, 1]),
            ])
        })
        .expect_err("one public address is enough to refuse");
        assert!(err.to_string().contains("1.1.1.1"), "{err}");
    }

    #[test]
    fn remote_opt_in_does_not_allow_local_name_resolving_public() {
        assert!(is_local_ai_host("http://ollama:11434"));
        let err = ensure_named_ai_host("http://ollama:11434", "AI host", true, &mut |_| {
            Ok(vec![IpAddr::from([8, 8, 8, 8])])
        })
        .expect_err("opt-in applies to a public hostname, not a poisoned local name");
        assert!(err.to_string().contains("8.8.8.8"), "{err}");
    }

    #[test]
    fn public_host_string_check_does_not_resolve() {
        let mut called = false;
        let err = ensure_named_ai_host("https://api.openai.com", "AI host", false, &mut |_| {
            called = true;
            Ok(vec![IpAddr::from([1, 1, 1, 1])])
        })
        .expect_err("public host string must still be refused");
        assert!(!called, "public host must be refused before DNS");
        let msg = err.to_string();
        assert!(msg.contains("https://api.openai.com"), "{msg}");
        assert!(msg.contains(ALLOW_REMOTE_AI_ENV), "{msg}");
    }

    #[test]
    fn unresolved_local_name_and_empty_resolution_refused() {
        let err = ensure_named_ai_host("http://ollama:11434", "AI host", false, &mut |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such host",
            ))
        })
        .expect_err("resolution failure must be refused");
        assert!(err.to_string().contains("no such host"), "{err}");

        let err =
            ensure_named_ai_host("http://ollama:11434", "AI host", false, &mut |_| Ok(vec![]))
                .expect_err("empty resolution must be refused");
        assert!(err.to_string().contains("no addresses"), "{err}");
    }

    #[tokio::test]
    async fn redirect_to_openai_is_not_followed() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        use super::super::anthropic_compat::AnthropicCompatBackend;
        use super::super::ollama::OllamaBackend;
        use super::super::openai_compatible::OpenAICompatibleBackend;
        use super::super::AIBackend;

        let hits = Arc::new(AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits_srv = hits.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                hits_srv.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    read_http_request(&mut socket).await;
                    let response = concat!(
                        "HTTP/1.1 307 Temporary Redirect\r\n",
                        "Location: https://api.openai.com\r\n",
                        "Content-Length: 0\r\n",
                        "Connection: close\r\n",
                        "\r\n"
                    );
                    let _ =
                        tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes()).await;
                    let _ = tokio::io::AsyncWriteExt::shutdown(&mut socket).await;
                });
            }
        });

        let base = format!("http://{addr}");
        let resp = tokio::time::timeout(Duration::from_secs(2), ai_http_client().get(&base).send())
            .await
            .expect("timed out; the 307 to https://api.openai.com may have been followed")
            .expect("test double should return its 307");
        assert_eq!(resp.status(), reqwest::StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            resp.headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("https://api.openai.com")
        );
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "the redirect target must not be requested"
        );

        assert_stopped_at_307(
            &hits,
            AnthropicCompatBackend::new(&base, "test-model").list_models(),
        )
        .await;
        assert_stopped_at_307(
            &hits,
            OllamaBackend::new(&base, "test-model").classify_merchant("Netflix"),
        )
        .await;
        assert_stopped_at_307(
            &hits,
            OpenAICompatibleBackend::new(&base, "test-model").classify_merchant("Netflix"),
        )
        .await;
    }

    async fn assert_stopped_at_307<T, E, F>(hits: &std::sync::atomic::AtomicUsize, fut: F)
    where
        T: std::fmt::Debug,
        E: std::fmt::Display,
        F: std::future::Future<Output = std::result::Result<T, E>>,
    {
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        let before = hits.load(Ordering::SeqCst);
        let err = tokio::time::timeout(Duration::from_secs(2), fut)
            .await
            .expect("timed out; the 307 to https://api.openai.com may have been followed")
            .expect_err("307 must surface as an error from the AI client");
        let msg = err.to_string();
        assert!(
            msg.contains("307"),
            "expected the test double's 307, got: {msg}"
        );
        assert!(
            !msg.contains("api.openai.com"),
            "response names the redirect target, so a second hop may have been sent: {msg}"
        );
        assert_eq!(
            hits.load(Ordering::SeqCst),
            before + 1,
            "a second request was sent after the 307"
        );
    }

    async fn read_http_request(socket: &mut tokio::net::TcpStream) {
        use tokio::io::AsyncReadExt;

        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        let header_end = loop {
            let n = match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                socket.read(&mut tmp),
            )
            .await
            {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return,
                Ok(Ok(n)) => n,
            };
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
            if buf.len() > 65_536 {
                return;
            }
        };
        let headers = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);
        while buf.len() - header_end < content_length {
            let n = match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                socket.read(&mut tmp),
            )
            .await
            {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return,
                Ok(Ok(n)) => n,
            };
            buf.extend_from_slice(&tmp[..n]);
        }
    }
}
