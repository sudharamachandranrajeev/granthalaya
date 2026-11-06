//! The bias meter: an evidence-first genuineness score for one story.
//!
//! Every point in the score comes from a rule the reader can see: how many
//! independent outlets carry the event, how claims are attributed, how loaded
//! the wording is, how open the outlet is about who owns and funds it, and
//! whether anyone in the story owns or funds the outlet. No left/right label.

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::text::word_count;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Link {
    pub label: String,
    pub url: Option<String>,
}

/// What is known about the outlet behind a story.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OutletFacts {
    pub id: String,
    pub name: String,
    pub country: String,
    pub funding: String,
    pub funding_text: String,
    pub owners: Vec<Link>,
    pub funders: Vec<Link>,
    pub wikidata: Option<String>,
}

impl OutletFacts {
    /// Outlets sharing this key are not independent of each other.
    pub fn independence_key(&self) -> String {
        if let Some(o) = self.owners.first() {
            return format!("owner:{}", o.label.to_lowercase());
        }
        if self.funding == "state" {
            return format!("gov:{}", self.country.to_lowercase());
        }
        format!("outlet:{}", self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Actor {
    pub name: String,
    /// person, party, company, organisation or government.
    pub kind: String,
    pub mentions: usize,
    pub quotes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub id: String,
    pub label: String,
    pub points: u32,
    pub max: u32,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Influence {
    pub who: String,
    pub how: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Corroboration {
    /// Names of every outlet carrying the same event.
    pub outlets: Vec<String>,
    pub independent: usize,
    pub countries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Report {
    pub score: u32,
    pub verdict: String,
    pub basis: String,
    pub components: Vec<Component>,
    pub loaded: Vec<String>,
    pub unattributed: Vec<String>,
    pub actors: Vec<Actor>,
    pub influences: Vec<Influence>,
    pub corroboration: Corroboration,
    pub opinion: bool,
    pub outlet: OutletFacts,
}

/// Emotive or judgement words that a straight news report usually avoids.
const LOADED: &[&str] = &[
    "slammed", "slams", "blasted", "blasts", "ripped", "rips", "destroyed", "destroys", "obliterated", "eviscerated",
    "outrage", "outrageous", "outraged", "fury", "furious", "shocking", "shocked", "stunning", "stunned", "horrific",
    "disgraceful", "disgrace", "shameful", "radical", "extremist", "extremists", "regime", "thugs", "thug",
    "terrorists", "terror", "propaganda", "lies", "liar", "lied", "hoax", "scandal", "scandalous", "chaos", "chaotic",
    "catastrophic", "catastrophe", "disaster", "disastrous", "crisis", "meltdown", "bombshell", "explosive",
    "brutal", "savage", "evil", "heroic", "hero", "villain", "draconian", "tyranny", "tyrant", "dictator",
    "massacre", "slaughter", "genocide", "invasion", "illegal", "illegals", "woke", "elite", "elites", "puppet",
    "mob", "rogue", "sham", "witch-hunt", "crackdown", "backlash", "spree", "embattled", "beleaguered",
    "controversial", "notorious", "infamous", "devastating", "alarming", "ominous", "unprecedented",
];

/// Phrases that make a claim without saying who made it.
const UNATTRIBUTED: &[&str] = &[
    "reportedly", "it is believed", "it is understood", "sources say", "sources said", "a source said",
    "some say", "many say", "critics say", "experts say", "observers say", "it has been claimed", "allegedly",
    "rumoured", "rumored", "is said to", "are said to", "people familiar", "unnamed",
];

static ATTRIBUTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(said|says|told|according to|stated|announced|confirmed|wrote|testified|reported by)\b").unwrap()
});

static QUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[“"]([^”"]{12,})[”"]"#).unwrap());

static SPEAKER_AFTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:said|says|told reporters|according to|wrote|added|warned)\s+([A-Z][a-z]+(?:\s+[A-Z][a-zA-Z'-]+){1,2})").unwrap()
});
static SPEAKER_BEFORE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([A-Z][a-z]+(?:\s+[A-Z][a-zA-Z'-]+){1,2}),?\s+(?:said|says|told|added|argued|warned|claimed|wrote)\b").unwrap()
});
static TITLED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:President|Prime Minister|Minister|Chancellor|Senator|Governor|Mayor|Dr|Mr|Mrs|Ms|King|Queen|General|Secretary|Judge|Pope|Prince|Princess|Ambassador|Chief Executive|CEO)\.?\s+([A-Z][a-z]+(?:\s+[A-Z][a-zA-Z'-]+)?)").unwrap()
});

const COUNTRIES: &[&str] = &[
    "Afghanistan", "Albania", "Algeria", "Argentina", "Armenia", "Australia", "Austria", "Azerbaijan", "Bangladesh",
    "Belarus", "Belgium", "Bolivia", "Bosnia", "Brazil", "Bulgaria", "Cambodia", "Cameroon", "Canada", "Chile", "China",
    "Colombia", "Congo", "Croatia", "Cuba", "Cyprus", "Czech Republic", "Denmark", "Ecuador", "Egypt", "Estonia",
    "Ethiopia", "Finland", "France", "Georgia", "Germany", "Ghana", "Greece", "Guatemala", "Haiti", "Hong Kong",
    "Hungary", "India", "Indonesia", "Iran", "Iraq", "Ireland", "Israel", "Italy", "Japan", "Jordan", "Kazakhstan",
    "Kenya", "Kuwait", "Latvia", "Lebanon", "Libya", "Lithuania", "Malaysia", "Mali", "Mexico", "Moldova", "Mongolia",
    "Morocco", "Mozambique", "Myanmar", "Nepal", "Netherlands", "New Zealand", "Nicaragua", "Niger", "Nigeria",
    "North Korea", "Norway", "Pakistan", "Palestine", "Panama", "Peru", "Philippines", "Poland", "Portugal", "Qatar",
    "Romania", "Russia", "Rwanda", "Saudi Arabia", "Senegal", "Serbia", "Singapore", "Slovakia", "Somalia",
    "South Africa", "South Korea", "South Sudan", "Spain", "Sri Lanka", "Sudan", "Sweden", "Switzerland", "Syria",
    "Taiwan", "Tanzania", "Thailand", "Tunisia", "Turkey", "Uganda", "Ukraine", "United Arab Emirates",
    "United Kingdom", "United States", "Uruguay", "Uzbekistan", "Venezuela", "Vietnam", "Yemen", "Zambia", "Zimbabwe",
    "UK", "US", "Britain", "America", "Gaza", "Kremlin", "Beijing", "Washington", "Moscow", "Tehran",
];

/// Capital-city and building names used as shorthand for a government.
fn country_alias(name: &str) -> &str {
    match name {
        "UK" | "Britain" => "United Kingdom",
        "US" | "America" | "Washington" => "United States",
        "Kremlin" | "Moscow" => "Russia",
        "Beijing" => "China",
        "Tehran" => "Iran",
        other => other,
    }
}

const GOV_WORDS: &[&str] = &[
    "government", "ministry", "minister", "president", "prime minister", "officials", "state", "authorities",
    "administration", "cabinet", "parliament", "embassy", "foreign office", "military", "army", "police",
];

const PARTIES: &[&str] = &[
    "Democratic Party", "Democrats", "Republican Party", "Republicans", "GOP", "Labour Party", "Labour",
    "Conservative Party", "Conservatives", "Tories", "Liberal Democrats", "Reform UK", "SNP", "BJP",
    "Bharatiya Janata Party", "Congress party", "Indian National Congress", "Communist Party", "CCP", "CDU", "SPD",
    "AfD", "Greens", "Green Party", "National Rally", "Likud", "Hamas", "Hezbollah", "ANC", "PTI", "PML-N",
    "Liberal Party", "Bloc Québécois", "New Democratic Party", "Fidesz", "Law and Justice", "Sinn Féin", "Fianna Fáil",
    "Fine Gael", "United Russia", "Morena", "Workers' Party", "Peronists", "ruling party", "opposition party",
];

const ORGS: &[&str] = &[
    "United Nations", "UN", "NATO", "European Union", "EU", "World Health Organization", "WHO", "IMF",
    "International Monetary Fund", "World Bank", "OPEC", "African Union", "ASEAN", "G7", "G20", "BRICS", "Red Cross",
    "Amnesty International", "Human Rights Watch", "Greenpeace", "UNICEF", "UNHCR", "ICC", "International Criminal Court",
    "WTO", "Federal Reserve", "European Central Bank", "Bank of England", "Interpol", "FIFA", "IOC", "Oxfam",
    "Doctors Without Borders", "MSF", "Pentagon", "FBI", "CIA",
];

const COMPANIES: &[&str] = &[
    "Apple", "Google", "Alphabet", "Microsoft", "Amazon", "Meta", "Facebook", "Instagram", "WhatsApp", "TikTok",
    "ByteDance", "Tesla", "SpaceX", "X Corp", "OpenAI", "Anthropic", "Nvidia", "Intel", "AMD", "TSMC", "Samsung",
    "Huawei", "Tencent", "Alibaba", "Sony", "Toyota", "Volkswagen", "BMW", "Boeing", "Airbus", "Shell", "BP",
    "ExxonMobil", "Chevron", "Saudi Aramco", "Gazprom", "Rosneft", "Pfizer", "Moderna", "AstraZeneca", "Novartis",
    "JPMorgan", "Goldman Sachs", "BlackRock", "HSBC", "Barclays", "Reliance", "Tata", "Adani", "Infosys", "Netflix",
    "Disney", "Uber", "Walmart", "Nestlé", "Unilever", "Glencore", "Rio Tinto", "BHP", "Lockheed Martin", "Raytheon",
];

static COMPANY_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([A-Z][A-Za-z&]+(?:\s+[A-Z][A-Za-z&]+){0,2})\s+(Inc|Corp|Corporation|Ltd|Limited|plc|Group|Holdings|AG|SA)\b").unwrap()
});

fn word_re(needle: &str) -> Regex {
    Regex::new(&format!(r"\b{}\b", regex::escape(needle))).unwrap()
}

fn compile(list: &'static [&'static str]) -> Vec<(&'static str, Regex)> {
    list.iter().map(|w| (*w, word_re(w))).collect()
}

static RE_LOADED: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| compile(LOADED));
static RE_UNATTRIBUTED: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| compile(UNATTRIBUTED));
static RE_COUNTRIES: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| compile(COUNTRIES));
static RE_PARTIES: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| compile(PARTIES));
static RE_ORGS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| compile(ORGS));
static RE_COMPANIES: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| compile(COMPANIES));

