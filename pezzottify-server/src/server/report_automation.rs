//! Report control plane and single bounded outbound worker. Database work stays on
//! the server executor; DNS and HTTP never hold a database lock or executor slot.
use super::{
    session::Session,
    state::{DatabaseHandles, ServerState},
};
use crate::{
    db_executor::DbPriority,
    server_store::{report_automation::*, reports::*},
    user::Permission,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures::FutureExt;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use simple_server::{
    extract::Extract,
    web::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::{get, post},
        Json, Router,
    },
};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

async fn run<T: Send + 'static>(
    db: DatabaseHandles,
    operation: &'static str,
    f: impl FnOnce(&dyn AutomationRepository) -> ReportResult<T> + Send + 'static,
) -> ReportResult<T> {
    let result = db
        .server
        .run(DbPriority::Interactive, move |s| {
            Ok(f(s.report_automation().ok_or(ReportError::Storage)?))
        })
        .await
        .map_err(|_| ReportError::Storage)
        .and_then(|r| r);
    if matches!(result, Err(ReportError::Storage)) {
        tracing::error!(operation, "Report automation database operation failed");
    }
    result
}
pub(super) async fn stats(db: DatabaseHandles) -> ReportResult<DeliveryStats> {
    run(db, "delivery_stats", |r| r.delivery_stats()).await
}
fn allowed(session: &Session) -> bool {
    session.has_permission(Permission::ManageReportIntegrations)
}
async fn integrations(Extract(s): Extract<Session>, State(db): State<DatabaseHandles>) -> Response {
    if !allowed(&s) {
        return super::report_admission::error(StatusCode::FORBIDDEN);
    }
    super::report_routes::output(run(db, "integrations", move |r| r.integrations(s.user_id)).await)
}
async fn create(
    Extract(s): Extract<Session>,
    State(db): State<DatabaseHandles>,
    Json(input): Json<NewIntegration>,
) -> Response {
    if !allowed(&s) {
        return super::report_admission::error(StatusCode::FORBIDDEN);
    }
    match run(db, "create_integration", move |r| {
        r.create_integration(s.user_id, input)
    })
    .await
    {
        Ok(value) => (StatusCode::CREATED, Json(value)).into_response(),
        Err(e) => e.into_response(),
    }
}
async fn update(
    Extract(s): Extract<Session>,
    State(db): State<DatabaseHandles>,
    Path(id): Path<String>,
    Json(input): Json<IntegrationUpdate>,
) -> Response {
    if !allowed(&s) {
        return super::report_admission::error(StatusCode::FORBIDDEN);
    }
    super::report_routes::output(
        run(db, "update_integration", move |r| {
            r.update_integration(s.user_id, &id, input)
        })
        .await,
    )
}
async fn deliveries(
    Extract(s): Extract<Session>,
    State(db): State<DatabaseHandles>,
    Query(filter): Query<DeliveryFilter>,
) -> Response {
    if !allowed(&s) {
        return super::report_admission::error(StatusCode::FORBIDDEN);
    }
    super::report_routes::output(
        run(db, "deliveries", move |r| r.deliveries(s.user_id, filter)).await,
    )
}
async fn retry(
    Extract(s): Extract<Session>,
    State(db): State<DatabaseHandles>,
    Path(id): Path<String>,
    Json(input): Json<DeliveryRetry>,
) -> Response {
    if !allowed(&s) {
        return super::report_admission::error(StatusCode::FORBIDDEN);
    }
    super::report_routes::output(
        run(db, "retry_delivery", move |r| {
            r.retry_delivery(s.user_id, &id, input)
        })
        .await,
    )
}
pub(super) fn routes() -> Router<ServerState> {
    Router::new()
        .route(
            "/v1/admin/reports/integrations",
            get(integrations).post(create),
        )
        .route(
            "/v1/admin/reports/integrations/{id}",
            simple_server::web::routing::patch(update),
        )
        .route("/v1/admin/reports/deliveries", get(deliveries))
        .route("/v1/admin/reports/deliveries/{id}/retry", post(retry))
}

