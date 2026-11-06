//! Group stories that report the same event across outlets.

use std::collections::HashMap;

use crate::feeds::Story;
use crate::text::{cosine, tf, tokens};

/// Similarity above which two stories are the same event. Headlines count twice.
const THRESHOLD: f64 = 0.32;

fn vector(s: &Story) -> HashMap<String, f64> {
    let mut t = tokens(&s.title);
    t.extend(tokens(&s.title));
    t.extend(tokens(&s.summary).into_iter().take(40));
    tf(&t)
}

/// Greedy single-pass clustering against each cluster's first (seed) story and
/// running centroid. Returns clusters as lists of indices into `stories`.
pub fn cluster(stories: &[Story]) -> Vec<Vec<usize>> {
    let vecs: Vec<_> = stories.iter().map(vector).collect();
    let mut clusters: Vec<(Vec<usize>, HashMap<String, f64>)> = Vec::new();
    for (i, v) in vecs.iter().enumerate() {
        let best = clusters
            .iter()
            .enumerate()
            .map(|(ci, (members, centroid))| {
                let seed = cosine(v, &vecs[members[0]]);
                (ci, seed.max(cosine(v, centroid)))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((ci, score)) if score >= THRESHOLD => {
                let (members, centroid) = &mut clusters[ci];
                members.push(i);
                for (k, w) in v {
                    *centroid.entry(k.clone()).or_insert(0.0) += w;
                }
            }
            _ => clusters.push((vec![i], v.clone())),
        }
    }
    clusters.into_iter().map(|(m, _)| m).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(outlet: &str, title: &str, summary: &str) -> Story {
        Story {
            id: title.into(),
            outlet_id: outlet.into(),
            outlet_name: outlet.into(),
            section: "world".into(),
            title: title.into(),
            summary: summary.into(),
            url: String::new(),
            published: None,
            image: None,
            topics: vec![],
            paywall: false,
        }
    }

    #[test]
    fn groups_same_event_and_separates_others() {
        let v = vec![
            s("bbc", "Earthquake strikes northern Japan, tsunami warning issued", "A magnitude 7 earthquake hit"),
            s("dw", "Tsunami warning after strong earthquake hits northern Japan", "Japan issued a tsunami warning"),
            s("npr", "Central bank holds interest rates steady", "The bank kept rates unchanged"),
            s("cbc", "Japan earthquake: tsunami warning lifted", "Authorities lifted the tsunami warning after the earthquake"),
        ];
        let c = cluster(&v);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], vec![0, 1, 3]);
        assert_eq!(c[1], vec![2]);
    }
}
