mod common;
use common::{TestClient, TestServer, ALBUM_1_ID, TRACK_1_ID};
use reqwest::StatusCode;
use serde_json::Value;

#[simple_server::test(host_runtime = true)]
async fn lyrics_routes_require_auth_csrf_and_available_catalog_items() {
    let server = TestServer::builder().with_download_manager().spawn().await;
    let user = TestClient::authenticated(server.base_url.clone()).await;
    let read = format!("{}/v1/content/track/{TRACK_1_ID}/lyrics", server.base_url);
    let download = format!(
        "{}/v1/content/lyrics/track/{TRACK_1_ID}/download",
        server.base_url
    );
    assert_eq!(
        reqwest::Client::new()
            .get(&read)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        reqwest::Client::new()
            .post(&download)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        user.client
            .post_without_csrf(&download)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = user.client.get(&read).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.json::<Value>().await.unwrap(), Value::Null);
    for (entity, id, expected) in [
        ("track", TRACK_1_ID, StatusCode::BAD_REQUEST),
        ("album", ALBUM_1_ID, StatusCode::BAD_REQUEST),
        ("track", "missing", StatusCode::NOT_FOUND),
        ("album", "missing", StatusCode::NOT_FOUND),
        ("artist", "artist", StatusCode::BAD_REQUEST),
    ] {
        let response = user
            .client
            .post(format!(
                "{}/v1/content/lyrics/{entity}/{id}/download",
                server.base_url
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{entity}/{id}");
    }
}

#[simple_server::test(host_runtime = true)]
async fn lyrics_track_and_album_downloads_accept_and_reuse_stored_results() {
    use pezzottify_server::{catalog_store::CatalogStore, lyrics::TrackLyrics};
    let server = TestServer::builder().with_available_catalog().spawn().await;
    let ids = server
        .catalog_store
        .get_available_album_track_ids(ALBUM_1_ID)
        .unwrap();
    assert_eq!(ids.len(), 3);
    for id in ids {
        server
            .catalog_store
            .save_track_lyrics(&TrackLyrics {
                track_id: id,
                provider: "lrclib".into(),
                provider_id: Some(1),
                status: "found".into(),
                plain_lyrics: Some("A test line".into()),
                synced_lyrics: Some("[00:01.00]A test line".into()),
                fetched_at: 100,
                retry_at: 0,
            })
            .unwrap();
    }
    let user = TestClient::authenticated(server.base_url.clone()).await;
    for (entity, id, count) in [("track", TRACK_1_ID, 1), ("album", ALBUM_1_ID, 3)] {
        let response = user
            .client
            .post(format!(
                "{}/v1/content/lyrics/{entity}/{id}/download",
                server.base_url
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["tracks"], count);
    }
    let response = user
        .client
        .get(format!(
            "{}/v1/content/track/{TRACK_1_ID}/lyrics",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "found");
    assert_eq!(body["synced_lyrics"], "[00:01.00]A test line");
    assert_eq!(body["fetched_at"], 100);
}
