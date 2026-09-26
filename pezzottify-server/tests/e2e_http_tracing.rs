#[allow(dead_code)]
mod common;

use common::{TestClient, TestServer};
use pezzottify_server::server::RequestsLoggingLevel;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Buffer(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Buffer {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

#[tokio::test]
async fn actual_router_preserves_disabled_mode_and_traces_auth_rejections_safely() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .with_writer(Buffer(bytes.clone()))
        .init();
    for level in [
        RequestsLoggingLevel::None,
        RequestsLoggingLevel::Path,
        RequestsLoggingLevel::Headers,
        RequestsLoggingLevel::Body,
    ] {
        let enabled = level != RequestsLoggingLevel::None;
        let server = TestServer::builder()
            .with_request_logging(level)
            .spawn()
            .await;
        bytes.lock().unwrap().clear();
        let response = reqwest::Client::new()
            .get(format!(
                "{}/v1/content/artist/private-path-sentinel?token=secret-query-sentinel",
                server.base_url
            ))
            .header("authorization", "Bearer secret-header-sentinel")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        let body = response.bytes().await.unwrap();
        assert!(body.is_empty());
        // Body completion is observed by the server, independently of client receipt.
        if enabled {
            for _ in 0..100 {
                if String::from_utf8_lossy(&bytes.lock().unwrap()).contains("http.finished") {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
        let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert_eq!(output.contains("http.finished"), enabled, "{output}");
        assert_eq!(
            output.matches("http.response_headers").count(),
            usize::from(enabled),
            "{output}"
        );
        assert_eq!(
            output.matches("http.finished").count(),
            usize::from(enabled),
            "{output}"
        );
        if enabled {
            let headers = output
                .lines()
                .find(|line| line.contains("http.response_headers"))
                .unwrap();
            assert!(headers.contains("INFO"), "{headers}");
            assert!(headers.contains("simple_server::http_tracing"), "{headers}");
        }

        if enabled {
            assert!(output.contains("http.request"), "{output}");
            assert!(output.contains("http.response_headers"), "{output}");
            assert!(output.contains("/v1/content/artist/{id}"), "{output}");
            assert!(output.contains("status=401"), "{output}");
        }
        assert!(!output.contains("private-path-sentinel"), "{output}");
        assert!(!output.contains("secret-query-sentinel"), "{output}");
        assert!(!output.contains("secret-header-sentinel"), "{output}");
        assert!(!output.contains(">>>"), "{output}");
        assert!(!output.contains("<<<"), "{output}");
        // Exercise the authenticated route as well, including its unchanged error-ID policy.
        let client = TestClient::authenticated_admin(server.base_url.clone()).await;
        let response = client
            .client
            .get(format!("{}/v1/content/artist/not-present", server.base_url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    }
}
