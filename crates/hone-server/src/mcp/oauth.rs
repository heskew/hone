//! MCP OAuth 2.1 resource-server surface (spec 2026-07-28).
//!
//! Hone is the MCP resource server, not an authorization server. This module
//! implements the smallest honest subset of
//! [MCP Authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization):
//!
//! - RFC 9728 Protected Resource Metadata
//! - RFC 6750 `WWW-Authenticate` on 401 (`resource_metadata`, `scope`)
//! - RFC 8707 audience: tokens must be issued for the MCP resource URI
//!
//! Skipped (documented in `docs/mcp.md`): hosting an AS (RFC 8414, authorize,
//! token, PKCE, DCR, CIMD), refresh tokens, DPoP, step-up 403.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::ServerConfig;

/// Scope advertised for read-only MCP tools.
pub const MCP_READ_SCOPE: &str = "mcp:read";

/// Canonical MCP resource and how to validate audience-bound tokens.
#[derive(Clone, Default)]
pub struct McpOAuthConfig {
    /// RFC 8707 / RFC 9728 resource identifier (e.g. `http://pi:3001/mcp`).
    pub resource: Option<String>,
    /// AS issuer URIs for PRM `authorization_servers` (empty if we are not
    /// pointing clients at an AS).
    pub authorization_servers: Vec<String>,
    /// HS256 secret for locally minted MCP-audience JWTs (`HONE_MCP_JWT_SECRET`).
    pub jwt_secret: Option<String>,
    /// Expected `iss` when validating AS-issued JWTs.
    pub issuer: Option<String>,
    /// JWKS URL for RS256 tokens from an external AS (`HONE_MCP_JWKS_URL`).
    pub jwks_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpAccessClaims {
    aud: Audience,
    exp: u64,
    #[serde(default)]
    iat: Option<u64>,
    #[serde(default)]
    iss: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    scp: Option<Vec<String>>,
}

/// JWT `aud` may be a string or array (RFC 7519).
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

/// RFC 9728 document for this MCP server. `authorization_servers` is omitted
/// when none are configured — we are not pretending to host an AS.
pub fn protected_resource_metadata(config: &ServerConfig) -> serde_json::Value {
    let resource = config
        .mcp_oauth
        .resource
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1/mcp".to_string());

    let mut doc = serde_json::json!({
        "resource": resource,
        "bearer_methods_supported": ["header"],
        "scopes_supported": [MCP_READ_SCOPE],
        "resource_name": "Hone MCP",
    });

    if !config.mcp_oauth.authorization_servers.is_empty() {
        doc["authorization_servers"] = serde_json::json!(config.mcp_oauth.authorization_servers);
    }

    doc
}

/// RFC 9728 §5.1 / RFC 6750 challenge for MCP 401s.
pub fn www_authenticate(config: &ServerConfig) -> String {
    let mut parts = vec![
        r#"Bearer realm="mcp""#.to_string(),
        format!(r#"scope="{MCP_READ_SCOPE}""#),
    ];
    if let Some(url) = resource_metadata_url(config) {
        parts.insert(1, format!(r#"resource_metadata="{url}""#));
    }
    parts.join(", ")
}

/// Origin `/.well-known/oauth-protected-resource` (RFC 9728 insert-after-authority).
pub fn resource_metadata_url(config: &ServerConfig) -> Option<String> {
    let resource = config.mcp_oauth.resource.as_deref()?;
    let origin = resource_origin(resource)?;
    Some(format!("{origin}/.well-known/oauth-protected-resource"))
}

/// Path-aware well-known URL for a resource that includes `/mcp`.
#[cfg(test)]
pub fn resource_metadata_url_with_path(config: &ServerConfig) -> Option<String> {
    let resource = config.mcp_oauth.resource.as_deref()?;
    let origin = resource_origin(resource)?;
    let path = resource_path(resource);
    if path.is_empty() || path == "/" {
        return Some(format!("{origin}/.well-known/oauth-protected-resource"));
    }
    Some(format!(
        "{origin}/.well-known/oauth-protected-resource{path}"
    ))
}

pub fn resource_origin(resource: &str) -> Option<String> {
    let (scheme, after) = resource.split_once("://")?;
    if scheme.is_empty() || after.is_empty() {
        return None;
    }
    let authority = after.split('/').next().filter(|a| !a.is_empty())?;
    Some(format!("{scheme}://{authority}"))
}

#[cfg(test)]
fn resource_path(resource: &str) -> &str {
    resource
        .split_once("://")
        .and_then(|(_, after)| after.find('/').map(|i| &after[i..]))
        .unwrap_or("")
}

/// Mint an HS256 access token bound to `resource` (RFC 8707 audience).
///
/// This is not an authorization-server token endpoint. It exists so a
/// self-hosted install can issue MCP-audience JWTs without standing up an AS.
pub fn mint_mcp_access_token(
    secret: &str,
    resource: &str,
    ttl_secs: u64,
) -> anyhow::Result<String> {
    if secret.is_empty() {
        anyhow::bail!("HONE_MCP_JWT_SECRET is empty");
    }
    if resource_origin(resource).is_none() {
        anyhow::bail!("HONE_MCP_RESOURCE must be an absolute URI with a scheme");
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("clock error: {e}"))?
        .as_secs();

    let claims = McpAccessClaims {
        aud: Audience::One(resource.to_string()),
        exp: now.saturating_add(ttl_secs),
        iat: Some(now),
        iss: None,
        scope: Some(MCP_READ_SCOPE.to_string()),
        scp: None,
    };

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow::anyhow!("failed to sign MCP token: {e}"))
}

pub fn looks_like_jwt(token: &str) -> bool {
    let mut dots = 0;
    for c in token.chars() {
        if c == '.' {
            dots += 1;
        }
    }
    dots == 2 && !token.starts_with('.') && !token.ends_with('.')
}

/// Validate an HS256 MCP-audience JWT. Returns `Ok` only when `aud` matches
/// the configured MCP resource (not the main API).
pub fn validate_mcp_hs256(token: &str, config: &McpOAuthConfig) -> Result<(), String> {
    let secret = config
        .jwt_secret
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("MCP JWT secret not configured")?;
    let resource = config
        .resource
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("MCP resource not configured")?;

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_required_spec_claims(&["exp", "aud"]);
    validation.set_audience(&[resource]);
    validation.validate_exp = true;
    if let Some(iss) = config.issuer.as_deref().filter(|s| !s.is_empty()) {
        validation.set_issuer(&[iss]);
    }

    let data = jsonwebtoken::decode::<McpAccessClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| format!("MCP JWT validation failed: {e}"))?;

    if !has_mcp_read_scope(&data.claims) {
        return Err("MCP token missing mcp:read scope".into());
    }
    Ok(())
}

fn has_mcp_read_scope(claims: &McpAccessClaims) -> bool {
    let has_scope_claim =
        claims.scope.is_some() || claims.scp.as_ref().is_some_and(|s| !s.is_empty());
    if !has_scope_claim {
        return true;
    }
    if claims
        .scope
        .as_deref()
        .map(|s| s.split_whitespace().any(|p| p == MCP_READ_SCOPE))
        .unwrap_or(false)
    {
        return true;
    }
    claims
        .scp
        .as_ref()
        .map(|list| list.iter().any(|p| p == MCP_READ_SCOPE))
        .unwrap_or(false)
}

/// Validate an RS256 JWT against a fetched JWKS, requiring MCP `aud`.
pub fn validate_mcp_rs256_with_keys(
    token: &str,
    config: &McpOAuthConfig,
    keys: &[jsonwebtoken::jwk::Jwk],
) -> Result<(), String> {
    let resource = config
        .resource
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("MCP resource not configured")?;

    let header =
        jsonwebtoken::decode_header(token).map_err(|e| format!("Invalid JWT header: {e}"))?;
    let decoding_key = if let Some(kid) = header.kid.as_deref() {
        let jwk = keys
            .iter()
            .find(|k| k.common.key_id.as_deref() == Some(kid))
            .ok_or_else(|| format!("No matching key for kid: {kid}"))?;
        jsonwebtoken::DecodingKey::from_jwk(jwk).map_err(|e| format!("Invalid JWK: {e}"))?
    } else {
        let jwk = keys.first().ok_or("JWKS is empty")?;
        jsonwebtoken::DecodingKey::from_jwk(jwk).map_err(|e| format!("Invalid JWK: {e}"))?
    };

    let mut validation = jsonwebtoken::Validation::new(header.alg);
    validation.set_required_spec_claims(&["exp", "aud"]);
    validation.set_audience(&[resource]);
    validation.validate_exp = true;
    if let Some(iss) = config.issuer.as_deref().filter(|s| !s.is_empty()) {
        validation.set_issuer(&[iss]);
    }

    let data = jsonwebtoken::decode::<McpAccessClaims>(token, &decoding_key, &validation)
        .map_err(|e| format!("MCP JWT validation failed: {e}"))?;
    if !has_mcp_read_scope(&data.claims) {
        return Err("MCP token missing mcp:read scope".into());
    }
    Ok(())
}

pub async fn fetch_jwk_set(url: &str) -> Result<Vec<jsonwebtoken::jwk::Jwk>, String> {
    #[derive(Deserialize)]
    struct JwkSet {
        keys: Vec<jsonwebtoken::jwk::Jwk>,
    }

    let response = reqwest::Client::new()
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("JWKS request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("JWKS HTTP error: {}", response.status()));
    }

    let jwk_set: JwkSet = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JWKS: {e}"))?;
    Ok(jwk_set.keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServerConfig;

    fn config_with(resource: &str, secret: &str) -> ServerConfig {
        ServerConfig {
            mcp_oauth: McpOAuthConfig {
                resource: Some(resource.to_string()),
                jwt_secret: Some(secret.to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn metadata_omits_authorization_servers_when_unset() {
        let config = config_with("http://pi:3001/mcp", "secret");
        let doc = protected_resource_metadata(&config);
        assert_eq!(doc["resource"], "http://pi:3001/mcp");
        assert_eq!(doc["bearer_methods_supported"][0], "header");
        assert_eq!(doc["scopes_supported"][0], MCP_READ_SCOPE);
        assert!(doc.get("authorization_servers").is_none());
    }

    #[test]
    fn metadata_includes_authorization_servers_when_set() {
        let mut config = config_with("http://pi:3001/mcp", "secret");
        config.mcp_oauth.authorization_servers = vec!["https://auth.example".to_string()];
        let doc = protected_resource_metadata(&config);
        assert_eq!(doc["authorization_servers"][0], "https://auth.example");
    }

    #[test]
    fn challenge_includes_resource_metadata_and_scope() {
        let config = config_with("http://pi:3001/mcp", "secret");
        let header = www_authenticate(&config);
        assert!(header.starts_with("Bearer "));
        assert!(header.contains(
            r#"resource_metadata="http://pi:3001/.well-known/oauth-protected-resource""#
        ));
        assert!(header.contains(r#"scope="mcp:read""#));
        assert!(header.contains(r#"realm="mcp""#));
    }

    #[test]
    fn well_known_urls_follow_rfc9728_insert() {
        let config = config_with("http://pi:3001/mcp", "secret");
        assert_eq!(
            resource_metadata_url(&config).as_deref(),
            Some("http://pi:3001/.well-known/oauth-protected-resource")
        );
        assert_eq!(
            resource_metadata_url_with_path(&config).as_deref(),
            Some("http://pi:3001/.well-known/oauth-protected-resource/mcp")
        );
    }

    #[test]
    fn minted_token_validates_for_mcp_audience() {
        let resource = "http://127.0.0.1:3001/mcp";
        let secret = "test-mcp-jwt-secret";
        let token = mint_mcp_access_token(secret, resource, 60).unwrap();
        assert!(looks_like_jwt(&token));
        let config = McpOAuthConfig {
            resource: Some(resource.to_string()),
            jwt_secret: Some(secret.to_string()),
            ..Default::default()
        };
        validate_mcp_hs256(&token, &config).unwrap();
    }

    #[test]
    fn token_for_api_audience_is_rejected_on_mcp() {
        let secret = "test-mcp-jwt-secret";
        let token = mint_mcp_access_token(secret, "http://127.0.0.1:3000/api", 60).unwrap();
        let config = McpOAuthConfig {
            resource: Some("http://127.0.0.1:3001/mcp".to_string()),
            jwt_secret: Some(secret.to_string()),
            ..Default::default()
        };
        assert!(validate_mcp_hs256(&token, &config).is_err());
    }

    #[test]
    fn expired_token_is_rejected() {
        let resource = "http://127.0.0.1:3001/mcp";
        let secret = "test-mcp-jwt-secret";
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = McpAccessClaims {
            aud: Audience::One(resource.to_string()),
            exp: now.saturating_sub(120),
            iat: Some(now.saturating_sub(180)),
            iss: None,
            scope: Some(MCP_READ_SCOPE.to_string()),
            scp: None,
        };
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();
        let config = McpOAuthConfig {
            resource: Some(resource.to_string()),
            jwt_secret: Some(secret.to_string()),
            ..Default::default()
        };
        assert!(validate_mcp_hs256(&token, &config).is_err());
    }

    #[test]
    fn resource_origin_requires_scheme() {
        assert_eq!(
            resource_origin("http://pi:3001/mcp").as_deref(),
            Some("http://pi:3001")
        );
        assert!(resource_origin("pi:3001/mcp").is_none());
        assert!(resource_origin("http://").is_none());
    }
}
