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

use std::net::IpAddr;

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
pub fn ensure_ai_host_allowed(url: &str) -> Result<()> {
    ensure_ai_host_allowed_with(url, remote_ai_is_allowed())
}

/// Same as [`ensure_ai_host_allowed`] with an explicit opt-in flag (for tests).
pub fn ensure_ai_host_allowed_with(url: &str, allow_remote: bool) -> Result<()> {
    if is_local_ai_host(url) || allow_remote {
        return Ok(());
    }
    Err(remote_ai_host_error("AI host", url))
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
        if is_local_ai_host(url) || allow_remote {
            continue;
        }
        return Err(remote_ai_host_error(var, url));
    }
    Ok(())
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
    if host == "localhost" {
        return true;
    }
    if host.ends_with(".local") && host.len() > ".local".len() {
        return true;
    }
    if host.ends_with(".docker.internal") {
        return true;
    }
    // Docker Compose service names and short LAN names (`ollama`, `mac`).
    if !host.contains('.') {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(is_local_ip)
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
            "http://mac.local:11434",
            "http://ollama.local",
            "http://ollama:11434",
            "http://mac:11434",
            "http://host.docker.internal:11434",
            "http://gateway.docker.internal",
        ] {
            assert!(is_local_ai_host(url), "expected local: {url}");
            ensure_ai_host_allowed_with(url, false)
                .unwrap_or_else(|e| panic!("LAN/docker host should be accepted ({url}): {e}"));
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
}
