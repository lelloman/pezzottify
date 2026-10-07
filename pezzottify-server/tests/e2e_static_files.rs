//! Exercise the configured production frontend through real loopback HTTP.
mod common;
use common::TestServer;
use reqwest::{Client, StatusCode};

#[simple_server::test(host_runtime = true)]
async fn frontend_assets_spa_and_api_keep_their_contracts() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("frontend")).unwrap();
    let root = dir.path().join("frontend");
    std::fs::write(root.join("index.html"), "<h1>frontend fixture</h1>").unwrap();
    std::fs::write(root.join("app.js"), "0123456789").unwrap();
    std::fs::create_dir(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/index.html"), "docs fixture").unwrap();
    std::fs::write(dir.path().join("secret.txt"), "outside secret").unwrap();
    let server = TestServer::builder().with_frontend(&root).spawn().await;
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    for path in ["/", "/albums/123", "/missing.js", "/%2e%2e%2fsecret.txt"] {
        let response = client
            .get(format!("{}{path}", server.base_url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(response.text().await.unwrap(), "<h1>frontend fixture</h1>");
    }
    let response = client
        .get(format!("{}/app.js?v=2", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .contains("javascript"));
    let modified = response.headers()["last-modified"]
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(response.text().await.unwrap(), "0123456789");
    let response = client
        .head(format!("{}/app.js", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-length"], "10");
    assert!(response.bytes().await.unwrap().is_empty());
    let response = client
        .get(format!("{}/app.js", server.base_url))
        .header("range", "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-range"], "bytes 2-5/10");
    assert_eq!(response.text().await.unwrap(), "2345");
    let response = client
        .get(format!("{}/app.js", server.base_url))
        .header("if-modified-since", modified)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert!(response.bytes().await.unwrap().is_empty());
    let response = client
        .get(format!("{}/docs?x=1", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(response.headers()["location"], "/docs/?x=1");
    let response = client
        .get(format!("{}/docs/", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "docs fixture");
    let response = client
        .post(format!("{}/albums/123", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let response = client
        .get(format!("{}/v1/content/catalog/stats", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(response.text().await.unwrap(), "<h1>frontend fixture</h1>");
    let api_client = common::TestClient::new(server.base_url.clone());
    let response = api_client.login(common::TEST_USER, common::TEST_PASS).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_ne!(response.text().await.unwrap(), "<h1>frontend fixture</h1>");
}