fn contains_word(hay: &str, needle: &str) -> usize {
    word_re(needle).find_iter(hay).count()
}

fn sentences(text: &str) -> Vec<&str> {
    text.split_inclusive(['.', '!', '?', '\n']).map(str::trim).filter(|s| !s.is_empty()).collect()
}

/// Named people, parties, companies, organisations and governments, found by
/// text rules (no model). Quotes are counted per sentence a name appears in.
pub fn actors(text: &str) -> Vec<Actor> {
    let lower = text.to_lowercase();
    let mut found: BTreeMap<String, Actor> = BTreeMap::new();
    fn add(found: &mut BTreeMap<String, Actor>, name: &str, kind: &str, mentions: usize) {
        if mentions == 0 {
            return;
        }
        let e = found.entry(name.to_string()).or_insert(Actor { name: name.into(), kind: kind.into(), mentions: 0, quotes: 0 });
        e.mentions = e.mentions.max(mentions);
    }
    let gov_context = GOV_WORDS.iter().any(|w| lower.contains(w));
    for (c, re) in RE_COUNTRIES.iter() {
        let n = re.find_iter(text).count();
        if n > 0 && gov_context {
            add(&mut found, &format!("Government of {}", country_alias(c)), "government", n);
        }
    }
    for (kind, list) in [("party", &*RE_PARTIES), ("organisation", &*RE_ORGS), ("company", &*RE_COMPANIES)] {
        for (name, re) in list {
            add(&mut found, name, kind, re.find_iter(text).count());
        }
    }
    for cap in COMPANY_SUFFIX.captures_iter(text) {
        add(&mut found, cap.get(0).unwrap().as_str(), "company", 1);
    }
    let known: HashSet<String> = found.keys().map(|k| k.to_lowercase()).collect();
    let mut people: Vec<String> = Vec::new();
    for re in [&*SPEAKER_AFTER, &*SPEAKER_BEFORE, &*TITLED] {
        for cap in re.captures_iter(text) {
            let name = cap[1].trim().to_string();
            let first = name.split_whitespace().next().unwrap_or("");
            let is_place = COUNTRIES.iter().any(|c| name.contains(c));
            if !known.contains(&name.to_lowercase()) && !is_place && !["The", "A", "An", "But", "And", "In", "On", "He", "She", "They", "It"].contains(&first) {
                people.push(name);
            }
        }
    }
    // "Justice Diana Kavedza" and later "Judge Kavedza" are one person: drop
    // titles, then fold a lone surname into the full name that ends with it.
    let people: Vec<String> = people.into_iter().map(|p| strip_title(&p)).filter(|p| !p.is_empty()).collect();
    let mut merged: Vec<String> = Vec::new();
    for p in &people {
        let lone = !p.contains(' ');
        let fuller = people.iter().find(|q| q.len() > p.len() && q.rsplit(' ').next() == Some(p.as_str()));
        let name = if lone { fuller.unwrap_or(p) } else { p };
        if !merged.contains(name) {
            merged.push(name.clone());
        }
    }
    for p in merged {
        // Count surname mentions too, so "Kavedza" later in the text counts for her.
        let surname = p.rsplit(' ').next().unwrap_or(&p);
        let n = contains_word(text, &p).max(contains_word(text, surname)).max(1);
        add(&mut found, &p, "person", n);
    }
    // Quotes: a name in the same sentence as a quoted passage gets the quote.
    for s in sentences(text) {
        if QUOTE.is_match(s) {
            for a in found.values_mut() {
                let key = a.name.strip_prefix("Government of ").unwrap_or(&a.name);
                // People are often quoted by surname alone after the first mention.
                let key = if a.kind == "person" { key.rsplit(' ').next().unwrap_or(key) } else { key };
                if s.contains(key) {
                    a.quotes += QUOTE.find_iter(s).count();
                }
            }
        }
    }
    let mut v: Vec<Actor> = found.into_values().collect();
    v.sort_by_key(|a| std::cmp::Reverse((a.quotes, a.mentions)));
    v.truncate(12);
    v
}

