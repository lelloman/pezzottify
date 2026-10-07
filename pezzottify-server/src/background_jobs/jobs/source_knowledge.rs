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
    client: simple_server::client::Client,
    mb: String,
    wd: String,
    sparql: String,
}

impl ReferenceClient {
    pub(super) async fn research_search(&self, kind: &str, query: &str) -> Result<Value> {
        ensure!(
            matches!(kind, "recording" | "work"),
            "invalid search resource"
        );
        ensure!(
            !query.trim().is_empty() && query.len() <= 500,
            "invalid query"
        );
        self.get(
            &format!("{}/{kind}", self.mb),
            &[("query", query), ("fmt", "json"), ("limit", "10")],
        )
        .await
    }

    pub(super) async fn research_fetch(&self, kind: &str, id: &str) -> Result<Value> {
        ensure!(
            matches!(kind, "recording" | "work") && valid_mbid(id),
            "invalid source identity"
        );
        let inc = if kind == "recording" {
            "artist-credits+isrcs+work-rels+work-level-rels+artist-rels"
        } else {
            "artist-rels+recording-rels"
        };
        let value = self
            .get(
                &format!("{}/{kind}/{id}", self.mb),
                &[("fmt", "json"), ("inc", inc)],
            )
            .await?;
        ensure!(value["id"] == id, "source identity changed");
        Ok(value)
    }
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: simple_server::client::Client::builder()
                .user_agent(concat!(
                    "pezzottify/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/lelloman/pezzottify)"
                ))
                .no_redirect()
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
            static NEXT: OnceLock<
                crate::execution::sync::Mutex<Option<crate::execution::time::Instant>>,
            > = OnceLock::new();
            let mut next = NEXT
                .get_or_init(|| crate::execution::sync::Mutex::new(None))
                .lock()
                .await;
            if let Some(at) = *next {
                crate::execution::time::sleep_until(at).await;
            }
            *next = Some(crate::execution::time::Instant::now() + Duration::from_millis(1100));
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
        let spotify_match = identifier_values(entity, "P1902")
            .iter()
            .any(|v| v.as_str() == Some(spotify));
        let mbid_match = mbid.is_some_and(|id| {
            identifier_values(entity, "P434")
                .iter()
                .any(|v| v.as_str() == Some(id))
        });
        ensure!(
            mbid.is_none() || identifier_values(entity, "P434").is_empty() || mbid_match,
            "conflicting MusicBrainz and Wikidata artist identities"
        );
        ensure!(
            spotify_match || mbid_match,
            "Wikidata identity does not match catalog identifiers"
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
            result["barcode"]
                .as_str()
                .is_some_and(|barcode| barcodes_match(barcode, code))
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
        // Treat typographic apostrophes as equivalent without dropping punctuation.
        .replace(['‘', '’'], "'")
}

// A mastering label does not identify a different performance. Strip only one
// explicit terminal label, never arbitrary parentheses, years, or version text.
fn recording_title(s: &str) -> String {
    fn mastering_label(label: &str) -> bool {
        fn year(s: &str) -> bool {
            s.len() == 4
                && s.bytes().all(|c| c.is_ascii_digit())
                && s.parse::<u16>().is_ok_and(|n| (1900..=2099).contains(&n))
        }
        let words: Vec<_> = label.split_whitespace().collect();
        match words.as_slice() {
            ["remaster" | "remastered"] => true,
            [y, "remaster" | "remastered"] | ["remaster" | "remastered", y] => year(y),
            _ => false,
        }
    }
    let normalized = normalize(s);
    let bracketed = [('(', ')'), ('[', ']')]
        .into_iter()
        .find_map(|(open, close)| {
            let body = normalized.strip_suffix(close)?;
            let (title, label) = body.rsplit_once(open)?;
            mastering_label(label).then_some(title.trim_end())
        });
    let dashed = [" - ", " – ", " — "].into_iter().find_map(|separator| {
        let (title, label) = normalized.rsplit_once(separator)?;
        mastering_label(label).then_some(title.trim_end())
    });
    match bracketed.or(dashed).filter(|title| !title.is_empty()) {
        Some(title) => title.to_owned(),
        None => normalized,
    }
}

fn corroborates(candidate: &Value, context: &Value, kind: &str) -> bool {
    let title = context[kind]["name"].as_str().unwrap_or_default();
    let candidate_title = candidate["title"].as_str().unwrap_or_default();
    let titles_match = normalize(candidate_title) == normalize(title)
        || (kind == "track"
            && context["track"]["external_id_isrc"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
            && recording_title(candidate_title) == recording_title(title));
    if title.trim().is_empty() || !titles_match {
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

pub(super) fn recording_matches_catalog(recording: &Value, context: &Value) -> bool {
    let Some(code) = context["track"]["external_id_isrc"]
        .as_str()
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    corroborates(recording, context, "track")
        && recording["isrcs"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(code)))
}

// GTINs may be represented as UPC/EAN or zero-padded GTIN-14. Do not strip
// arbitrary punctuation or compare partial identifiers.
fn barcodes_match(left: &str, right: &str) -> bool {
    fn canonical(value: &str) -> Option<&str> {
        if !matches!(value.len(), 8 | 12 | 13 | 14) || !value.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let value = value.trim_start_matches('0');
        (!value.is_empty()).then_some(value)
    }
    matches!((canonical(left), canonical(right)), (Some(a), Some(b)) if a == b)
}

fn identifier_values(entity: &Value, property: &str) -> Vec<Value> {
    // These describe the identifier's label/account, not a temporal restriction
    // or a different subject. Other qualifiers still require interpretation.
    claim_values_with_qualifiers(entity, property, &["P1810", "P1552"])
}

fn claim_values(entity: &Value, property: &str) -> Vec<Value> {
    claim_values_with_qualifiers(entity, property, &[])
}

fn claim_values_with_qualifiers(entity: &Value, property: &str, allowed: &[&str]) -> Vec<Value> {
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
                .is_none_or(|q| q.keys().all(|key| allowed.contains(&key.as_str())))
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

    #[test]
    fn source_knowledge_barcodes_preserve_full_identity() {
        assert!(barcodes_match("00042282501523", "042282501523"));
        assert!(barcodes_match("081227371968", "00081227371968"));
        assert!(!barcodes_match("00042282501523", "042282501524"));
        assert!(!barcodes_match("00000000000000", "000000000000"));
        assert!(!barcodes_match("0422-82501523", "042282501523"));
        assert!(!barcodes_match("42282501523", "042282501523"));
    }

    #[test]
    fn source_knowledge_identifier_qualifiers_do_not_relax_facts() {
        let claim = |value: &str, qualifier: &str, rank: &str| {
            json!({
                "rank":rank,"qualifiers":{qualifier:[{"snaktype":"value"}]},
                "mainsnak":{"datavalue":{"value":value}}
            })
        };
        let entity = json!({"claims":{"P434":[
            claim("artist-mbid","P1810","normal"),
            claim("former-id","P582","normal"),
            claim("deprecated-id","P1810","deprecated")
        ],"P1902":[claim("spotify-id","P1552","normal")],
        "P569":[claim("1970","P1810","normal")]}});
        assert_eq!(
            identifier_values(&entity, "P434"),
            vec![json!("artist-mbid")]
        );
        assert_eq!(
            identifier_values(&entity, "P1902"),
            vec![json!("spotify-id")]
        );
        assert!(claim_values(&entity, "P569").is_empty());
    }

    #[simple_server::test(host_runtime = true)]
    async fn source_knowledge_http_resolves_ids_before_fetching_precise_facts() {
        use crate::web::{routing::get, Json, Query, Router};
        let app = Router::new().route("/sparql",get(|Query(q):Query<std::collections::HashMap<String,String>>| async move {
            assert!(q["query"].contains("wdt:P1902"));
            let ids = if q["query"].contains("ambiguous") {vec!["Q1","Q2"]} else {vec!["Q1"]};
            Json(json!({"results":{"bindings":ids.into_iter().map(|id| json!({"item":{"value":format!("http://www.wikidata.org/entity/{id}")}})).collect::<Vec<_>>()}}))
        })).route("/wd",get(|Query(q):Query<std::collections::HashMap<String,String>>| async move {
            assert_eq!(q["maxlag"],"5");
            assert_eq!(q["ids"],"Q1");
            Json(json!({"entities":{"Q1":{"id":"Q1","claims":{
                "P1902":[{"rank":"normal","qualifiers":{"P1810":[{"snaktype":"value"}]},"mainsnak":{"datavalue":{"value":"artist-id"}}}],
                "P31":[{"rank":"normal","mainsnak":{"datavalue":{"value":{"id":"Q5"}}}}],
                "P569":[{"rank":"normal","mainsnak":{"datavalue":{"value":{"time":"+1970-01-01T00:00:00Z","precision":9,"calendarmodel":"http://www.wikidata.org/entity/Q1985727"}}}}]
            }}}}))
        }));
        let upstream_server = crate::web::TestServer::tcp(app).await.unwrap();
        let addr = upstream_server.address().unwrap();
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
        drop(upstream_server);
    }

    #[simple_server::test(host_runtime = true)]
    async fn source_knowledge_http_failures_are_not_empty_knowledge() {
        use crate::web::{routing::get, Json, Router};
        let app = Router::new()
            .route(
                "/lag",
                get(|| async { Json(json!({"error":{"code":"maxlag"}})) }),
            )
            .route(
                "/limited",
                get(|| async { crate::web::StatusCode::TOO_MANY_REQUESTS }),
            );
        let upstream_server = crate::web::TestServer::tcp(app).await.unwrap();
        let addr = upstream_server.address().unwrap();
        let client = ReferenceClient::new().unwrap();
        assert!(client
            .get(&format!("http://{addr}/lag"), &[])
            .await
            .is_err());
        assert!(client
            .get(&format!("http://{addr}/limited"), &[])
            .await
            .is_err());
        drop(upstream_server);
    }

    #[simple_server::test(host_runtime = true)]
    async fn source_knowledge_http_album_accepts_zero_padded_barcode_but_not_wrong_id() {
        use crate::web::{routing::get, Json, Router};
        const ID: &str = "00000000-0000-0000-0000-000000000010";
        const WRONG: &str = "00000000-0000-0000-0000-000000000011";
        let release = |id: &str, barcode: &str| {
            json!({"id":id,"title":"Standards, Vol. 2","barcode":barcode,
            "artist-credit":[{"artist":{"name":"Keith Jarrett"}}]})
        };
        let app = Router::new()
            .route(
                "/release",
                get(move || async move {
                    Json(json!({"count":1,"releases":[release(ID,"042282501523")]}))
                }),
            )
            .route(
                &format!("/release/{ID}"),
                get(move || async move { Json(release(ID, "042282501523")) }),
            )
            .route(
                &format!("/release/{WRONG}"),
                get(move || async move { Json(release(WRONG, "042282501524")) }),
            );
        let server = crate::web::TestServer::tcp(app).await.unwrap();
        let mut client = ReferenceClient::new().unwrap();
        client.mb = format!("http://{}", server.address().unwrap());
        let context = json!({"album":{"name":"Standards, Vol. 2","external_id_upc":"00042282501523"},
            "artists":[{"name":"Keith Jarrett"}]});
        assert_eq!(
            client
                .music_entity("album", &context, None)
                .await
                .unwrap()
                .unwrap()["id"],
            ID
        );
        assert!(client
            .music_entity("album", &context, Some(WRONG))
            .await
            .is_err());
    }

    #[simple_server::test(host_runtime = true)]
    async fn source_knowledge_http_musicbrainz_corroborates_and_reuses_identity() {
        use crate::web::{routing::get, Json, Query, Router};
        const ID: &str = "00000000-0000-0000-0000-000000000001";
        let record = || json!({"id":ID,"title":"If It’s Magic","isrcs":["USAAA1200001"],"artist-credit":[{"artist":{"name":"Artist"}}]});
        let app = Router::new()
            .route(
                "/recording",
                get(
                    move |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                        if q["query"] == "isrc:USAAA1200002" {
                            let mut other = record();
                            other["id"] = json!("00000000-0000-0000-0000-000000000002");
                            Json(json!({"count":2,"recordings":[record(),other]}))
                        } else {
                            assert_eq!(q["query"], "isrc:USAAA1200001");
                            Json(json!({"count":1,"recordings":[record()]}))
                        }
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
        let upstream_server = crate::web::TestServer::tcp(app).await.unwrap();
        let addr = upstream_server.address().unwrap();
        let mut client = ReferenceClient::new().unwrap();
        client.mb = format!("http://{addr}");
        let context = json!({"track":{"name":"If It's Magic","external_id_isrc":"USAAA1200001"},"artists":[{"artist":{"name":"Artist"}}]});
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
        let mut remaster = context.clone();
        remaster["track"]["name"] = json!("If It's Magic - 2003 Remastered");
        assert!(client
            .music_entity("track", &remaster, None)
            .await
            .unwrap()
            .is_some());
        assert!(client
            .music_entity("track", &remaster, Some(ID))
            .await
            .unwrap()
            .is_some());
        remaster["track"]["name"] = json!("If It's Magic - Live - 2003 Remastered");
        assert!(client
            .music_entity("track", &remaster, None)
            .await
            .unwrap()
            .is_none());
        assert!(client
            .music_entity("track", &remaster, Some(ID))
            .await
            .is_err());
        let mut wrong = context.clone();
        wrong["artists"][0]["artist"]["name"] = json!("Namesake");
        assert!(client
            .music_entity("track", &wrong, None)
            .await
            .unwrap()
            .is_none());
        let mut ambiguous = context.clone();
        ambiguous["track"]["external_id_isrc"] = json!("USAAA1200002");
        assert!(client
            .music_entity("track", &ambiguous, None)
            .await
            .unwrap()
            .is_none());
        drop(upstream_server);
    }
    #[test]
    fn source_knowledge_remaster_labels_require_the_same_isrc_and_artists() {
        let recording = json!({"title":"Two of a Mind","isrcs":["USBB10300135"],"artist-credit":[{"artist":{"name":"Paul Desmond"}},{"artist":{"name":"Gerry Mulligan"}}]});
        let base = json!({"track":{"name":"Two of a Mind","external_id_isrc":"USBB10300135"},"artists":[{"name":"Paul Desmond"},{"name":"Gerry Mulligan"}]});
        for title in [
            "Two of a Mind - 2003 Remastered",
            "Two of a Mind (Remastered 2003)",
            "Two of a Mind [2003 Remaster]",
            "Two of a Mind – Remastered",
            "Two of a Mind — Remaster",
            "Two of a Mind - REMASTERED 2003",
        ] {
            let mut context = base.clone();
            context["track"]["name"] = json!(title);
            assert!(recording_matches_catalog(&recording, &context), "{title}");
            let mut source = recording.clone();
            source["title"] = json!(title);
            assert!(
                recording_matches_catalog(&source, &base),
                "source suffix: {title}"
            );
            context["track"]["external_id_isrc"] = json!("OTHER");
            assert!(!recording_matches_catalog(&recording, &context));
            context["track"]["external_id_isrc"] = Value::Null;
            assert!(!recording_matches_catalog(&recording, &context));
            context = base.clone();
            context["track"]["name"] = json!(title);
            context["artists"][0]["name"] = json!("Another Artist");
            assert!(!recording_matches_catalog(&recording, &context));
            let album = json!({"album":{"name":title},"artists":base["artists"]});
            assert!(
                !corroborates(&recording, &album, "album"),
                "album editions stay distinct"
            );
        }
    }

    #[test]
    fn source_knowledge_remaster_normalization_preserves_performance_and_composition_variants() {
        for different in [
            "Song (Live)",
            "Song - Live - 2003 Remastered",
            "Song (Remix) [Remastered]",
            "Song - Radio Edit",
            "Song - Acoustic",
            "Song (Part II)",
            "Song - Remastered Live",
            "Song - 2003",
            "Song (2003)",
            "Song - Remastered Bonus Track",
            "Song - 20030 Remastered",
            "Song - Remaster - Remaster",
        ] {
            assert_ne!(
                recording_title(different),
                recording_title("Song"),
                "{different}"
            );
        }
        assert_eq!(
            recording_title("Song (Live) - 2003 Remastered"),
            recording_title("Song (Live)")
        );
        assert_ne!(
            recording_title("Song (Part I) - Remastered"),
            recording_title("Song (Part II)")
        );
        assert_ne!(recording_title("(Remastered)"), "");
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

    #[test]
    fn source_knowledge_corroborates_typographic_apostrophes() {
        for kind in ["track", "album"] {
            for (local, remote) in [
                (
                    "Love's In Need Of Love Today",
                    "Love’s in Need of Love Today",
                ),
                ("If It's Magic", "If It’s Magic"),
                ("Isn't She Lovely", "Isn’t She Lovely"),
                ("‘Quoted’ Title", "'Quoted' Title"),
            ] {
                let mut context = json!({"artists":[{"name":"Artist's Name"}]});
                context[kind] = json!({"name":local});
                let candidate =
                    json!({"title":remote,"artist-credit":[{"artist":{"name":"Artist’s Name"}}]});
                assert!(corroborates(&candidate, &context, kind), "{kind}: {local}");

                // Apostrophes remain significant, and edition suffixes still differ.
                for different in [
                    local.replace(['‘', '’', '\''], ""),
                    format!("{local} (Live)"),
                ] {
                    context[kind]["name"] = json!(different);
                    assert!(!corroborates(&candidate, &context, kind));
                }
            }
        }
    }
}
