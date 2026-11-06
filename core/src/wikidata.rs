//! Owner and funder facts from Wikidata, cached in the store.

use anyhow::{Context, Result};
use serde_json::Value;

use crate::bias::{Link, OutletFacts};
use crate::sources::{Outlet, funding_text};

const API: &str = "https://www.wikidata.org/w/api.php";
/// owned by, parent organisation, funded by, sponsor.
const OWNER_PROPS: &[&str] = &["P127", "P749"];
const FUNDER_PROPS: &[&str] = &["P8324", "P859"];

pub fn base_facts(o: &Outlet) -> OutletFacts {
    OutletFacts {
        id: o.id.clone(),
        name: o.name.clone(),
        country: o.country.clone(),
        funding: o.funding.clone(),
        funding_text: funding_text(&o.funding).into(),
        owners: vec![],
        funders: vec![],
        wikidata: o.qid.as_ref().map(|q| format!("https://www.wikidata.org/wiki/{q}")),
    }
}

fn claim_ids(entity: &Value, props: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for p in props {
        for c in entity["claims"][p].as_array().into_iter().flatten() {
            if let Some(id) = c["mainsnak"]["datavalue"]["value"]["id"].as_str()
                && !out.iter().any(|x| x == id)
            {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// Item ids for owners and funders in a wbgetentities response.
pub fn parse_claims(body: &Value, qid: &str) -> (Vec<String>, Vec<String>) {
    let e = &body["entities"][qid];
    (claim_ids(e, OWNER_PROPS), claim_ids(e, FUNDER_PROPS))
}

pub fn parse_labels(body: &Value, ids: &[String]) -> Vec<Link> {
    ids.iter()
        .map(|id| Link {
            label: ["en", "mul", "de", "fr", "es"]
                .iter()
                .find_map(|l| body["entities"][id]["labels"][l]["value"].as_str())
                .unwrap_or(id)
                .to_string(),
            url: Some(format!("https://www.wikidata.org/wiki/{id}")),
        })
        .collect()
}

/// wbgetentities takes at most 50 ids per request.
const MAX_IDS: usize = 50;

async fn entities(client: &reqwest::Client, ids: &[String], props: &str) -> Result<Value> {
    let mut merged = serde_json::Map::new();
    for chunk in ids.chunks(MAX_IDS) {
        let body: Value = client
            .get(API)
            .header(reqwest::header::USER_AGENT, crate::feeds::CONTACT_USER_AGENT)
            .query(&[("action", "wbgetentities"), ("format", "json"), ("props", props), ("languages", "en|mul|de|fr|es"), ("ids", &chunk.join("|"))])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .with_context(|| format!("wikidata {props}"))?;
        if let Some(m) = body["entities"].as_object() {
            merged.extend(m.clone());
        }
    }
    Ok(serde_json::json!({ "entities": merged }))
}

/// Facts for several outlets in two batched requests (claims, then labels),
/// so a first run does not wait on one round trip per outlet.
pub async fn fetch_many(client: &reqwest::Client, outlets: &[&Outlet]) -> Result<Vec<OutletFacts>> {
    let qids: Vec<String> = outlets.iter().filter_map(|o| o.qid.clone()).collect();
    if qids.is_empty() {
        return Ok(outlets.iter().map(|o| base_facts(o)).collect());
    }
    let claims = entities(client, &qids, "claims").await?;
    let parsed: Vec<(Vec<String>, Vec<String>)> =
        outlets.iter().map(|o| o.qid.as_deref().map(|q| parse_claims(&claims, q)).unwrap_or_default()).collect();
    let mut all: Vec<String> = parsed.iter().flat_map(|(a, b)| a.iter().chain(b)).cloned().collect();
    all.sort();
    all.dedup();
    let labels = if all.is_empty() { Value::Null } else { entities(client, &all, "labels").await? };
    Ok(outlets
        .iter()
        .zip(parsed)
        .map(|(o, (owners, funders))| {
            let mut f = base_facts(o);
            f.owners = parse_labels(&labels, &owners);
            f.funders = parse_labels(&labels, &funders);
            f
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_owner_and_funder_ids_and_labels() {
        let claims = serde_json::json!({"entities":{"Q1":{"claims":{
            "P127":[{"mainsnak":{"datavalue":{"value":{"id":"Q9"}}}}],
            "P749":[{"mainsnak":{"datavalue":{"value":{"id":"Q9"}}}}],
            "P8324":[{"mainsnak":{"datavalue":{"value":{"id":"Q5"}}}}]}}}});
        let (o, f) = parse_claims(&claims, "Q1");
        assert_eq!(o, vec!["Q9"]);
        assert_eq!(f, vec!["Q5"]);
        let labels = serde_json::json!({"entities":{"Q9":{"labels":{"en":{"value":"Parent Co"}}}}});
        let l = parse_labels(&labels, &o);
        assert_eq!(l[0].label, "Parent Co");
        assert_eq!(parse_labels(&labels, &f)[0].label, "Q5");
    }
}
