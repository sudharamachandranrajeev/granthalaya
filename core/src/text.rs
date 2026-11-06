//! Plain-text helpers shared by clustering, topics, bias rules and the learner.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

const STOPWORDS: &str = "a about above after again against all am an and any are as at be because been before being \
below between both but by can could did do does doing down during each few for from further had has have having he her \
here hers herself him himself his how i if in into is it its itself just me more most my myself no nor not now of off on \
once only or other our ours ourselves out over own same she should so some such than that the their theirs them \
themselves then there these they this those through to too under until up very was we were what when where which while \
who whom why will with would you your yours yourself yourselves said says say new also one two three after amid over \
year years first last could may might us told get got make made like back still even way week day today yesterday \
tomorrow people according report reports news";

static STOP: LazyLock<std::collections::HashSet<&'static str>> =
    LazyLock::new(|| STOPWORDS.split_whitespace().collect());

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
static BLOCK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</?(p|div|br|li|h[1-6]|blockquote|section|article|tr)[^>]*>").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t\u{a0}]+").unwrap());
static SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(script|style|noscript|figure|figcaption)[^>]*>.*?</(script|style|noscript|figure|figcaption)>").unwrap());

/// Strip HTML to plain text: one string, whitespace collapsed.
pub fn strip_html(html: &str) -> String {
    paragraphs(html).join(" ")
}

/// Strip HTML to a list of plain-text paragraphs. Output is plain text and is
/// rendered as text by the UI, never as HTML.
pub fn paragraphs(html: &str) -> Vec<String> {
    let no_script = SCRIPT.replace_all(html, " ");
    let marked = BLOCK.replace_all(&no_script, "\n");
    let text = TAG.replace_all(&marked, "");
    let decoded = html_escape::decode_html_entities(&text);
    decoded
        .split('\n')
        .map(|p| SPACE.replace_all(p.trim(), " ").to_string())
        .filter(|p| p.chars().count() > 1)
        .collect()
}

/// Lower-case content words, stopwords removed.
pub fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|w| w.trim_matches('\'').to_lowercase())
        .filter(|w| w.chars().count() > 2 && !STOP.contains(w.as_str()) && !w.chars().all(|c| c.is_ascii_digit()))
        .collect()
}

/// Term-frequency vector.
pub fn tf(tokens: &[String]) -> HashMap<String, f64> {
    let mut m = HashMap::new();
    for t in tokens {
        *m.entry(t.clone()).or_insert(0.0) += 1.0;
    }
    m
}

pub fn cosine(a: &HashMap<String, f64>, b: &HashMap<String, f64>) -> f64 {
    let dot: f64 = a.iter().filter_map(|(k, v)| b.get(k).map(|w| v * w)).sum();
    let na: f64 = a.values().map(|v| v * v).sum::<f64>().sqrt();
    let nb: f64 = b.values().map(|v| v * v).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_scripts_and_entities() {
        let p = paragraphs("<p>Hello &amp; <b>world</b></p><script>bad()</script><p>Second&nbsp;line</p>");
        assert_eq!(p, vec!["Hello & world", "Second line"]);
    }

    #[test]
    fn tokens_drop_stopwords_and_numbers() {
        assert_eq!(tokens("The Prime Minister said 2026 elections"), vec!["prime", "minister", "elections"]);
    }

    #[test]
    fn cosine_of_identical_is_one() {
        let a = tf(&tokens("flood rescue villages"));
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-9);
    }
}
