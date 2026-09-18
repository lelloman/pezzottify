//! Identity-first reference retrieval. No model-generated URLs or identifiers.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Map, Value};
use std::{collections::BTreeSet, sync::OnceLock, time::Duration};

#[derive(Debug, Default)]
pub(super) struct Knowledge {
    pub facts: Map<String, Value>,
    pub evidence: Vec<Value>,
    pub ids: Vec<(String, String, String)>,
    pub passages: Vec<(String, String)>,
    pub conflicts: BTreeSet<String>,
}

impl Knowledge {
    fn fact(&mut self, field: &str, value: Value, url: &str, evidence: Value) {
        if value.is_null() || value.as_str().is_some_and(|s| s.trim().is_empty()) {
            return;
        }
        self.evidence
            .push(json!({"field":field,"value":value,"source_url":url,
            "retrieved_at":super::metadata_enrichment::now_secs(),"evidence":evidence}));
        if self.facts.get(field).is_some_and(|old| old != &value) {
            self.conflicts.insert(field.to_owned());
        }
        if self.conflicts.contains(field) {
            self.facts.remove(field);
        } else {
            self.facts.insert(field.to_owned(), value);
        }
    }
}

pub(super) struct ReferenceClient {
    client: reqwest::Client,
    mb: String,
    wd: String,
    sparql: String,
}

