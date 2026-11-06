//! The bundled list of open news outlets and their feeds.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub url: String,
    pub section: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outlet {
    pub id: String,
    pub name: String,
    pub country: String,
    /// public, state, nonprofit or commercial.
    pub funding: String,
    /// Wikidata item for owner and funder lookups.
    pub qid: Option<String>,
    pub paywall: bool,
    pub feeds: Vec<Feed>,
}

#[derive(Deserialize)]
struct File {
    outlets: Vec<Outlet>,
}

static OUTLETS: LazyLock<Vec<Outlet>> = LazyLock::new(|| {
    serde_json::from_str::<File>(include_str!("../data/sources.json"))
        .expect("bundled sources.json is valid")
        .outlets
});

pub fn outlets() -> &'static [Outlet] {
    &OUTLETS
}

pub fn outlet(id: &str) -> Option<&'static Outlet> {
    OUTLETS.iter().find(|o| o.id == id)
}

/// Plain-language description of a funding model.
pub fn funding_text(funding: &str) -> &'static str {
    match funding {
        "public" => "Public broadcaster: paid for by a licence fee or public grant, with editorial independence set by law or charter.",
        "state" => "State-funded: owned or paid for by a national government.",
        "nonprofit" => "Nonprofit: paid for by donors, foundations and members.",
        "commercial" => "Commercial: paid for by advertising, subscriptions or an owner company.",
        _ => "Funding not recorded.",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_sources_parse_and_ids_are_unique() {
        let o = super::outlets();
        assert!(o.len() >= 15);
        let mut ids: Vec<_> = o.iter().map(|x| &x.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), o.len());
        assert!(o.iter().all(|x| ["public", "state", "nonprofit", "commercial"].contains(&x.funding.as_str())));
    }
}
