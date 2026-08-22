//! Server command implementation

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::open_db;

pub async fn cmd_serve(
    db_path: &Path,
    host: &str,
    port: u16,
    no_auth: bool,
    no_encrypt: bool,
    static_dir: Option<&Path>,
    mcp_port: Option<u16>,
    mcp_allowed_hosts: Vec<String>,
) -> Result<()> {
    ensure_no_auth_allowed(host, no_auth)?;
    ensure_ai_hosts_allowed()?;

    let static_dir = resolve_static_dir(
        static_dir,
        running_in_container(),
        Path::new(CONTAINER_UI_DIST),
    );

    println!("🚀 Starting Hone web server...");
    println!("   Database: {}", db_path.display());
    println!("   Listening: http://{}:{}", host, port);
    if is_loopback_host(host) && running_in_container() {
        println!();
        println!(
            "   ⚠️  Bound to {} inside a container — published ports will NOT be reachable.",
            host
        );
        println!(
            "      Use --host 0.0.0.0 to accept connections through the container port mapping."
        );
    }
    if let Some(dir) = &static_dir {
        println!("   Static files: {}", dir.display());
    }
    if let Some(mcp) = mcp_port {
        println!("   MCP server: http://{}:{}/mcp", host, mcp);
    }

    let api_keys = parse_env_keys("HONE_API_KEYS");
    let mcp_keys = parse_env_keys("HONE_MCP_KEYS");
    let mcp_oauth = load_mcp_oauth_config(host, mcp_port);

    // Parse Cloudflare Access JWT configuration
    let cf_team_name = std::env::var("CF_TEAM_NAME").ok().filter(|s| !s.is_empty());
    let cf_aud_tag = std::env::var("CF_AUD_TAG").ok().filter(|s| !s.is_empty());
    let cf_jwt_enabled = cf_team_name.is_some() && cf_aud_tag.is_some();

    // Parse trusted networks (for local network access without auth)
    let trusted_networks_str = std::env::var("HONE_TRUSTED_NETWORKS").unwrap_or_default();
    let trusted_networks = hone_server::parse_trusted_networks(&trusted_networks_str);

    // Parse trusted proxies (for extracting real client IP behind reverse proxies)
    let trusted_proxies_str = std::env::var("HONE_TRUSTED_PROXIES").unwrap_or_default();
    let trusted_proxies = hone_server::parse_trusted_networks(&trusted_proxies_str);

    if no_auth {
        println!();
        println!("   ⚠️  Authentication DISABLED - do not expose to network!");
    } else {
        if cf_jwt_enabled {
            println!("   🔐 Authentication: Cloudflare Access (JWT validated)");
        } else {
            println!("   🔒 Authentication: Cloudflare Access (header only)");
            println!("      Set CF_TEAM_NAME and CF_AUD_TAG for cryptographic JWT validation");
        }
        if !api_keys.is_empty() {
            println!(
                "   🔑 API keys: {} configured (HONE_API_KEYS; /api and /mcp)",
                api_keys.len()
            );
        }
        if !mcp_keys.is_empty() {
            println!(
                "   🔑 MCP keys: {} configured (HONE_MCP_KEYS; opaque /mcp fallback)",
                mcp_keys.len()
            );
        }
        if let Some(resource) = mcp_oauth.resource.as_deref() {
            println!("   🪪 MCP resource: {resource} (HONE_MCP_RESOURCE)");
        }
        if mcp_oauth.jwt_secret.is_some() {
            println!("   🪪 MCP JWTs: HS256 (HONE_MCP_JWT_SECRET; aud = MCP resource)");
        }
        if mcp_oauth.jwks_url.is_some() {
            println!(
                "   🪪 MCP JWTs: JWKS {} (HONE_MCP_JWKS_URL)",
                mcp_oauth.jwks_url.as_deref().unwrap_or("")
            );
        }
        if !trusted_networks.is_empty() {
            println!(
                "   🏠 Trusted networks: {} (HONE_TRUSTED_NETWORKS)",
                trusted_networks
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !trusted_proxies.is_empty() {
            println!(
                "   🔀 Trusted proxies: {} (HONE_TRUSTED_PROXIES)",
                trusted_proxies
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    if no_encrypt {
        println!("   ⚠️  Encryption DISABLED (--no-encrypt)");
    }
    warn_if_remote_ai_hosts();
    println!();
    println!("   Press Ctrl+C to stop");

    let db = open_db(db_path, no_encrypt)?;

    // Ensure root tags are seeded (idempotent)
    db.seed_root_tags().context("Failed to seed root tags")?;

    let config = hone_server::ServerConfig {
        require_auth: !no_auth,
        allowed_origins: vec![],
        api_keys,
        mcp_keys,
        mcp_oauth,
        cf_jwt: hone_server::CfJwtConfig {
            team_name: cf_team_name,
            audience: cf_aud_tag,
            cached_keys: None,
        },
        trusted_networks,
        trusted_proxies,
    };

    // Start MCP server if port specified (same auth config as the REST API)
    if let Some(mcp) = mcp_port {
        let mcp_db = db.clone();
        let mcp_host = host.to_string();
        let mcp_hosts = mcp_allowed_hosts.clone();
        let mcp_config = config.clone();
        tokio::spawn(async move {
            if let Err(e) =
                hone_server::mcp::start_mcp_server(mcp_db, &mcp_host, mcp, &mcp_hosts, mcp_config)
                    .await
            {
                eprintln!("MCP server error: {}", e);
            }
        });
    }

    let static_dir_str = static_dir
        .as_deref()
        .map(|p| p.to_str().expect("static_dir path must be valid UTF-8"));
    hone_server::serve_with_config(db, host, port, static_dir_str, config).await?;

    Ok(())
}

/// Public `OLLAMA_HOST` / `ANTHROPIC_COMPATIBLE_HOST` values send financial
/// data off-LAN. Refuse them unless the user set `HONE_ALLOW_REMOTE_AI`.
fn ensure_ai_hosts_allowed() -> Result<()> {
    hone_core::ensure_configured_ai_hosts().map_err(|e| anyhow::anyhow!("{e}"))
}

/// After the refuse gate passes, warn if a remote host is in use via opt-in.
fn warn_if_remote_ai_hosts() {
    if !hone_core::remote_ai_is_allowed() {
        return;
    }
    for var in hone_core::AI_HOST_ENV_VARS {
        let Ok(url) = std::env::var(var) else {
            continue;
        };
        let url = url.trim();
        if url.is_empty() || hone_core::is_local_ai_host(url) {
            continue;
        }
        println!();
        println!(
            "   ⚠️  {var} is a remote AI host ({url}). Financial data will be sent there \
             because {allow}=1.",
            allow = hone_core::ALLOW_REMOTE_AI_ENV
        );
    }
}

fn load_mcp_oauth_config(host: &str, mcp_port: Option<u16>) -> hone_server::McpOAuthConfig {
    let resource = std::env::var("HONE_MCP_RESOURCE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| mcp_port.map(|port| default_mcp_resource(host, port)));

    if let (Some(port), Some(resource)) = (mcp_port, resource.as_deref()) {
        if host == "0.0.0.0" || host == "::" {
            if std::env::var("HONE_MCP_RESOURCE")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .is_none()
            {
                println!(
                    "   ⚠️  HONE_MCP_RESOURCE unset; advertised MCP resource is {resource}. \
                     Set it to the URI clients use (e.g. http://pi:{port}/mcp)."
                );
            }
        }
    }

    hone_server::McpOAuthConfig {
        resource,
        authorization_servers: parse_env_keys("HONE_MCP_AUTHORIZATION_SERVERS"),
        jwt_secret: std::env::var("HONE_MCP_JWT_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        issuer: std::env::var("HONE_MCP_ISSUER")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        jwks_url: std::env::var("HONE_MCP_JWKS_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    }
}

/// Fallback resource URI when `HONE_MCP_RESOURCE` is unset. `0.0.0.0` / `::`
/// are bind addresses, not client-reachable hosts.
fn default_mcp_resource(host: &str, port: u16) -> String {
    let host = match host {
        "0.0.0.0" | "::" => "127.0.0.1",
        "::1" => "[::1]",
        other => other,
    };
    format!("http://{host}:{port}/mcp")
}

/// Mint a local MCP-audience JWT (not an OAuth token endpoint).
pub fn cmd_mcp_token(ttl: u64) -> Result<()> {
    let secret = std::env::var("HONE_MCP_JWT_SECRET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .context("HONE_MCP_JWT_SECRET is required to mint an MCP token")?;
    let resource = std::env::var("HONE_MCP_RESOURCE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .context(
            "HONE_MCP_RESOURCE is required (canonical MCP URI, e.g. http://127.0.0.1:3001/mcp)",
        )?;
    let token = hone_server::mint_mcp_access_token(&secret, &resource, ttl)?;
    println!("{token}");
    Ok(())
}

/// Parse comma-separated Bearer keys from an environment variable.
fn parse_env_keys(var: &str) -> Vec<String> {
    parse_comma_separated_keys(&std::env::var(var).unwrap_or_default())
}

fn parse_comma_separated_keys(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `--no-auth` skips `auth_middleware`. That is only safe when the process
/// cannot be reached from another machine, i.e. the bind host is loopback.
/// Non-loopback binds (including `0.0.0.0` / `::` used for Docker published
/// ports) must use Cloudflare Access, API keys, or trusted networks.
fn ensure_no_auth_allowed(host: &str, no_auth: bool) -> Result<()> {
    if no_auth && !is_loopback_host(host) {
        anyhow::bail!(
            "--no-auth is only allowed when binding to loopback \
             (127.0.0.1, ::1, or localhost); refused for host '{host}'. \
             For non-loopback binds, use HONE_API_KEYS or HONE_TRUSTED_NETWORKS."
        );
    }
    Ok(())
}

/// Loopback inside a container is almost always a misconfiguration: the
/// container's port mapping forwards to its external interface, so a server
/// bound to 127.0.0.1 is unreachable from the host (issue #1).
fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false)
}

fn running_in_container() -> bool {
    // HONE_IN_CONTAINER is baked into the published images; /.dockerenv covers
    // plain Docker for images that predate the env var
    std::env::var_os("HONE_IN_CONTAINER").is_some() || Path::new("/.dockerenv").exists()
}

/// UI path baked into Dockerfile / Dockerfile.release.
const CONTAINER_UI_DIST: &str = "/app/ui/dist";

/// Use an explicit `--static-dir` when given. In the published image, compose
/// `command:` replaces CMD and can drop that flag (issue #63); if the image
/// UI directory is present, serve it anyway. Unset outside the image layout
/// stays unset so local CLI use is unchanged.
fn resolve_static_dir(
    explicit: Option<&Path>,
    in_container: bool,
    image_ui: &Path,
) -> Option<PathBuf> {
    if let Some(dir) = explicit {
        return Some(dir.to_path_buf());
    }
    if in_container && image_ui.is_dir() {
        return Some(image_ui.to_path_buf());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mcp_resource_rewrites_wildcard_binds() {
        assert_eq!(
            default_mcp_resource("0.0.0.0", 3001),
            "http://127.0.0.1:3001/mcp"
        );
        assert_eq!(
            default_mcp_resource("127.0.0.1", 3001),
            "http://127.0.0.1:3001/mcp"
        );
        assert_eq!(
            default_mcp_resource("pi-hostname", 3001),
            "http://pi-hostname:3001/mcp"
        );
    }

    #[test]
    fn parse_comma_separated_keys_trims_and_skips_empty() {
        assert_eq!(
            parse_comma_separated_keys(" a, b, ,c "),
            vec!["a", "b", "c"]
        );
        assert!(parse_comma_separated_keys("").is_empty());
        assert!(parse_comma_separated_keys("  , , ").is_empty());
    }

    #[test]
    fn loopback_hosts_detected() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("127.0.0.53"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("LOCALHOST"));
        assert!(is_loopback_host("::1"));
    }

    #[test]
    fn non_loopback_hosts_not_flagged() {
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("::"));
        assert!(!is_loopback_host("192.168.1.10"));
        assert!(!is_loopback_host("example.internal"));
    }

    #[test]
    fn no_auth_allowed_on_loopback() {
        for host in ["127.0.0.1", "127.0.0.53", "localhost", "LOCALHOST", "::1"] {
            ensure_no_auth_allowed(host, true)
                .unwrap_or_else(|e| panic!("--no-auth should be allowed on {host}: {e}"));
        }
    }

    #[test]
    fn no_auth_rejected_on_non_loopback() {
        for host in ["0.0.0.0", "::", "192.168.1.10", "example.internal"] {
            let err = ensure_no_auth_allowed(host, true)
                .expect_err(&format!("--no-auth must be refused on {host}"));
            let msg = err.to_string();
            assert!(
                msg.contains(host),
                "error should name the refused host {host}: {msg}"
            );
            assert!(
                msg.contains("--no-auth"),
                "error should name the flag: {msg}"
            );
        }
    }

    #[test]
    fn auth_enabled_allowed_on_any_host() {
        for host in ["0.0.0.0", "::", "192.168.1.10", "127.0.0.1"] {
            ensure_no_auth_allowed(host, false)
                .unwrap_or_else(|e| panic!("auth-enabled serve should accept host {host}: {e}"));
        }
    }

    #[test]
    fn resolve_static_dir_keeps_explicit() {
        let path = Path::new("/custom/ui");
        assert_eq!(
            resolve_static_dir(Some(path), true, Path::new(CONTAINER_UI_DIST)).as_deref(),
            Some(path)
        );
    }

    #[test]
    fn resolve_static_dir_unset_outside_container_is_none() {
        assert_eq!(
            resolve_static_dir(None, false, Path::new(CONTAINER_UI_DIST)),
            None
        );
    }

    #[test]
    fn resolve_static_dir_falls_back_when_image_path_exists() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_static_dir(None, true, tmp.path()).as_deref(),
            Some(tmp.path())
        );
    }

    #[test]
    fn resolve_static_dir_no_fallback_if_image_path_missing() {
        assert_eq!(
            resolve_static_dir(None, true, Path::new("/definitely/not/a/hone/ui/dist")),
            None
        );
    }

    #[test]
    fn compose_serve_commands_include_static_dir() {
        let contents = read_repo_file("deploy/docker-compose.yml");
        let commands: Vec<&str> = contents
            .lines()
            .filter(|line| line.contains("command:") && line.contains("serve"))
            .collect();
        assert!(
            !commands.is_empty(),
            "expected at least one serve command in deploy/docker-compose.yml"
        );
        for line in commands {
            assert!(
                line.contains("--static-dir"),
                "compose command must keep --static-dir so the image UI is served: {line}"
            );
            assert!(
                line.contains("/app/ui/dist"),
                "compose --static-dir should point at the image UI path: {line}"
            );
            assert!(
                line.contains("--host") && line.contains("0.0.0.0"),
                "compose command must keep --host 0.0.0.0: {line}"
            );
            assert!(
                !line.contains("--no-auth"),
                "compose must not pass --no-auth on the published 0.0.0.0 bind: {line}"
            );
        }
    }

    #[test]
    fn deployment_docs_mention_remote_ai_opt_in() {
        let contents = read_repo_file("docs/deployment.md");
        assert!(
            contents.contains("HONE_ALLOW_REMOTE_AI"),
            "deployment.md must document HONE_ALLOW_REMOTE_AI"
        );
        assert!(
            contents.contains("ANTHROPIC_COMPATIBLE_HOST"),
            "deployment.md must document ANTHROPIC_COMPATIBLE_HOST"
        );
        assert!(
            contents.contains("HONE_MCP_KEYS"),
            "deployment.md must document HONE_MCP_KEYS"
        );
        assert!(
            contents.contains("HONE_MCP_JWT_SECRET"),
            "deployment.md must document HONE_MCP_JWT_SECRET"
        );
        assert!(
            contents.contains("HONE_MCP_RESOURCE"),
            "deployment.md must document HONE_MCP_RESOURCE"
        );
    }

    #[test]
    fn mcp_docs_mention_scoped_keys() {
        let contents = read_repo_file("docs/mcp.md");
        assert!(
            contents.contains("HONE_MCP_JWT_SECRET"),
            "mcp.md must document HONE_MCP_JWT_SECRET"
        );
        assert!(
            contents.contains("oauth-protected-resource"),
            "mcp.md must document RFC 9728 well-known metadata"
        );
        assert!(
            contents.contains("rejected on `/api`"),
            "mcp.md must say MCP tokens are rejected on /api"
        );
    }

    #[test]
    fn deployment_docs_restore_examples_include_static_dir() {
        let contents = read_repo_file("docs/deployment.md");
        let commands: Vec<&str> = contents
            .lines()
            .filter(|line| line.contains("command:") && line.contains("serve"))
            .collect();
        assert!(
            !commands.is_empty(),
            "expected a restore command example in docs/deployment.md"
        );
        for line in commands {
            assert!(
                line.contains("--static-dir") && line.contains("/app/ui/dist"),
                "deployment.md serve examples must include --static-dir /app/ui/dist: {line}"
            );
            assert!(
                !line.contains("--no-auth"),
                "deployment.md must not recommend --no-auth on a published bind: {line}"
            );
        }
    }

    fn read_repo_file(relative: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }
}