impl ReferenceClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent(concat!(
                    "pezzottify/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/lelloman/pezzottify)"
                ))
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(25))
                .build()?,
            mb: "https://musicbrainz.org/ws/2".into(),
            wd: "https://www.wikidata.org/w/api.php".into(),
            sparql: "https://query.wikidata.org/sparql".into(),
        })
    }

    async fn get(&self, url: &str, query: &[(&str, &str)]) -> Result<Value> {
        // Shared across jobs and clients: respect MusicBrainz's one-request/sec limit.
        if url.starts_with(&self.mb) {
            static NEXT: OnceLock<tokio::sync::Mutex<Option<tokio::time::Instant>>> =
                OnceLock::new();
            let mut next = NEXT
                .get_or_init(|| tokio::sync::Mutex::new(None))
                .lock()
                .await;
            if let Some(at) = *next {
                tokio::time::sleep_until(at).await;
            }
            *next = Some(tokio::time::Instant::now() + Duration::from_millis(1100));
        }
        let mut response = self
            .client
            .get(url)
            .query(query)
            .send()
            .await?
            .error_for_status()?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= 2_097_152,
                "reference response too large"
            );
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)?;
        ensure!(
            value.get("error").is_none(),
            "reference API error: {}",
            value["error"]
        );
        Ok(value)
    }

    pub async fn artist(
        &self,
        spotify: &str,
        mbid: Option<&str>,
        saved_qid: Option<&str>,
    ) -> Result<Knowledge> {
        let mut selectors = vec![format!(
            "{{ ?item wdt:P1902 {} }}",
            serde_json::to_string(spotify)?
        )];
        if let Some(id) = mbid.filter(|id| valid_mbid(id)) {
            selectors.push(format!(
                "{{ ?item wdt:P434 {} }}",
                serde_json::to_string(id)?
            ));
        }
        let query = format!(
            "SELECT DISTINCT ?item WHERE {{ {} }} LIMIT 3",
            selectors.join(" UNION ")
        );
        let mut result = Knowledge::default();
        let id = if let Some(id) = saved_qid.filter(|s| super::work_knowledge::valid_qid(s)) {
            result
                .evidence
                .push(json!({"stage":"identity","method":"saved_identifier","qid":id}));
            id.to_owned()
        } else {
            let response = self
                .get(&self.sparql, &[("query", &query), ("format", "json")])
                .await?;
            let rows = response
                .pointer("/results/bindings")
                .and_then(Value::as_array)
                .context("missing identity bindings")?;
            let ids: BTreeSet<_> = rows
                .iter()
                .filter_map(|r| r.pointer("/item/value").and_then(Value::as_str))
                .filter_map(|s| s.rsplit('/').next())
                .filter(|s| super::work_knowledge::valid_qid(s))
                .map(str::to_owned)
                .collect();
            result
                .evidence
                .push(json!({"stage":"identity","query":query,"response":response}));
            if ids.len() != 1 {
                return Ok(result);
            }
            ids.into_iter().next().unwrap()
        };
        let response = self.entities(&id).await?;
        let entity = &response["entities"][&id];
        ensure!(entity["id"] == id, "missing resolved Wikidata entity");
        let spotify_match = claim_values(entity, "P1902")
            .iter()
            .any(|v| v.as_str() == Some(spotify));
        let mbid_match = mbid.is_some_and(|id| {
            claim_values(entity, "P434")
                .iter()
                .any(|v| v.as_str() == Some(id))
        });
        ensure!(
            mbid.is_none() || claim_values(entity, "P434").is_empty() || mbid_match,
            "conflicting MusicBrainz and Wikidata artist identities"
        );
        ensure!(
            spotify_match || mbid_match,
            "saved Wikidata identity no longer matches catalog identifiers"
        );
        let url = format!("https://www.wikidata.org/wiki/{id}");
        result
            .ids
            .push(("wikidata".into(), id.clone(), url.clone()));
        if spotify_match {
            result
                .ids
                .push(("spotify".into(), spotify.into(), url.clone()));
        }
        if let Some(mbid) = mbid.filter(|id| valid_mbid(id) && mbid_match) {
            result.ids.push((
                "musicbrainz".into(),
                mbid.into(),
                format!("https://musicbrainz.org/artist/{mbid}"),
            ));
        }
        let types = claim_values(entity, "P31");
        if types.iter().any(|v| v["id"] == "Q5") {
            result.fact(
                "kind",
                json!("person"),
                &url,
                json!({"property":"P31","claims":entity["claims"]["P31"]}),
            );
            result.fact("is_person", json!(true), &url, json!({"property":"P31"}));
        }
        if types.iter().any(|v| v["id"] == "Q215380") {
            result.fact(
                "kind",
                json!("group"),
                &url,
                json!({"property":"P31","claims":entity["claims"]["P31"]}),
            );
            result.fact("is_group", json!(true), &url, json!({"property":"P31"}));
        }
        for (property, field) in [
            ("P569", "birth_date"),
            ("P570", "death_date"),
            ("P571", "foundation_date"),
            ("P576", "dissolution_date"),
        ] {
            for value in claim_values(entity, property) {
                if let Some(date) = wikidata_time(&value) {
                    result.fact(
                        field,
                        json!(date),
                        &url,
                        json!({"property":property,"claims":entity["claims"][property]}),
                    );
                }
            }
        }
        // Citizenship is not birthplace/origin country: don't silently equate P27 with it.
        let place_property = if result.facts.get("is_person") == Some(&json!(true)) {
            "P19"
        } else {
            "P740"
        };
        let place_ids: BTreeSet<_> = claim_values(entity, place_property)
            .into_iter()
            .filter_map(|v| v["id"].as_str().map(str::to_owned))
            .collect();
        if !place_ids.is_empty() {
            let places = self
                .entities(&place_ids.iter().cloned().collect::<Vec<_>>().join("|"))
                .await?;
            for place_id in place_ids {
                if let Some(label) = places["entities"][&place_id]
                    .pointer("/labels/en/value")
                    .and_then(Value::as_str)
                {
                    result.fact("origin_place", json!(label), &url, json!({"property":place_property,"claims":entity["claims"][place_property],"place":places["entities"][&place_id]}));
                }
            }
        }
        if let Some(title) = entity
            .pointer("/sitelinks/enwiki/title")
            .and_then(Value::as_str)
        {
            let page = self
                .get(
                    "https://en.wikipedia.org/w/api.php",
                    &[
                        ("action", "query"),
                        ("format", "json"),
                        ("prop", "extracts"),
                        ("exintro", "1"),
                        ("explaintext", "1"),
                        ("titles", title),
                    ],
                )
                .await?;
            if let Some(pages) = page.pointer("/query/pages").and_then(Value::as_object) {
                for p in pages.values() {
                    if let (Some(id), Some(text)) = (p["pageid"].as_u64(), p["extract"].as_str()) {
                        result.passages.push((
                            format!("https://en.wikipedia.org/?curid={id}"),
                            text.chars().take(12_000).collect(),
                        ));
                    }
                }
            }
        }
        Ok(result)
    }

    async fn entities(&self, ids: &str) -> Result<Value> {
        self.get(
            &self.wd,
            &[
                ("action", "wbgetentities"),
                ("format", "json"),
                ("ids", ids),
                ("props", "claims|labels|sitelinks"),
                ("languages", "en"),
                ("sitefilter", "enwiki"),
                ("maxlag", "5"),
            ],
        )
        .await
    }

    pub async fn music_entity(
        &self,
        kind: &str,
        context: &Value,
        saved_mbid: Option<&str>,
    ) -> Result<Option<Value>> {
        let (entity, identifier, search_field, list) = match kind {
            "album" => (&context["album"], "external_id_upc", "barcode", "releases"),
            "track" => (&context["track"], "external_id_isrc", "isrc", "recordings"),
            _ => anyhow::bail!("unsupported reference kind"),
        };
        let Some(code) = entity[identifier].as_str().filter(|s| {
            !s.is_empty() && s.len() <= 32 && s.bytes().all(|c| c.is_ascii_alphanumeric())
        }) else {
            return Ok(None);
        };
        let resource = if kind == "album" {
            "release"
        } else {
            "recording"
        };
        let query = format!("{search_field}:{code}");
        let id = if let Some(id) = saved_mbid.filter(|s| valid_mbid(s)) {
            id.to_owned()
        } else {
            let response = self
                .get(
                    &format!("{}/{resource}", self.mb),
                    &[("query", &query), ("fmt", "json"), ("limit", "100")],
                )
                .await?;
            let rows = response[list]
                .as_array()
                .context("missing MusicBrainz candidates")?;
            if response["count"].as_u64().unwrap_or(rows.len() as u64) > rows.len() as u64 {
                return Ok(None);
            }
            let candidates: Vec<_> = rows
                .iter()
                .filter(|r| corroborates(r, context, kind))
                .collect();
            if candidates.len() != 1 {
                return Ok(None);
            }
            candidates[0]["id"]
                .as_str()
                .filter(|id| valid_mbid(id))
                .context("invalid MusicBrainz ID")?
                .to_owned()
        };
        let inc = if kind == "album" {
            "artist-credits+labels+release-groups+annotation"
        } else {
            "artist-credits+isrcs+work-rels+work-level-rels+artist-rels+annotation"
        };
        let result = self
            .get(
                &format!("{}/{resource}/{id}", self.mb),
                &[("inc", inc), ("fmt", "json")],
            )
            .await?;
        ensure!(
            result["id"] == id && corroborates(&result, context, kind),
            "MusicBrainz lookup changed identity"
        );
        let identifier_matches = if kind == "album" {
            result["barcode"].as_str() == Some(code)
        } else {
            result["isrcs"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(code)))
        };
        ensure!(identifier_matches, "MusicBrainz identifier mismatch");
        Ok(Some(result))
    }
}

