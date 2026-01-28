use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

pub fn decision_schema() -> serde_json::Value {
    // Schema for the model's decision JSON (used in v0.2 sentry).
    json!({
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "title": "AcipDecision",
      "type": "object",
      "required": ["tools_allowed", "risk_level", "action", "fenced_content", "reasons", "detected_patterns"],
      "properties": {
        "tools_allowed": {"type": "boolean"},
        "risk_level": {"type": "string", "enum": ["low", "medium", "high"]},
        "action": {"type": "string", "enum": ["allow", "sanitize", "block", "needs_review"]},
        "fenced_content": {"type": "string"},
        "reasons": {"type": "array", "items": {"type": "string"}},
        "detected_patterns": {"type": "array", "items": {"type": "string"}}
      },
      "additionalProperties": false
    })
}

pub fn json_error(status: StatusCode, msg: &str, extra: serde_json::Value) -> impl IntoResponse {
    let body = json!({
        "error": msg,
        "extra": extra
    });
    (status, Json(body))
}
