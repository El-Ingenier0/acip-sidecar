use crate::state::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

pub async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Only include non-secret runtime data.
    let mut policies = state.policies.list();
    policies.sort();

    let extractor = json!({
        "timeout_secs": std::env::var("ACIP_EXTRACTOR_TIMEOUT_SECS").ok(),
        "rlimit_as_mb": std::env::var("ACIP_EXTRACTOR_RLIMIT_AS_MB").ok(),
        "rlimit_nofile": std::env::var("ACIP_EXTRACTOR_RLIMIT_NOFILE").ok(),
        "rlimit_fsize_mb": std::env::var("ACIP_EXTRACTOR_RLIMIT_FSIZE_MB").ok(),
        "nice": std::env::var("ACIP_EXTRACTOR_NICE").ok(),
        "rlimit_nproc": std::env::var("ACIP_EXTRACTOR_RLIMIT_NPROC").ok(),
        "tmpdir": std::env::var("ACIP_EXTRACTOR_TMPDIR").ok(),
        // Path is not a secret but could be sensitive; include only if explicitly set.
        "bin": std::env::var("ACIP_EXTRACTOR_BIN").ok(),
    });

    let v = json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "sentry_mode": std::env::var("ACIP_SENTRY_MODE").unwrap_or_else(|_| "live".to_string()),
        "policy": {
            "head": state.policy.head,
            "tail": state.policy.tail,
            "full_if_lte": state.policy.full_if_lte,
        },
        "policies": policies,
        "extractor": extractor,
    });

    (StatusCode::OK, Json(v)).into_response()
}