pub(super) fn music_facts(kind: &str, entity: &Value) -> Knowledge {
    let mut out = Knowledge::default();
    let resource = if kind == "album" {
        "release"
    } else {
        "recording"
    };
    let id = entity["id"].as_str().unwrap_or_default();
    let url = format!("https://musicbrainz.org/{resource}/{id}");
    out.ids.push(("musicbrainz".into(), id.into(), url.clone()));
    if kind == "album" {
        if let Some(code) = entity["barcode"].as_str() {
            out.ids.push(("upc".into(), code.into(), url.clone()));
        }
    } else if let Some(codes) = entity["isrcs"].as_array() {
        for code in codes.iter().filter_map(Value::as_str) {
            out.ids.push(("isrc".into(), code.into(), url.clone()));
        }
    }
    out.evidence
        .push(json!({"stage":"resolved_entity","source_url":url,"response":entity}));
    if kind == "album" {
        // A specific release date is NOT the original release date of the album.
        if let Some(date) = entity
            .pointer("/release-group/first-release-date")
            .and_then(Value::as_str)
            .filter(|d| valid_date(d))
        {
            out.fact(
                "original_release_date",
                json!(date),
                &url,
                json!({"path":"/release-group/first-release-date"}),
            );
        }
        if let Some(labels) = entity["label-info"].as_array() {
            for label in labels {
                out.fact(
                    "label",
                    label.pointer("/label/name").cloned().unwrap_or(Value::Null),
                    &url,
                    label.clone(),
                );
                out.fact(
                    "catalog_number",
                    label["catalog-number"].clone(),
                    &url,
                    label.clone(),
                );
            }
        }
    } else {
        let works = recording_works(entity);
        if works.len() == 1 {
            out.fact(
                "work_title",
                works[0]["title"].clone(),
                &url,
                json!({"relationship":works[0]}),
            );
        }
    }
    if let Some(text) = entity["annotation"].as_str() {
        out.passages
            .push((url, text.chars().take(12_000).collect()));
    }
    out
}

