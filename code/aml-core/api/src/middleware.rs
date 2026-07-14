use std::sync::Arc;

use axum::{extract::State, middleware::Next};

use crate::error::ApiError;
use crate::state::AppState;

const API_VERSION_HEADER: &str = "X-API-Version";
const SUPPORTED_API_VERSIONS: &[&str] = &["1", "2"];

/// Parsed `X-API-Version`, made available to handlers via request
/// extensions so they can branch on response shape (e.g. `GET /graph`'s
/// `nodes` field: flat strings on `1`, enriched `NodeDto` on `2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiVersion(pub u8);

pub async fn version_middleware(
    mut req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, ApiError> {
    match req
        .headers()
        .get(API_VERSION_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        Some(v) if SUPPORTED_API_VERSIONS.contains(&v) => {
            let version: u8 = v.parse().expect("validated against SUPPORTED_API_VERSIONS");
            req.extensions_mut().insert(ApiVersion(version));
            Ok(next.run(req).await)
        }
        Some(other) => Err(ApiError::bad_request(format!(
            "unsupported {API_VERSION_HEADER}: {other}; supported: {}",
            SUPPORTED_API_VERSIONS.join(", ")
        ))),
        None => Err(ApiError::bad_request(format!(
            "missing required header: {API_VERSION_HEADER}"
        ))),
    }
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, ApiError> {
    let Some(expected) = state.api_key() else {
        return Ok(next.run(req).await);
    };

    let headers = req.headers();
    let provided = headers
        .get("X-Api-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(str::to_string)
        });

    match provided {
        Some(p) if p == expected => Ok(next.run(req).await),
        _ => Err(ApiError::Unauthorized),
    }
}

pub async fn admin_auth_middleware(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, ApiError> {
    let Some(expected) = state.admin_api_key() else {
        return auth_middleware(State(state), req, next).await;
    };

    let provided = req
        .headers()
        .get("X-Admin-Api-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    match provided {
        Some(p) if p == expected => Ok(next.run(req).await),
        _ => Err(ApiError::Unauthorized),
    }
}