const TITLES: &[&str] = &[
    "Prime Minister", "Chief Executive", "President", "Minister", "Chancellor", "Senator", "Governor", "Mayor", "Justice",
    "Judge", "General", "Secretary", "Ambassador", "King", "Queen", "Pope", "Prince", "Princess", "Dr", "Mr", "Mrs", "Ms", "CEO",
];

/// The name after the last title word: "Later Judge Kavedza" gives "Kavedza".
fn strip_title(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().map(|w| w.trim_end_matches('.')).collect();
    let mut start = 0;
    for i in 0..words.len() {
        let two = words.get(i + 1).map(|n| format!("{} {n}", words[i]));
        if two.as_deref().is_some_and(|t| TITLES.contains(&t)) {
            start = i + 2;
        } else if TITLES.contains(&words[i]) {
            start = i + 1;
        }
    }
    words[start.min(words.len())..].join(" ")
}

fn hits(text_lower: &str, list: &[(&str, Regex)]) -> Vec<String> {
    list.iter().filter(|(_, re)| re.is_match(text_lower)).map(|(w, _)| w.to_string()).collect()
}

pub fn is_opinion(url: &str, title: &str) -> bool {
    let u = url.to_lowercase();
    let t = title.to_lowercase();
    ["/opinion", "/comment", "/commentisfree", "/editorial", "/analysis", "/column", "/blogs/", "/perspective"]
        .iter()
        .any(|p| u.contains(p))
        || t.starts_with("opinion")
        || t.starts_with("analysis:")
        || t.starts_with("comment:")
}

