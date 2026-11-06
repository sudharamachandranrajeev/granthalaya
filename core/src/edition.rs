//! Build one finished, dated daily paper from the day's feeds.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::bias::{self, OutletFacts, Report};
use crate::feeds::{self, Story};
use crate::{cluster, learner, sources, topics};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Prefs {
    pub blocked_outlets: Vec<String>,
    pub muted_topics: Vec<String>,
    pub followed_topics: Vec<String>,
    /// True once the reader has picked sections on first open.
    pub onboarded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telling {
    pub story_id: String,
    pub outlet_name: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditionStory {
    pub story: Story,
    pub report: Report,
    pub interest: f64,
    pub why: Vec<String>,
    /// The same event as told by other outlets.
    pub others: Vec<Telling>,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub title: String,
    pub stories: Vec<EditionStory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edition {
    pub date: String,
    pub built_at: DateTime<Utc>,
    pub number: u32,
    pub pages: Vec<Page>,
    pub sources_ok: Vec<String>,
    pub sources_failed: Vec<String>,
    pub stories_read: usize,
}

/// Fetch every feed concurrently. Failures are reported, not fatal.
/// Longest wait for any one feed, retry included.
const FEED_BUDGET: std::time::Duration = std::time::Duration::from_secs(12);

pub async fn gather(client: &reqwest::Client) -> (Vec<Story>, Vec<String>, Vec<String>) {
    let jobs = sources::outlets().iter().flat_map(|o| o.feeds.iter().map(move |f| (o, f)));
    // One slow newsroom must not hold up the whole paper: past the budget it counts as unreachable.
    let results = futures::future::join_all(jobs.map(|(o, f)| async move {
        let r = tokio::time::timeout(FEED_BUDGET, feeds::fetch(client, o, f)).await;
        (o, r.map_err(anyhow::Error::from).and_then(|r| r))
    }))
    .await;
    let mut stories = Vec::new();
    let mut ok = HashSet::new();
    let mut failed = HashSet::new();
    for (o, r) in results {
        match r {
            Ok(s) => {
                ok.insert(o.name.clone());
                stories.extend(s);
            }
            Err(_) => {
                failed.insert(o.name.clone());
            }
        }
    }
    let failed: Vec<String> = failed.difference(&ok).cloned().collect();
    let mut ok: Vec<String> = ok.into_iter().collect();
    ok.sort();
    (stories, ok, failed)
}

const FRONT: usize = 7;
const PER_PAGE: usize = 8;

fn page_for(s: &Story) -> &'static str {
    let has = |t: &str| s.topics.iter().any(|x| x == t);
    match s.section.as_str() {
        "investigations" => return "Investigations",
        "ideas" => return "Ideas & Analysis",
        _ => {}
    }
    if has("sport") {
        "Sport"
    } else if has("health") {
        "Health"
    } else if has("science") || has("technology") || has("ai") || has("climate") || matches!(s.section.as_str(), "science" | "technology") {
        "Science & Technology"
    } else if has("economy") || s.section == "business" {
        "Business"
    } else if has("culture") || has("education") || s.section == "culture" {
        "Culture"
    } else {
        "World"
    }
}

const PAGE_ORDER: &[&str] = &["World", "Business", "Science & Technology", "Health", "Investigations", "Ideas & Analysis", "Culture", "Sport"];

pub struct BuildInput<'a> {
    pub stories: Vec<Story>,
    pub facts: &'a HashMap<String, OutletFacts>,
    pub weights: &'a HashMap<String, f64>,
    pub prefs: &'a Prefs,
    pub now: DateTime<Utc>,
    pub number: u32,
    pub sources_ok: Vec<String>,
    pub sources_failed: Vec<String>,
}

pub fn build(i: BuildInput) -> Edition {
    let cutoff = i.now - Duration::hours(48);
    let mut seen = HashSet::new();
    let stories: Vec<Story> = i
        .stories
        .into_iter()
        .filter(|s| !i.prefs.blocked_outlets.contains(&s.outlet_id))
        .filter(|s| s.published.is_none_or(|p| p >= cutoff && p <= i.now + Duration::hours(2)))
        .filter(|s| seen.insert(s.id.clone()))
        .collect();
    let facts_of = |s: &Story| {
        i.facts.get(&s.outlet_id).cloned().unwrap_or_else(|| sources::outlet(&s.outlet_id).map(crate::wikidata::base_facts).unwrap_or_default())
    };

    let mut items: Vec<(EditionStory, f64, f64)> = Vec::new();
    for members in cluster::cluster(&stories) {
        let group: Vec<&Story> = members.iter().map(|&m| &stories[m]).collect();
        if group.iter().any(|s| s.topics.iter().any(|t| i.prefs.muted_topics.contains(t))) {
            continue;
        }
        let cluster_facts: Vec<OutletFacts> = group.iter().map(|s| facts_of(s)).collect();
        // Lead with the most neutral, best-attributed telling; skip paywalled ones when possible.
        let mut scored: Vec<(&Story, Report)> = group
            .iter()
            .map(|s| {
                let r = bias::assess(bias::Input { title: &s.title, text: &s.summary, url: &s.url, full_text: false, outlet: facts_of(s), cluster: cluster_facts.clone() });
                (*s, r)
            })
            .collect();
        scored.sort_by_key(|(s, r)| (s.paywall, std::cmp::Reverse(r.score), std::cmp::Reverse(s.summary.len())));
        let (lead, report) = scored.remove(0);
        let feats = learner::features(lead);
        let followed = lead.topics.iter().any(|t| i.prefs.followed_topics.contains(t));
        let interest = (learner::predict(i.weights, &feats) + if followed { 0.2 } else { 0.0 }).min(1.0);
        let importance = ((report.corroboration.independent as f64 - 1.0) / 4.0).clamp(0.0, 1.0);
        let mut why = learner::explain(i.weights, &feats);
        if followed {
            why.insert(0, "You follow this topic".into());
        }
        if report.corroboration.independent >= 3 {
            why.push(format!("Covered by {} independent outlets today", report.corroboration.independent));
        }
        let others = scored
            .into_iter()
            .map(|(s, _)| Telling { story_id: s.id.clone(), outlet_name: s.outlet_name.clone(), title: s.title.clone(), url: s.url.clone() })
            .collect();
        let sensitive = topics::is_sensitive(&lead.topics);
        items.push((EditionStory { story: lead.clone(), report, interest, why, others, sensitive }, interest, importance));
    }

    let rank = |it: &(EditionStory, f64, f64)| 0.55 * it.1 + 0.45 * it.2;
    // Sensitive stories need three independent outlets to lead.
    let front_ok = |e: &EditionStory| !(e.sensitive && e.report.corroboration.independent < 3) && !e.report.opinion;

    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| rank(&items[b]).total_cmp(&rank(&items[a])));
    let mut by_importance = order.clone();
    by_importance.sort_by(|&a, &b| items[b].2.total_cmp(&items[a].2));

    let mut front: Vec<usize> = Vec::new();
    let mut topic_count: HashMap<String, usize> = HashMap::new();
    let mut outlet_count: HashMap<String, usize> = HashMap::new();
    // Two slots go to the day's most-covered events whatever your taste, so the
    // paper cannot narrow you into one view. The rest follow your interests.
    let candidates = by_importance.iter().take(2).chain(order.iter()).copied().collect::<Vec<_>>();
    for idx in candidates {
        if front.len() >= FRONT {
            break;
        }
        let e = &items[idx].0;
        let t = e.story.topics.first().cloned().unwrap_or_default();
        if front.contains(&idx) || !front_ok(e) || topic_count.get(&t).copied().unwrap_or(0) >= 2 || outlet_count.get(&e.story.outlet_id).copied().unwrap_or(0) >= 2 {
            continue;
        }
        *topic_count.entry(t).or_default() += 1;
        *outlet_count.entry(e.story.outlet_id.clone()).or_default() += 1;
        front.push(idx);
    }

    let mut pages = vec![Page { title: "Front Page".into(), stories: front.iter().map(|&i| items[i].0.clone()).collect() }];
    let front_set: HashSet<usize> = front.into_iter().collect();
    let mut sections: HashMap<&str, Vec<usize>> = HashMap::new();
    for &idx in &order {
        if !front_set.contains(&idx) {
            sections.entry(page_for(&items[idx].0.story)).or_default().push(idx);
        }
    }
    for name in PAGE_ORDER {
        let Some(list) = sections.get(name) else { continue };
        let max_pages = if *name == "World" { 3 } else { 1 };
        for (n, chunk) in list.chunks(PER_PAGE).take(max_pages).enumerate() {
            let title = if n == 0 { name.to_string() } else { format!("{name}, continued") };
            pages.push(Page { title, stories: chunk.iter().map(|&i| items[i].0.clone()).collect() });
        }
    }
    pages.retain(|p| !p.stories.is_empty());

    Edition {
        date: i.now.format("%Y-%m-%d").to_string(),
        built_at: i.now,
        number: i.number,
        pages,
        sources_ok: i.sources_ok,
        sources_failed: i.sources_failed,
        stories_read: stories.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: &str, outlet: &str, title: &str, topics: &[&str]) -> Story {
        Story {
            id: id.into(),
            outlet_id: outlet.into(),
            outlet_name: outlet.into(),
            section: "world".into(),
            title: title.into(),
            summary: format!("{title}, officials said."),
            url: format!("https://x.org/{id}"),
            published: Some(Utc::now()),
            image: None,
            topics: topics.iter().map(|t| t.to_string()).collect(),
            paywall: false,
        }
    }

    fn input<'a>(stories: Vec<Story>, prefs: &'a Prefs, facts: &'a HashMap<String, OutletFacts>, w: &'a HashMap<String, f64>) -> BuildInput<'a> {
        BuildInput { stories, facts, weights: w, prefs, now: Utc::now(), number: 1, sources_ok: vec![], sources_failed: vec![] }
    }

    #[test]
    fn single_source_sensitive_story_never_leads_and_muted_topics_vanish() {
        let prefs = Prefs { muted_topics: vec!["sport".into()], ..Default::default() };
        let (f, w) = (HashMap::new(), HashMap::new());
        let e = build(input(
            vec![
                s("1", "bbc", "Election result disputed by opposition party", &["politics"]),
                s("2", "dw", "Telescope spots new comet near Jupiter", &["science"]),
                s("3", "cbc", "Cup final ends in penalty shootout", &["sport"]),
            ],
            &prefs,
            &f,
            &w,
        ));
        let front: Vec<_> = e.pages[0].stories.iter().map(|x| x.story.id.as_str()).collect();
        assert_eq!(front, vec!["2"]);
        assert!(e.pages.iter().flat_map(|p| &p.stories).all(|x| x.story.id != "3"));
        assert!(e.pages.iter().flat_map(|p| &p.stories).any(|x| x.story.id == "1"));
    }

    #[test]
    fn corroborated_event_merges_into_one_story_with_other_tellings() {
        let (p, f, w) = (Prefs::default(), HashMap::new(), HashMap::new());
        let e = build(input(
            vec![
                s("1", "bbc", "Earthquake hits northern Japan, tsunami warning issued", &[]),
                s("2", "dw", "Tsunami warning after earthquake hits northern Japan", &[]),
                s("3", "cbc", "Northern Japan earthquake triggers tsunami warning", &[]),
            ],
            &p,
            &f,
            &w,
        ));
        let all: Vec<_> = e.pages.iter().flat_map(|p| &p.stories).collect();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].others.len(), 2);
        assert_eq!(all[0].report.corroboration.independent, 3);
    }

    #[test]
    fn blocked_outlets_are_dropped() {
        let p = Prefs { blocked_outlets: vec!["bbc".into()], ..Default::default() };
        let (f, w) = (HashMap::new(), HashMap::new());
        let e = build(input(vec![s("1", "bbc", "Bridge reopens after repairs", &[])], &p, &f, &w));
        assert!(e.pages.is_empty());
    }
}
