//! Server command implementation

use std::path::Path;

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
) -> Result<()> {
    println!("🚀 Starting Hone web server...");
    println!("   Database: {}", db_path.display());
    println!("   Listening: http://{}:{}", host, port);
    if let Some(dir) = static_dir {
        println!("   Static files: {}", dir.display());
    }
    if let Some(mcp) = mcp_port {
        println!("   MCP server: http://{}:{}/mcp", host, mcp);
    }

    // Parse API keys from environment (comma-separated)
    let api_keys: Vec<String> = std::env::var("HONE_API_KEYS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

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
                "   🔑 API keys: {} configured (HONE_API_KEYS)",
                api_keys.len()
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
    println!();
    println!("   Press Ctrl+C to stop");

    let db = open_db(db_path, no_encrypt)?;

    // Ensure root tags are seeded (idempotent)
    db.seed_root_tags().context("Failed to seed root tags")?;

    let config = hone_server::ServerConfig {
        require_auth: !no_auth,
        allowed_origins: vec![],
        api_keys,
        cf_jwt: hone_server::CfJwtConfig {
            team_name: cf_team_name,
            audience: cf_aud_tag,
            cached_keys: None,
        },
        trusted_networks,
        trusted_proxies,
    };

    // Start MCP server if port specified
    if let Some(mcp) = mcp_port {
        let mcp_db = db.clone();
        let mcp_host = host.to_string();
        tokio::spawn(async move {
            if let Err(e) = hone_server::mcp::start_mcp_server(mcp_db, &mcp_host, mcp).await {
                eprintln!("MCP server error: {}", e);
            }
        });
    }

    let static_dir_str =
        static_dir.map(|p| p.to_str().expect("static_dir path must be valid UTF-8"));
    hone_server::serve_with_config(db, host, port, static_dir_str, config).await?;

    Ok(())
}
