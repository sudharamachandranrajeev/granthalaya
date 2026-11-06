//! Local preference learner: online logistic regression over story features.
//! Runs on the device; nothing leaves it.

use std::collections::HashMap;

use crate::feeds::Story;
use crate::text::tokens;

const LEARNING_RATE: f64 = 0.15;
const L2: f64 = 0.002;

/// Feature names for a story. Prefixes keep kinds apart and readable in "why".
pub fn features(s: &Story) -> Vec<String> {
    let mut f = vec![format!("o:{}", s.outlet_id), format!("s:{}", s.section)];
    f.extend(s.topics.iter().map(|t| format!("t:{t}")));
    let mut words = tokens(&s.title);
    words.extend(tokens(&s.summary).into_iter().take(20));
    words.sort();
    words.dedup();
    f.extend(words.into_iter().take(25).map(|w| format!("w:{w}")));
    f
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Predicted interest 0..1. Unknown stories sit at 0.5.
pub fn predict(weights: &HashMap<String, f64>, feats: &[String]) -> f64 {
    let z: f64 = feats.iter().filter_map(|f| weights.get(f)).sum::<f64>() + weights.get("bias").copied().unwrap_or(0.0);
    sigmoid(z)
}

/// Turn a vote and/or star rating into a 0..1 target. Stars win when given.
pub fn target(vote: Option<i8>, stars: Option<u8>) -> Option<f64> {
    match (vote, stars) {
        (_, Some(s)) => Some((s.clamp(1, 10) as f64 - 1.0) / 9.0),
        (Some(v), None) if v > 0 => Some(1.0),
        (Some(v), None) if v < 0 => Some(0.0),
        _ => None,
    }
}

/// One SGD step. Returns the changed weights so the caller can persist them.
pub fn update(weights: &mut HashMap<String, f64>, feats: &[String], y: f64) -> Vec<(String, f64)> {
    let p = predict(weights, feats);
    let g = y - p;
    let mut changed = Vec::new();
    for f in feats.iter().map(String::as_str).chain(std::iter::once("bias")) {
        let w = weights.entry(f.to_string()).or_insert(0.0);
        *w += LEARNING_RATE * (g - L2 * *w);
        changed.push((f.to_string(), *w));
    }
    changed
}

/// The strongest reasons behind a prediction, in plain words.
pub fn explain(weights: &HashMap<String, f64>, feats: &[String]) -> Vec<String> {
    let mut v: Vec<(&String, f64)> = feats.iter().filter_map(|f| weights.get(f).map(|w| (f, *w))).filter(|(_, w)| w.abs() > 0.05).collect();
    v.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
    v.into_iter()
        .take(3)
        .map(|(f, w)| {
            let (kind, name) = f.split_once(':').unwrap_or(("", f));
            let what = match kind {
                "o" => format!("stories from {}", crate::sources::outlet(name).map(|o| o.name.as_str()).unwrap_or(name)),
                "t" => format!("{} stories", crate::topics::topic(name).map(|t| t.label).unwrap_or(name)),
                "s" => format!("the {name} section"),
                _ => format!("stories mentioning \"{name}\""),
            };
            if w > 0.0 { format!("You rate {what} highly") } else { format!("You tend to skip {what}") }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn story(topic: &str, title: &str) -> Story {
        Story {
            id: title.into(),
            outlet_id: "bbc".into(),
            outlet_name: "BBC".into(),
            section: "world".into(),
            title: title.into(),
            summary: String::new(),
            url: String::new(),
            published: None,
            image: None,
            topics: vec![topic.into()],
            paywall: false,
        }
    }

    #[test]
    fn learns_to_prefer_liked_topic() {
        let mut w = HashMap::new();
        let liked = story("science", "Telescope finds water on distant planet");
        let disliked = story("sport", "League match ends in draw");
        for _ in 0..60 {
            update(&mut w, &features(&liked), 1.0);
            update(&mut w, &features(&disliked), 0.0);
        }
        let new_sci = features(&story("science", "Astronomers map galaxy"));
        let new_sport = features(&story("sport", "Tennis final tonight"));
        assert!(predict(&w, &new_sci) > 0.6);
        assert!(predict(&w, &new_sport) < 0.4);
        assert!(explain(&w, &new_sci)[0].contains("Science"));
    }

    #[test]
    fn stars_map_to_targets() {
        assert_eq!(target(Some(1), None), Some(1.0));
        assert_eq!(target(Some(-1), Some(10)), Some(1.0));
        assert_eq!(target(None, Some(1)), Some(0.0));
        assert_eq!(target(None, None), None);
    }
}
