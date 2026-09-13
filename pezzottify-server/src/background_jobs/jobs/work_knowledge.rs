//! Bounded Wikidata lookup. Search results are filtered to compositions before
//! they are offered to the model; a search hit is not an identity match.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct WorkReference {
    pub qid: String,
    pub title: String,
    pub creators: Vec<String>,
    pub kind: String,
    pub url: String,
}

#[derive(Debug, Default, Serialize)]
pub(super) struct WorkKnowledge {
    pub candidates: Vec<WorkReference>,
    pub evidence: Vec<Value>,
}

#[async_trait::async_trait]
pub(super) trait WorkKnowledgeLookup: Send + Sync {
    async fn lookup(&self, title: &str) -> Result<WorkKnowledge>;
}

pub(super) struct WikidataWorkLookup;

#[async_trait::async_trait]
impl WorkKnowledgeLookup for WikidataWorkLookup {
    async fn lookup(&self, title: &str) -> Result<WorkKnowledge> {
        lookup_at(
            title,
            "https://www.wikidata.org/w/api.php",
            "https://query.wikidata.org/sparql",
        )
        .await
    }
}

pub(super) fn valid_qid(value: &str) -> bool {
    value
        .strip_prefix('Q')
        .is_some_and(|n| !n.is_empty() && n.len() <= 20 && n.bytes().all(|c| c.is_ascii_digit()))
}

async fn json_response(mut response: reqwest::Response) -> Result<Value> {
    response = response.error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 1_048_576,
            "Wikidata response too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    ensure!(
        value.get("error").is_none(),
        "Wikidata returned an API error: {}",
        value["error"]
    );
    Ok(value)
}

async fn lookup_at(title: &str, api: &str, sparql: &str) -> Result<WorkKnowledge> {
    ensure!(
        !title.trim().is_empty() && title.len() <= 500,
        "invalid Wikidata search title"
    );
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "pezzottify-server/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/lelloman/pezzottify; Work enrichment)"
        ))
        .timeout(Duration::from_secs(20))
        .build()?;
    let search = json_response(
        client
            .get(api)
            .query(&[
                ("action", "wbsearchentities"),
                ("format", "json"),
                ("language", "en"),
                ("uselang", "en"),
                ("type", "item"),
                ("limit", "10"),
                ("maxlag", "5"),
                ("search", title.trim()),
            ])
            .send()
            .await?,
    )
    .await
    .context("Wikidata entity search failed")?;
    let rows = search["search"]
        .as_array()
        .context("missing Wikidata search results")?;
    let ids: Vec<_> = rows
        .iter()
        .take(10)
        .filter_map(|r| r["id"].as_str())
        .filter(|id| valid_qid(id))
        .collect();
    let retrieved_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let mut result = WorkKnowledge {
        candidates: Vec::new(),
        evidence: vec![serde_json::json!({
            "provider":"wikidata","query":title,"retrieved_at":retrieved_at,"search_response":search,
        })],
    };
    if ids.is_empty() {
        return Ok(result);
    }
    let query = candidate_query(&ids);
    let facts = json_response(
        client
            .get(sparql)
            .header("Accept", "application/sparql-results+json")
            .query(&[("query", query.as_str()), ("format", "json")])
            .send()
            .await?,
    )
    .await
    .context("Wikidata work facts lookup failed")?;
    result.candidates = parse_candidates(&facts, &ids)?;
    result.evidence.push(serde_json::json!({"provider":"wikidata","query":query,"retrieved_at":retrieved_at,"response":facts}));
    Ok(result)
}

fn candidate_query(ids: &[&str]) -> String {
    let values = ids
        .iter()
        .filter(|id| valid_qid(id))
        .map(|id| format!("wd:{id}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        r#"PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX bd: <http://www.bigdata.com/rdf#>
SELECT DISTINCT ?item ?itemLabel ?creator ?creatorLabel ?form WHERE {{
  VALUES ?item {{ {values} }}
  VALUES ?workClass {{ wd:Q105543609 wd:Q207628 wd:Q7366 }}
  ?item wdt:P31/wdt:P279* ?workClass .
  FILTER NOT EXISTS {{ ?item wdt:P31/wdt:P279* wd:Q482994 }}
  FILTER NOT EXISTS {{ ?item wdt:P31/wdt:P279* wd:Q7302866 }}
  ?item (wdt:P86|wdt:P676) ?creator .
  OPTIONAL {{
    VALUES ?form {{ wd:Q7366 wd:Q929848 wd:Q178122 }}
    ?item (wdt:P31|wdt:P7937)/wdt:P279* ?form .
  }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "en". }}
}} LIMIT 301"#
    )
}