fn signature(secret: &str, timestamp: i64, body: &str) -> Option<String> {
    let key = STANDARD.decode(secret).ok()?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).ok()?;
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body.as_bytes());
    Some(format!("v1={:x}", mac.finalize().into_bytes()))
}
fn vetted(addresses: Vec<SocketAddr>) -> Result<Vec<SocketAddr>, &'static str> {
    if addresses.is_empty()
        || addresses.len() > 64
        || addresses.iter().any(|a| !public_address(a.ip()))
    {
        Err("destination_rejected")
    } else {
        Ok(addresses)
    }
}
async fn resolve(url: &reqwest::Url) -> Result<Vec<SocketAddr>, &'static str> {
    let host = url
        .host_str()
        .ok_or("destination_rejected")?
        .trim_matches(['[', ']']);
    let port = url.port_or_known_default().ok_or("destination_rejected")?;
    let addresses = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| "transport_error")?
            .take(65)
            .collect()
    };
    vetted(addresses)
}
fn retry_after(value: Option<&str>, now: i64) -> i64 {
    value
        .and_then(|v| {
            v.parse::<i64>().ok().or_else(|| {
                chrono::DateTime::parse_from_rfc2822(v)
                    .ok()
                    .map(|d| d.timestamp() - now)
            })
        })
        .unwrap_or(0)
        .clamp(0, 3600)
}
fn transport_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
}
async fn send(d: &ClaimedDelivery) -> Result<(u16, i64), &'static str> {
    let url = validate_destination(&d.url).map_err(|_| "destination_rejected")?;
    let addresses = resolve(&url).await?;
    let host = url.host_str().ok_or("destination_rejected")?;
    let client = transport_builder()
        .https_only(true)
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|_| "transport_error")?;
    let now = chrono::Utc::now().timestamp();
    let signature = signature(&d.secret, now, &d.body).ok_or("transport_error")?;
    let response = client
        .post(url)
        .header("content-type", "application/json")
        .header("X-Pezzottify-Event-ID", &d.event_id)
        .header("X-Pezzottify-Delivery-ID", &d.id)
        .header("X-Pezzottify-Timestamp", now.to_string())
        .header("X-Pezzottify-Signature", signature)
        .body(d.body.clone())
        .send()
        .await
        .map_err(|_| "transport_error")?;
    inspect_response(response, now).await
}
async fn inspect_response(
    mut response: reqwest::Response,
    now: i64,
) -> Result<(u16, i64), &'static str> {
    let status = response.status().as_u16();
    let delay = if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        retry_after(
            response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok()),
            now,
        )
    } else {
        0
    };
    if response.content_length().is_some_and(|v| v > 4096) {
        return Err("response_too_large");
    }
    let mut bytes = 0;
    while let Some(chunk) = response.chunk().await.map_err(|_| "transport_error")? {
        bytes += chunk.len();
        if bytes > 4096 {
            return Err("response_too_large");
        }
    }
    Ok((status, delay))
}
async fn attempt(d: &ClaimedDelivery) -> (bool, i64, String) {
    // A panicking HTTP future also releases its lease through the normal finish path.
    match tokio::time::timeout(
        Duration::from_secs(10),
        std::panic::AssertUnwindSafe(send(d)).catch_unwind(),
    )
    .await
    {
        Ok(Ok(Ok((status, delay)))) => (
            (200..300).contains(&status),
            delay,
            format!("http_{status}"),
        ),
        Ok(Ok(Err(reason))) => (false, 0, reason.into()),
        Ok(Err(_)) => (false, 0, "transport_error".into()),
        Err(_) => (false, 0, "deadline".into()),
    }
}
async fn worker_step(db: DatabaseHandles) {
    let now = chrono::Utc::now().timestamp();
    let claimed = run(db.clone(), "claim_delivery", move |r| r.claim_delivery(now)).await;
    let d = match claimed {
        Ok(Some(d)) => d,
        Ok(None) => return,
        Err(_) => return,
    };
    let (success, delay, outcome) = attempt(&d).await;
    tracing::info!(delivery_id=%d.id,event_id=%d.event_id,outcome,"Report delivery attempt finished");
    let id = d.id.clone();
    let saved = run(db, "finish_delivery", move |r| {
        r.finish_delivery(
            &d.id,
            &d.token,
            &outcome,
            success,
            delay,
            chrono::Utc::now().timestamp(),
        )
    })
    .await;
    if saved.is_err() {
        tracing::error!(delivery_id=%id,"Report delivery completion failed; lease will recover");
    }
}
pub(super) fn start(server: &ServerState) {
    let db = server.database.clone();
    let shutdown = server.runtime_tasks.shutdown.clone();
    server.runtime_tasks.tasks.spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(1));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! { biased; _=shutdown.requested()=>break, _=timer.tick()=>{} }
            worker_step(db.clone()).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn transport_never_follows_redirects() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            socket.read(&mut request).await.unwrap();
            socket.write_all(format!("HTTP/1.1 302 Found\r\nLocation: http://{address}/must-not-be-requested\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
            // A redirect would create a second connection to this same listener.
            assert!(
                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err()
            );
        });
        let client = transport_builder().build().unwrap();
        let response = client
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 302);
        task.await.unwrap();
        // The production client adds HTTPS-only enforcement on top of this policy.
        assert!(transport_builder()
            .https_only(true)
            .build()
            .unwrap()
            .get(format!("http://{address}/"))
            .send()
            .await
            .is_err());
    }
    #[test]
    fn signature_covers_timestamp_and_exact_bytes() {
        let key = STANDARD.encode(b"key");
        assert_eq!(
            signature(&key, 123, "{}").unwrap(),
            "v1=28739ce20e2d3d0eb7847a31bca889409d78898aa78efb82e124f99ddd0818a2"
        );
        assert_ne!(signature(&key, 123, "{}"), signature(&key, 124, "{}"));
        assert_ne!(signature(&key, 123, "{}"), signature(&key, 123, "{}\n"));
    }
    #[test]
    fn dns_rebinding_mixed_results_and_resolution_bounds_fail_closed() {
        let good = "8.8.8.8:443".parse().unwrap();
        let private = "127.0.0.1:443".parse().unwrap();
        assert!(vetted(vec![]).is_err());
        assert!(vetted(vec![good, private]).is_err());
        assert!(vetted(vec![good; 65]).is_err());
        assert_eq!(vetted(vec![good]).unwrap(), vec![good]);
        assert!(vetted(vec!["[::ffff:8.8.8.8]:443".parse().unwrap()]).is_err());
    }
    #[test]
    fn retry_after_seconds_dates_invalid_and_extremes() {
        let now = chrono::DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT")
            .unwrap()
            .timestamp();
        assert_eq!(retry_after(Some("45"), now), 45);
        assert_eq!(retry_after(Some("999999"), now), 3600);
        assert_eq!(retry_after(Some("-1"), now), 0);
        assert_eq!(retry_after(Some("garbage"), now), 0);
        assert_eq!(retry_after(Some("Wed, 21 Oct 2015 07:29:00 GMT"), now), 60);
        assert_eq!(retry_after(Some("Wed, 21 Oct 2015 07:27:00 GMT"), now), 0);
    }
    fn response(status: u16, body: impl Into<reqwest::Body>) -> reqwest::Response {
        http::Response::builder()
            .status(status)
            .body(body.into())
            .unwrap()
            .into()
    }
    #[tokio::test]
    async fn response_bytes_and_streams_are_bounded_without_persisting_remote_content() {
        assert_eq!(
            inspect_response(response(204, ""), 0).await.unwrap(),
            (204, 0)
        );
        assert_eq!(
            inspect_response(response(200, "x".repeat(4096)), 0)
                .await
                .unwrap(),
            (200, 0)
        );
        assert_eq!(
            inspect_response(response(200, "x".repeat(4097)), 0)
                .await
                .unwrap_err(),
            "response_too_large"
        );
        let stream = futures::stream::iter(vec![
            Ok::<_, std::io::Error>(bytes::Bytes::from(vec![0; 2048])),
            Ok(bytes::Bytes::from(vec![0; 2049])),
        ]);
        assert_eq!(
            inspect_response(response(200, reqwest::Body::wrap_stream(stream)), 0)
                .await
                .unwrap_err(),
            "response_too_large"
        );
        let redirected = http::Response::builder()
            .status(302)
            .header("location", "https://127.0.0.1/")
            .body(reqwest::Body::from(""))
            .unwrap()
            .into();
        assert_eq!(inspect_response(redirected, 0).await.unwrap(), (302, 0));
        let limited = http::Response::builder()
            .status(429)
            .header("retry-after", "10000")
            .body(reqwest::Body::from(""))
            .unwrap()
            .into();
        assert_eq!(inspect_response(limited, 0).await.unwrap(), (429, 3600));
    }
    #[tokio::test]
    async fn private_literal_and_dns_are_rejected_before_any_connection() {
        for url in [
            "https://127.0.0.1:1/",
            "https://localhost:1/",
            "https://[::1]:1/",
        ] {
            let d = ClaimedDelivery {
                id: "id".into(),
                event_id: "event".into(),
                url: url.into(),
                secret: STANDARD.encode(b"key"),
                body: "{}".into(),
                token: "token".into(),
            };
            assert_eq!(send(&d).await.unwrap_err(), "destination_rejected");
        }
    }
}
