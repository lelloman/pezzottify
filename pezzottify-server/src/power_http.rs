//! Request admission and response lifetime protection shared by both services.

use futures::StreamExt;
use simple_server::web::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Json, Router,
};

pub fn readiness_routes() -> anyhow::Result<Router> {
    if !crate::power::enabled() {
        return Ok(Router::new());
    }
    let token = std::fs::read_to_string(std::env::var("POWER_READINESS_TOKEN_FILE")?)?
        .trim()
        .to_owned();
    anyhow::ensure!(token.len() >= 32, "Invalid readiness token");
    let expected = format!("Bearer {token}");
    Ok(Router::new().route(
        "/internal/ready",
        get(move |headers: HeaderMap| {
            let expected = expected.clone();
            async move {
                let supplied = headers
                    .get("Authorization")
                    .map(|value| value.as_bytes())
                    .unwrap_or_default();
                let valid = supplied.len() == expected.len()
                    && supplied
                        .iter()
                        .zip(expected.as_bytes())
                        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
                        == 0;
                if !valid {
                    return StatusCode::UNAUTHORIZED;
                }
                if crate::power::ready().await {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }
        }),
    ))
}

pub async fn activity(request: Request, next: Next) -> Response {
    use simple_server::web::response::IntoResponse;
    if !crate::power::enabled()
        || request.uri().path() == "/health"
        || request.uri().path().starts_with("/internal/")
    {
        return next.run(request).await;
    }
    let passive = request
        .headers()
        .get("X-Homelab-Activity")
        .and_then(|value| value.to_str().ok())
        == Some("passive");
    let guard = match crate::power::acquire(!passive).await {
        Ok(guard) => guard,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"code":"ACTIVITY_UNAVAILABLE"})),
            )
                .into_response()
        }
    };
    let response = next.run(request).await;
    // The socket's own idle lifetime is not activity. Messages that do work
    // acquire their own guards, while playback is tracked separately.
    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        return response;
    }
    let (parts, body) = response.into_parts();
    let stream = body.into_data_stream().map(move |chunk| {
        let _ = &guard;
        chunk
    });
    Response::from_parts(parts, Body::from_stream(stream))
}