pub(super) fn recording_works(entity: &Value) -> Vec<&Value> {
    entity["relations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["target-type"] == "work" && r["type"] == "performance")
        .map(|r| &r["work"])
        .collect()
}

pub(super) fn valid_mbid(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok()
}

fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn corroborates(candidate: &Value, context: &Value, kind: &str) -> bool {
    let title = context[kind]["name"].as_str().unwrap_or_default();
    if title.is_empty()
        || normalize(candidate["title"].as_str().unwrap_or_default()) != normalize(title)
    {
        return false;
    }
    let local: BTreeSet<_> = context["artists"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            a["name"]
                .as_str()
                .or_else(|| a.pointer("/artist/name").and_then(Value::as_str))
        })
        .map(normalize)
        .collect();
    let remote: BTreeSet<_> = candidate["artist-credit"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a.pointer("/artist/name").and_then(Value::as_str))
        .map(normalize)
        .collect();
    !local.is_empty() && local == remote
}

fn claim_values(entity: &Value, property: &str) -> Vec<Value> {
    let rows: Vec<_> = entity["claims"][property]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["rank"] != "deprecated")
        .collect();
    let preferred = rows.iter().any(|r| r["rank"] == "preferred");
    rows.into_iter()
        .filter(|r| !preferred || r["rank"] == "preferred")
        // Qualified claims need interpretation; don't flatten their context away.
        .filter(|r| {
            r.get("qualifiers")
                .and_then(Value::as_object)
                .is_none_or(|q| q.is_empty())
        })
        .filter_map(|r| r.pointer("/mainsnak/datavalue/value").cloned())
        .collect()
}

pub(super) fn valid_date(date: &str) -> bool {
    let parts: Vec<_> = date.split('-').collect();
    match parts.as_slice() {
        [year] => year.len() == 4 && year.parse::<u32>().is_ok_and(|y| y > 0),
        [year, month] => {
            valid_date(year)
                && month.len() == 2
                && month.parse::<u32>().is_ok_and(|m| (1..=12).contains(&m))
        }
        [_, _, _] => {
            date.len() == 10 && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok()
        }
        _ => false,
    }
}