fn parse_candidates(value: &Value, allowed: &[&str]) -> Result<Vec<WorkReference>> {
    let rows = value["results"]["bindings"]
        .as_array()
        .context("missing Wikidata bindings")?;
    // Do not silently accept a truncated set of creators.
    ensure!(rows.len() <= 300, "too many Wikidata work facts");
    let mut works = BTreeMap::<String, WorkReference>::new();
    let mut incomplete = std::collections::HashSet::new();
    for row in rows {
        let field = |name: &str| {
            row[name]["value"]
                .as_str()
                .context("invalid Wikidata binding")
        };
        let uri = field("item")?;
        let qid = uri
            .strip_prefix("http://www.wikidata.org/entity/")
            .or_else(|| uri.strip_prefix("https://www.wikidata.org/entity/"))
            .context("invalid Wikidata item URL")?;
        ensure!(
            allowed.contains(&qid) && valid_qid(qid),
            "unrequested Wikidata item"
        );
        let title = field("itemLabel")?;
        let creator = field("creatorLabel")?;
        if title == qid
            || valid_qid(creator)
            || title.trim().is_empty()
            || creator.trim().is_empty()
        {
            incomplete.insert(qid.to_string());
            continue;
        }
        let work = works.entry(qid.into()).or_insert_with(|| WorkReference {
            qid: qid.into(),
            title: title.into(),
            creators: Vec::new(),
            kind: "composition".into(),
            url: format!("https://www.wikidata.org/wiki/{qid}"),
        });
        match row["form"]["value"]
            .as_str()
            .and_then(|s| s.rsplit('/').next())
        {
            Some("Q929848") => work.kind = "movement".into(),
            Some("Q178122") if work.kind != "movement" => work.kind = "aria".into(),
            Some("Q7366") if work.kind == "composition" => work.kind = "song".into(),
            _ => {}
        }
        if !work.creators.iter().any(|c| c == creator) {
            work.creators.push(creator.into());
        }
    }
    for work in works.values_mut() {
        work.creators.sort();
    }
    Ok(works
        .into_values()
        .filter(|w| !incomplete.contains(&w.qid))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wikidata_work_http_lookup_searches_then_fetches_filtered_facts() {
        use axum::{extract::Query, routing::get, Json, Router};
        let app = Router::new()
            .route(
                "/api",
                get(
                    |Query(params): Query<std::collections::HashMap<String, String>>| async move {
                        assert_eq!(params["action"], "wbsearchentities");
                        assert_eq!(params["search"], "Song - Live");
                        assert_eq!(params["limit"], "10");
                        Json(serde_json::json!({"search":[{"id":"Q1"}]}))
                    },
                ),
            )
            .route(
                "/sparql",
                get(
                    |Query(params): Query<std::collections::HashMap<String, String>>| async move {
                        assert!(params["query"].contains("VALUES ?item { wd:Q1 }"));
                        Json(serde_json::json!({"results":{"bindings":[{
                            "item":{"value":"http://www.wikidata.org/entity/Q1"},
                            "itemLabel":{"value":"Song"},"creatorLabel":{"value":"Writer"}
                        }]}}))
                    },
                ),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let result = lookup_at(
            "Song - Live",
            &format!("http://{addr}/api"),
            &format!("http://{addr}/sparql"),
        )
        .await;
        task.abort();
        let result = result.unwrap();
        assert_eq!(result.candidates[0].qid, "Q1");
        assert_eq!(result.evidence.len(), 2);
    }

    #[tokio::test]
    async fn wikidata_work_http_errors_are_not_treated_as_empty_searches() {
        use axum::{routing::get, Json, Router};
        let app = Router::new()
            .route(
                "/lag",
                get(|| async { Json(serde_json::json!({"error":{"code":"maxlag"}})) }),
            )
            .route(
                "/limited",
                get(|| async { axum::http::StatusCode::TOO_MANY_REQUESTS }),
            )
            .route(
                "/empty",
                get(|| async { Json(serde_json::json!({"search":[]})) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        for path in ["lag", "limited"] {
            assert!(lookup_at(
                "Song",
                &format!("http://{addr}/{path}"),
                "invalid-unused-endpoint"
            )
            .await
            .is_err());
        }
        let empty = lookup_at(
            "Song",
            &format!("http://{addr}/empty"),
            "invalid-unused-endpoint",
        )
        .await
        .unwrap();
        task.abort();
        assert!(empty.candidates.is_empty());
        assert_eq!(empty.evidence.len(), 1);
    }

    #[tokio::test]
    #[ignore = "requires live Wikidata APIs; run explicitly as a smoke test"]
    async fn wikidata_work_live_lookup() {
        for title in ["Für Elise", "Yesterday"] {
            let result = WikidataWorkLookup.lookup(title).await.unwrap();
            assert!(!result.candidates.is_empty(), "no candidates for {title}");
            if title == "Für Elise" {
                assert!(result.candidates.iter().any(|c| c.qid == "Q11980"));
            }
            for candidate in result.candidates {
                println!(
                    "{}: {} ({})",
                    candidate.qid,
                    candidate.title,
                    candidate.creators.join(", ")
                );
            }
        }
    }
    #[test]
    fn wikidata_work_query_accepts_only_item_ids_and_filters_recordings() {
        let query = candidate_query(&["Q1", "Q2 } UNION { ?x ?y ?z", "not-an-id"]);
        assert!(query.contains("VALUES ?item { wd:Q1 }"));
        assert!(query.contains("wd:Q207628"));
        assert!(query.contains("FILTER NOT EXISTS"));
        assert!(!query.contains("UNION"));
    }
    #[test]
    fn wikidata_work_facts_group_creators_and_reject_unrequested_entities() {
        let row = |writer: &str| serde_json::json!({"item":{"value":"http://www.wikidata.org/entity/Q1"},"itemLabel":{"value":"Song"},"creatorLabel":{"value":writer}});
        let value = serde_json::json!({"results":{"bindings":[row("B"),row("A"),row("B")]}});
        let works = parse_candidates(&value, &["Q1"]).unwrap();
        assert_eq!(works[0].creators, vec!["A", "B"]);
        assert!(parse_candidates(&value, &["Q2"]).is_err());
        assert!(parse_candidates(&serde_json::json!({}), &[]).is_err());
    }
}
