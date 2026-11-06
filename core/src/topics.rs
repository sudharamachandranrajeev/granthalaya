//! Keyword topic tagging. Transparent rules, so the UI can say why a story got a topic.

use crate::text::tokens;

pub struct Topic {
    pub id: &'static str,
    pub label: &'static str,
    /// Sensitive topics need several independent outlets before they can lead a page.
    pub sensitive: bool,
    words: &'static [&'static str],
}

pub static TOPICS: &[Topic] = &[
    Topic {
        id: "politics",
        label: "Politics",
        sensitive: true,
        words: &[
            "election",
            "elections",
            "vote",
            "voters",
            "parliament",
            "senate",
            "congress",
            "minister",
            "president",
            "party",
            "campaign",
            "coalition",
            "referendum",
            "lawmakers",
            "opposition",
            "ballot",
        ],
    },
    Topic {
        id: "conflict",
        label: "Conflict & security",
        sensitive: true,
        words: &[
            "war",
            "military",
            "troops",
            "missile",
            "strike",
            "strikes",
            "attack",
            "ceasefire",
            "army",
            "drone",
            "drones",
            "killed",
            "invasion",
            "hostages",
            "militants",
            "bombing",
            "airstrike",
            "nato",
        ],
    },
    Topic {
        id: "migration",
        label: "Migration",
        sensitive: true,
        words: &["migrants", "migrant", "refugees", "refugee", "asylum", "border", "deportation", "immigration"],
    },
    Topic {
        id: "religion",
        label: "Religion",
        sensitive: true,
        words: &["church", "mosque", "temple", "pope", "religious", "muslim", "christian", "hindu", "jewish", "faith", "vatican"],
    },
    Topic {
        id: "health",
        label: "Health",
        sensitive: true,
        words: &[
            "health", "hospital", "virus", "vaccine", "disease", "cancer", "patients", "doctors", "outbreak", "medical", "drug",
            "pandemic", "nhs",
        ],
    },
    Topic {
        id: "economy",
        label: "Economy & business",
        sensitive: false,
        words: &[
            "economy",
            "inflation",
            "market",
            "markets",
            "stocks",
            "bank",
            "trade",
            "tariffs",
            "tariff",
            "prices",
            "company",
            "companies",
            "shares",
            "investors",
            "gdp",
            "jobs",
            "budget",
            "oil",
        ],
    },
    Topic {
        id: "climate",
        label: "Climate & environment",
        sensitive: false,
        words: &[
            "climate",
            "emissions",
            "carbon",
            "flood",
            "floods",
            "wildfire",
            "heatwave",
            "drought",
            "environment",
            "species",
            "pollution",
            "storm",
            "hurricane",
            "typhoon",
            "cyclone",
        ],
    },
    Topic {
        id: "science",
        label: "Science",
        sensitive: false,
        words: &[
            "scientists",
            "study",
            "research",
            "researchers",
            "space",
            "nasa",
            "planet",
            "physics",
            "fossil",
            "telescope",
            "genetic",
            "species",
            "astronomers",
        ],
    },
    Topic {
        id: "technology",
        label: "Technology",
        sensitive: false,
        words: &[
            "tech",
            "technology",
            "software",
            "app",
            "apps",
            "internet",
            "cyber",
            "hackers",
            "chip",
            "chips",
            "robot",
            "smartphone",
            "google",
            "apple",
            "microsoft",
            "openai",
            "computing",
        ],
    },
    Topic {
        id: "ai",
        label: "Artificial intelligence",
        sensitive: false,
        words: &["ai", "artificial", "chatbot", "machine", "llm", "models"],
    },
    Topic {
        id: "justice",
        label: "Law & justice",
        sensitive: true,
        words: &["court", "judge", "trial", "police", "arrested", "charged", "lawsuit", "prison", "sentenced", "verdict", "supreme"],
    },
    Topic {
        id: "culture",
        label: "Culture & arts",
        sensitive: false,
        words: &[
            "film",
            "music",
            "album",
            "book",
            "novel",
            "art",
            "museum",
            "festival",
            "theatre",
            "actor",
            "singer",
            "artist",
            "television",
            "series",
        ],
    },
    Topic {
        id: "sport",
        label: "Sport",
        sensitive: false,
        words: &[
            "football",
            "cricket",
            "tennis",
            "olympic",
            "olympics",
            "match",
            "league",
            "cup",
            "championship",
            "coach",
            "rugby",
            "goal",
            "tournament",
            "athletes",
        ],
    },
    Topic {
        id: "education",
        label: "Education",
        sensitive: false,
        words: &["school", "schools", "students", "university", "universities", "teachers", "education"],
    },
];

pub fn topic(id: &str) -> Option<&'static Topic> {
    TOPICS.iter().find(|t| t.id == id)
}

/// Topics whose keywords appear in the text, strongest first. A topic needs
/// two hits, or one hit in the headline.
pub fn tag(title: &str, body: &str) -> Vec<&'static str> {
    let head = tokens(title);
    let all: Vec<String> = head.iter().cloned().chain(tokens(body)).collect();
    let mut scored: Vec<(&'static str, usize)> = TOPICS
        .iter()
        .filter_map(|t| {
            let hits = all.iter().filter(|w| t.words.contains(&w.as_str())).count();
            let in_head = head.iter().any(|w| t.words.contains(&w.as_str()));
            (hits >= 2 || in_head).then_some((t.id, hits + if in_head { 2 } else { 0 }))
        })
        .collect();
    scored.sort_by_key(|s| std::cmp::Reverse(s.1));
    scored.into_iter().take(3).map(|(id, _)| id).collect()
}

pub fn is_sensitive(topics: &[String]) -> bool {
    topics.iter().any(|t| topic(t).is_some_and(|x| x.sensitive))
}

#[cfg(test)]
mod tests {
    #[test]
    fn tags_headline_topics() {
        let t = super::tag("Ceasefire talks resume as troops pull back", "Officials said the army moved.");
        assert_eq!(t[0], "conflict");
        assert!(super::tag("Recipe for a quiet Sunday", "").is_empty());
    }
}