fn wikidata_time(value: &Value) -> Option<String> {
    // Preserve precision and reject non-Gregorian dates rather than silently converting.
    if !value["calendarmodel"].as_str()?.ends_with("/Q1985727") {
        return None;
    }
    let time = value["time"].as_str()?.trim_start_matches('+');
    let len = match value["precision"].as_u64()? {
        9 => 4,
        10 => 7,
        11 => 10,
        _ => return None,
    };
    let date = time.get(..len)?;
    valid_date(date).then(|| date.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn source_knowledge_http_resolves_ids_before_fetching_precise_facts() {
        use simple_server::axum::{extract::Query, routing::get, Json, Router};
        let app = Router::new().route("/sparql",get(|Query(q):Query<std::collections::HashMap<String,String>>| async move {
            assert!(q["query"].contains("wdt:P1902"));
            let ids = if q["query"].contains("ambiguous") {vec!["Q1","Q2"]} else {vec!["Q1"]};
            Json(json!({"results":{"bindings":ids.into_iter().map(|id| json!({"item":{"value":format!("http://www.wikidata.org/entity/{id}")}})).collect::<Vec<_>>()}}))
        })).route("/wd",get(|Query(q):Query<std::collections::HashMap<String,String>>| async move {
            assert_eq!(q["maxlag"],"5");
            assert_eq!(q["ids"],"Q1");
            Json(json!({"entities":{"Q1":{"id":"Q1","claims":{
                "P1902":[{"rank":"normal","mainsnak":{"datavalue":{"value":"artist-id"}}}],
                "P31":[{"rank":"normal","mainsnak":{"datavalue":{"value":{"id":"Q5"}}}}],
                "P569":[{"rank":"normal","mainsnak":{"datavalue":{"value":{"time":"+1970-01-01T00:00:00Z","precision":9,"calendarmodel":"http://www.wikidata.org/entity/Q1985727"}}}}]
            }}}}))
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task =
            tokio::spawn(async move { simple_server::axum::serve(listener, app).await.unwrap() });
        let mut client = ReferenceClient::new().unwrap();
        client.sparql = format!("http://{addr}/sparql");
        client.wd = format!("http://{addr}/wd");
        let facts = client.artist("artist-id", None, None).await.unwrap();
        assert_eq!(facts.facts["birth_date"], "1970");
        assert_eq!(facts.ids[0].1, "Q1");
        let ambiguous = client.artist("ambiguous", None, None).await.unwrap();
        assert!(ambiguous.ids.is_empty());
        assert!(ambiguous.facts.is_empty());
        // Reuse the saved identity without making a new search request.
        client.sparql = "http://127.0.0.1:1/unused".into();
        assert_eq!(
            client
                .artist("artist-id", None, Some("Q1"))
                .await
                .unwrap()
                .facts["birth_date"],
            "1970"
        );
        assert!(client.artist("wrong-id", None, Some("Q1")).await.is_err());
        task.abort();
    }

    #[tokio::test]
    async fn source_knowledge_http_failures_are_not_empty_knowledge() {
        use simple_server::axum::{routing::get, Json, Router};
        let app = Router::new()
            .route(
                "/lag",
                get(|| async { Json(json!({"error":{"code":"maxlag"}})) }),
            )
            .route(
                "/limited",
                get(|| async { simple_server::axum::http::StatusCode::TOO_MANY_REQUESTS }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task =
            tokio::spawn(async move { simple_server::axum::serve(listener, app).await.unwrap() });
        let client = ReferenceClient::new().unwrap();
        assert!(client
            .get(&format!("http://{addr}/lag"), &[])
            .await
            .is_err());
        assert!(client
            .get(&format!("http://{addr}/limited"), &[])
            .await
            .is_err());
        task.abort();
    }

    #[tokio::test]
    async fn source_knowledge_http_musicbrainz_corroborates_and_reuses_identity() {
        use simple_server::axum::{extract::Query, routing::get, Json, Router};
        const ID: &str = "00000000-0000-0000-0000-000000000001";
        let record = || json!({"id":ID,"title":"Song","isrcs":["USAAA1200001"],"artist-credit":[{"artist":{"name":"Artist"}}]});
        let app = Router::new()
            .route(
                "/recording",
                get(
                    move |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                        assert_eq!(q["query"], "isrc:USAAA1200001");
                        Json(json!({"count":1,"recordings":[record()]}))
                    },
                ),
            )
            .route(
                &format!("/recording/{ID}"),
                get(
                    move |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                        assert!(q["inc"].contains("work-level-rels"));
                        Json(record())
                    },
                ),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task =
            tokio::spawn(async move { simple_server::axum::serve(listener, app).await.unwrap() });
        let mut client = ReferenceClient::new().unwrap();
        client.mb = format!("http://{addr}");
        let context = json!({"track":{"name":"Song","external_id_isrc":"USAAA1200001"},"artists":[{"artist":{"name":"Artist"}}]});
        assert_eq!(
            client
                .music_entity("track", &context, None)
                .await
                .unwrap()
                .unwrap()["id"],
            ID
        );
        assert!(client
            .music_entity("track", &context, Some(ID))
            .await
            .unwrap()
            .is_some());
        let mut wrong = context.clone();
        wrong["artists"][0]["artist"]["name"] = json!("Namesake");
        assert!(client
            .music_entity("track", &wrong, None)
            .await
            .unwrap()
            .is_none());
        task.abort();
    }
    #[test]
    fn source_knowledge_dates_preserve_precision() {
        let date = |precision| json!({"time":"+1970-01-01T00:00:00Z","precision":precision,"calendarmodel":"http://www.wikidata.org/entity/Q1985727"});
        assert_eq!(wikidata_time(&date(9)).as_deref(), Some("1970"));
        assert_eq!(wikidata_time(&date(10)).as_deref(), Some("1970-01"));
        assert!(wikidata_time(&date(8)).is_none());
        assert!(!valid_date("2023-02-30"));
    }
    #[test]
    fn source_knowledge_conflicts_never_choose_last_value() {
        let mut k = Knowledge::default();
        k.fact("label", json!("A"), "url", json!({}));
        k.fact("label", json!("B"), "url", json!({}));
        k.fact("label", json!("A"), "url", json!({}));
        assert!(!k.facts.contains_key("label"));
        assert!(k.conflicts.contains("label"));
    }
    #[test]
    fn source_knowledge_namesakes_need_title_and_all_artists() {
        let context = json!({"track":{"name":"Song"},"artists":[{"artist":{"name":"Artist A"}}]});
        assert!(!corroborates(
            &json!({"title":"Song","artist-credit":[{"artist":{"name":"Artist B"}}]}),
            &context,
            "track"
        ));
        assert!(corroborates(
            &json!({"title":"Song","artist-credit":[{"artist":{"name":"Artist A"}}]}),
            &context,
            "track"
        ));
    }
}
