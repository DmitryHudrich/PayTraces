use std::sync::Arc;

use axum::{extract::State, middleware::Next};

use crate::error::ApiError;
use crate::state::AppState;

/// Verifies the shared secret that the Ledgerscope.Accounts (C#) facade
/// presents on every internal call. All user/role authorization lives in
/// the C# facade; Rust only checks that a request came from a trusted
/// caller (this secret, optionally on top of mTLS). When no key is
/// configured the check is skipped (e.g. local dev).
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
