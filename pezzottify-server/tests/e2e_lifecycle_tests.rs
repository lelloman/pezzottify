//! Production-binary lifecycle coverage (isolated DBs and loopback ports).
#![cfg(unix)]
#[allow(dead_code)]
mod common;

use common::{create_test_catalog, create_test_db_with_users, TestClient, ADMIN_PASS, ADMIN_USER};
use futures::StreamExt;
use std::{
    fs,
    net::TcpListener,
    process::{Child, Command},
    time::{Duration, Instant},
};

struct Process {
    child: Child,
    dir: tempfile::TempDir,
    port: u16,
    metrics_port: u16,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Process {
    fn start(port: u16, metrics_port: u16) -> Self {
        let (dir, _, media) = create_test_catalog().unwrap();
        let (_users, user_db) = create_test_db_with_users().unwrap();
        fs::copy(user_db, dir.path().join("user.db")).unwrap();
        let config = dir.path().join("config.toml");
        fs::write(
            &config,
            "secure_session_cookies = false\n[search]\nengine = \"noop\"\nbuild_on_start = false\n",
        )
        .unwrap();
        let output = fs::File::create(dir.path().join("server.log")).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_pezzottify-server"))
            .arg("--db-dir")
            .arg(dir.path())
            .arg("--media-path")
            .arg(media)
            .arg("--port")
            .arg(port.to_string())
            .arg("--metrics-port")
            .arg(metrics_port.to_string())
            .arg("--config")
            .arg(config)
            .env("LOG_LEVEL", "info")
            .stdout(output.try_clone().unwrap())
            .stderr(output)
            .spawn()
            .unwrap();
        Self {
            child,
            dir,
            port,
            metrics_port,
        }
    }
    fn logs(&self) -> String {
        fs::read_to_string(self.dir.path().join("server.log")).unwrap()
    }
    async fn ready(&mut self) {
        let client = reqwest::Client::new();
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(self.child.try_wait().unwrap().is_none(), "{}", self.logs());
            if self.logs().contains("Ready to serve") {
                let api = client
                    .get(format!("http://127.0.0.1:{}/v1/auth/session", self.port))
                    .send()
                    .await;
                let metrics = client
                    .get(format!("http://127.0.0.1:{}/metrics", self.metrics_port))
                    .send()
                    .await;
                if api.is_ok() && metrics.is_ok_and(|r| r.status().is_success()) {
                    return;
                }
            }
            assert!(Instant::now() < until, "{}", self.logs());
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    }
    async fn exited(&mut self, success: bool) {
        let until = Instant::now() + Duration::from_secs(35);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert_eq!(status.success(), success, "{}", self.logs());
                return;
            }
            assert!(Instant::now() < until, "{}", self.logs());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
fn ports() -> (u16, u16) {
    let a = TcpListener::bind("127.0.0.1:0").unwrap();
    let b = TcpListener::bind("127.0.0.1:0").unwrap();
    (
        a.local_addr().unwrap().port(),
        b.local_addr().unwrap().port(),
    )
}
async fn shutdown(signal: Option<i32>) {
    let (port, metrics_port) = ports();
    let mut server = Process::start(port, metrics_port);
    server.ready().await;
    let client = TestClient::new(format!("http://127.0.0.1:{port}"));
    let login = client.login(ADMIN_USER, ADMIN_PASS).await;
    assert_eq!(login.status(), reqwest::StatusCode::CREATED);
    let token = login
        .cookies()
        .find(|c| c.name() == "session_token")
        .unwrap()
        .value()
        .to_owned();
    let mut sockets = Vec::new();
    for path in ["ws", "mcp"] {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut request = format!("ws://127.0.0.1:{port}/v1/{path}")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        sockets.push(tokio_tungstenite::connect_async(request).await.unwrap().0);
    }
    if let Some(signal) = signal {
        // Only signal the child process created by this test.
        assert_eq!(unsafe { libc::kill(server.child.id() as i32, signal) }, 0);
    } else {
        let response = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/v1/admin/reboot"))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
        assert_eq!(response.text().await.unwrap(), "Server reboot initiated");
    }
    for mut socket in sockets {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let message = socket
                    .next()
                    .await
                    .expect("WebSocket ended without a close frame")
                    .expect("WebSocket transport failed during graceful shutdown");
                if message.is_close() {
                    break;
                }
            }
        })
        .await
        .expect("WebSocket did not close");
    }
    server.exited(true).await;
    let logs = server.logs();
    assert!(logs.contains("Scheduler shutdown complete"), "{logs}");
    assert!(logs.contains("Graceful shutdown complete"), "{logs}");
    for port in [port, metrics_port] {
        assert!(tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err());
    }
}
#[tokio::test]
async fn sigterm_drains_both_listeners_and_websockets() {
    shutdown(Some(libc::SIGTERM)).await;
}
#[tokio::test]
async fn sigint_drains_both_listeners_and_websockets() {
    shutdown(Some(libc::SIGINT)).await;
}
#[tokio::test]
async fn admin_reboot_uses_graceful_shutdown() {
    shutdown(None).await;
}
#[tokio::test]
async fn occupied_listener_fails_without_panicking() {
    for metrics_occupied in [false, true] {
        let occupied = TcpListener::bind("0.0.0.0:0").unwrap();
        let port = occupied.local_addr().unwrap().port();
        let (a, b) = ports();
        let mut server = Process::start(
            if metrics_occupied { a } else { port },
            if metrics_occupied { port } else { b },
        );
        server.exited(false).await;
        let logs = server.logs();
        assert!(logs.contains("Server failed"), "{logs}");
        assert!(!logs.contains("panicked"), "{logs}");
        assert!(!logs.contains("Ready to serve"), "{logs}");
    }
}