pub struct Input<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub url: &'a str,
    pub full_text: bool,
    pub outlet: OutletFacts,
    /// Every outlet carrying the same event, including this one.
    pub cluster: Vec<OutletFacts>,
}

pub fn assess(i: Input) -> Report {
    let all = format!("{}. {}", i.title, i.text);
    let lower = all.to_lowercase();
    let words = word_count(&all).max(1);

    // 1. Corroboration.
    let keys: HashSet<String> = i.cluster.iter().map(|o| o.independence_key()).collect();
    let countries: HashSet<&str> = i.cluster.iter().map(|o| o.country.as_str()).collect();
    let independent = keys.len().max(1);
    let corro_pts = match independent { 1 => 6, 2 => 15, 3 => 22, 4 => 26, _ => 30 };
    let mut names: Vec<String> = i.cluster.iter().map(|o| o.name.clone()).collect();
    names.sort();
    names.dedup();
    let corro_detail = if independent == 1 {
        "Only this outlet (or outlets with the same owner) is carrying this story right now.".to_string()
    } else {
        format!("{independent} independent outlets in {} countries carry this story: {}.", countries.len(), names.join(", "))
    };

    // 2. Attribution.
    let attributions = ATTRIBUTION.find_iter(&all).count();
    let unattributed = hits(&lower, &RE_UNATTRIBUTED);
    let per100 = attributions as f64 * 100.0 / words as f64;
    let base = if i.full_text { (per100 * 6.0).min(20.0) } else { 8.0 + attributions as f64 * 4.0 };
    let attr_pts = (base - unattributed.len() as f64 * 4.0).clamp(0.0, 20.0).round() as u32;
    let attr_detail = format!(
        "{attributions} attributed statement(s){}{}.",
        if unattributed.is_empty() { String::new() } else { format!("; unattributed claims: {}", unattributed.join(", ")) },
        if i.full_text { "" } else { " (headline and summary only; open the story for the full check)" }
    );

    // 3. Wording.
    let loaded = hits(&lower, &RE_LOADED);
    let title_lower = i.title.to_lowercase();
    let head_loaded = loaded.iter().filter(|w| contains_word(&title_lower, w) > 0).count();
    let weight = (loaded.len() + head_loaded) as f64;
    let density_penalty = if i.full_text { weight * 300.0 / words as f64 } else { weight * 5.0 };
    let lang_pts = (25.0 - density_penalty).clamp(0.0, 25.0).round() as u32;
    let lang_detail = if loaded.is_empty() {
        "No emotive or judgement words found.".to_string()
    } else {
        format!("Loaded words: {}{}.", loaded.join(", "), if head_loaded > 0 { " (some in the headline, counted twice)" } else { "" })
    };

    // 4. Transparency of ownership.
    let (trans_pts, trans_detail) = if !i.outlet.owners.is_empty() || !i.outlet.funders.is_empty() {
        (10, "Owner or funder is on public record (Wikidata).".to_string())
    } else if i.outlet.wikidata.is_some() {
        (6, "Funding model known; Wikidata lists no owner or funder yet.".to_string())
    } else {
        (3, "Funding model known; no public ownership record found.".to_string())
    };

    // 5. Independence from the people in the story.
    let actors = actors(&all);
    let mut influences = Vec::new();
    let own_gov = format!("Government of {}", i.outlet.country);
    for a in &actors {
        let a_low = a.name.to_lowercase();
        for o in i.outlet.owners.iter().chain(i.outlet.funders.iter()) {
            let o_low = o.label.to_lowercase();
            if o_low.len() > 2 && (a_low.contains(&o_low) || o_low.contains(&a_low)) {
                influences.push(Influence { who: a.name.clone(), how: format!("{} owns or funds {}, which published this story.", o.label, i.outlet.name) });
            }
        }
        if a.name == own_gov && (i.outlet.funding == "state" || i.outlet.funding == "public") {
            let how = if i.outlet.funding == "state" {
                format!("{} is funded by this government and is reporting on it.", i.outlet.name)
            } else {
                format!("{} is publicly funded in {}; its charter sets editorial independence, but it is reporting on its own government.", i.outlet.name, i.outlet.country)
            };
            influences.push(Influence { who: a.name.clone(), how });
        }
    }
    let quoted: Vec<&Actor> = actors.iter().filter(|a| a.quotes > 0).collect();
    if quoted.len() == 1 && quoted[0].quotes >= 2 {
        influences.push(Influence { who: quoted[0].name.clone(), how: "The only voice quoted in the story, so it frames the account.".into() });
    }
    let state_conflict = influences.iter().any(|x| x.how.contains("is funded by this government"));
    let owner_conflict = influences.iter().any(|x| x.how.contains("owns or funds"));
    let indep_pts = if owner_conflict || state_conflict { 5 } else if !influences.is_empty() { 10 } else { 15 };
    let indep_detail = if influences.is_empty() {
        "No one in the story owns or funds the outlet.".to_string()
    } else {
        influences.iter().map(|x| x.how.clone()).collect::<Vec<_>>().join(" ")
    };

    let components = vec![
        Component { id: "corroboration".into(), label: "Corroboration".into(), points: corro_pts, max: 30, detail: corro_detail },
        Component { id: "wording".into(), label: "Neutral wording".into(), points: lang_pts, max: 25, detail: lang_detail },
        Component { id: "attribution".into(), label: "Attributed claims".into(), points: attr_pts, max: 20, detail: attr_detail },
        Component { id: "independence".into(), label: "Independence".into(), points: indep_pts, max: 15, detail: indep_detail },
        Component { id: "transparency".into(), label: "Ownership on record".into(), points: trans_pts, max: 10, detail: trans_detail },
    ];
    let mut score: u32 = components.iter().map(|c| c.points).sum();
    let opinion = is_opinion(i.url, i.title);
    if opinion {
        score = score.min(60);
    }
    let verdict = match score {
        80.. => "Well sourced",
        60..80 => "Mostly solid",
        40..60 => "Read with care",
        _ => "Weak sourcing",
    }
    .to_string();
    Report {
        score,
        verdict: if opinion { format!("{verdict} · opinion or analysis") } else { verdict },
        basis: if i.full_text { "Full article".into() } else { "Headline and summary".into() },
        components,
        loaded,
        unattributed,
        actors,
        influences,
        corroboration: Corroboration { outlets: names, independent, countries: countries.len() },
        opinion,
        outlet: i.outlet,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(id: &str, country: &str, funding: &str, owner: Option<&str>) -> OutletFacts {
        OutletFacts {
            id: id.into(),
            name: id.to_uppercase(),
            country: country.into(),
            funding: funding.into(),
            owners: owner.map(|o| vec![Link { label: o.into(), url: None }]).unwrap_or_default(),
            ..Default::default()
        }
    }

    #[test]
    fn finds_people_parties_governments_and_quotes() {
        let a = actors("Prime Minister Jane Okafor said “we will not back down on this plan” as the Labour Party met. Nigeria government officials agreed.");
        let names: Vec<_> = a.iter().map(|x| (x.name.as_str(), x.kind.as_str())).collect();
        assert!(names.contains(&("Jane Okafor", "person")), "{names:?}");
        assert!(names.contains(&("Labour Party", "party")));
        assert!(names.contains(&("Government of Nigeria", "government")));
        assert_eq!(a.iter().find(|x| x.name == "Jane Okafor").unwrap().quotes, 1);
    }

    #[test]
    fn folds_titles_and_surnames_into_one_person() {
        let a = actors("Justice Diana Kavedza said the sentence was final. Later Judge Kavedza said “there is no appeal”.");
        let people: Vec<_> = a.iter().filter(|x| x.kind == "person").collect();
        assert_eq!(people.len(), 1, "{people:?}");
        assert_eq!(people[0].name, "Diana Kavedza");
        assert_eq!(people[0].mentions, 2);
        assert_eq!(people[0].quotes, 1);
    }

    #[test]
    fn corroborated_neutral_story_scores_high() {
        let o = facts("a", "Canada", "public", Some("CBC"));
        let r = assess(Input {
            title: "Central bank holds interest rates",
            text: "The bank said rates would stay at 3%. Its governor told reporters inflation was easing, according to the statement.",
            url: "https://x.org/news/1",
            full_text: false,
            outlet: o.clone(),
            cluster: vec![o, facts("b", "UK", "public", Some("BBC")), facts("c", "Germany", "state", None), facts("d", "Japan", "commercial", Some("Nikkei"))],
        });
        assert!(r.score >= 80, "{r:?}");
        assert!(r.loaded.is_empty());
        assert_eq!(r.corroboration.independent, 4);
    }

    #[test]
    fn loaded_single_source_state_story_on_own_government_scores_low() {
        let o = facts("s", "Qatar", "state", None);
        let r = assess(Input {
            title: "Shocking regime crackdown sparks outrage",
            text: "Critics say the Qatar government ministry is in chaos, reportedly.",
            url: "https://x.org/a",
            full_text: false,
            outlet: o.clone(),
            cluster: vec![o],
        });
        assert!(r.score < 40, "{r:?}");
        assert!(r.influences.iter().any(|i| i.who == "Government of Qatar"));
        assert!(r.unattributed.contains(&"critics say".to_string()));
    }

    #[test]
    fn same_owner_outlets_do_not_count_as_independent() {
        let a = facts("f24", "France", "state", Some("France Médias Monde"));
        let b = facts("rfi", "France", "state", Some("France Médias Monde"));
        let r = assess(Input { title: "T", text: "", url: "", full_text: false, outlet: a.clone(), cluster: vec![a, b] });
        assert_eq!(r.corroboration.independent, 1);
    }

    #[test]
    fn opinion_is_capped_and_labelled() {
        let o = facts("g", "UK", "commercial", Some("Scott Trust"));
        let r = assess(Input { title: "Why rates must fall", text: "", url: "https://g.com/commentisfree/x", full_text: false, outlet: o.clone(), cluster: vec![o; 1] });
        assert!(r.opinion && r.score <= 60 && r.verdict.contains("opinion"));
    }
}