#[tokio::test]
async fn production_restart_restores_persisted_pause_before_manual_admission() {
    let (port, metrics_port) = ports();
    let mut server = Process::start(port, metrics_port);
    server.ready().await;
    let client = TestClient::authenticated_admin(format!("http://127.0.0.1:{port}")).await;
    assert_eq!(
        client
            .admin_set_global_job_pause(true, false)
            .await
            .status(),
        200
    );
    let jobs = client
        .admin_list_jobs()
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    // Use deterministic local maintenance, never a randomly selected network job.
    let job_id = "device_pruning";
    assert!(jobs["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|job| job["id"] == job_id));
    assert_eq!(
        unsafe { libc::kill(server.child.id() as i32, libc::SIGTERM) },
        0
    );
    server.exited(true).await;
    // Start the actual binary against the same on-disk databases and config.
    let output = fs::File::create(server.dir.path().join("server.log")).unwrap();
    server.child = Command::new(env!("CARGO_BIN_EXE_pezzottify-server"))
        .arg("--db-dir")
        .arg(server.dir.path())
        .arg("--media-path")
        .arg(server.dir.path().join("media"))
        .arg("--port")
        .arg(port.to_string())
        .arg("--metrics-port")
        .arg(metrics_port.to_string())
        .arg("--config")
        .arg(server.dir.path().join("config.toml"))
        .env("LOG_LEVEL", "info")
        .stdout(output.try_clone().unwrap())
        .stderr(output)
        .spawn()
        .unwrap();
    server.ready().await;
    let client = TestClient::authenticated_admin(format!("http://127.0.0.1:{port}")).await;
    assert_eq!(
        client
            .admin_get_job_controls()
            .await
            .json::<serde_json::Value>()
            .await
            .unwrap()["global_paused"],
        true
    );
    let response = client.admin_trigger_job(job_id).await;
    assert_eq!(response.status(), 409);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"],
        "Job execution is paused"
    );
    assert_eq!(
        client
            .admin_set_global_job_pause(false, false)
            .await
            .status(),
        200
    );
    assert_eq!(client.admin_trigger_job(job_id).await.status(), 202);
    assert_eq!(
        unsafe { libc::kill(server.child.id() as i32, libc::SIGTERM) },
        0
    );
    server.exited(true).await;
    assert!(server.logs().contains("Scheduler shutdown complete"));
}
