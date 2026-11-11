//! Fetch and parse RSS/Atom feeds into stories.

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use reqwest::Client;

use crate::sources::{Feed, Outlet};
use crate::{text, topics};

/// Wikidata and some feeds require a contact in the User-Agent; this names the
/// project, never the user.
/// Plain product token. Some newsrooms (CBC) stall requests whose agent carries a URL.
pub const USER_AGENT: &str = "Granthalaya/0.1";
/// Wikimedia asks automated clients to name a contact; used for Wikidata only.
pub const CONTACT_USER_AGENT: &str = "Granthalaya/0.1 (https://github.com/sudharamachandranrajeev/granthalaya-retro-reader)";

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .http1_only()
        .build()
        .expect("http client")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Story {
    /// Stable id: hash of the canonical URL.
    pub id: String,
    pub outlet_id: String,
    pub outlet_name: String,
    pub section: String,
    pub title: String,
    pub summary: String,
    pub url: String,
    pub published: Option<DateTime<Utc>>,
    pub image: Option<String>,
    pub topics: Vec<String>,
    pub paywall: bool,
}

pub fn story_id(url: &str) -> String {
    // FNV-1a, 64-bit: stable across runs and platforms.
    let canonical = url.split(['?', '#']).next().unwrap_or(url).trim_end_matches('/');
    let mut h: u64 = 0xcbf29ce484222325;
    for b in canonical.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Parse one feed body. Kept separate from the network so it is testable.
pub fn parse(outlet: &Outlet, feed: &Feed, body: &[u8]) -> Result<Vec<Story>> {
    let parsed = feed_rs::parser::parse(body).context("feed did not parse")?;
    let mut out = Vec::new();
    for e in parsed.entries {
        let Some(link) = e.links.iter().find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate")).or(e.links.first()) else {
            continue;
        };
        let url = link.href.trim().to_string();
        if !url.starts_with("https://") && !url.starts_with("http://") {
            continue;
        }
        let title = e.title.map(|t| text::strip_html(&t.content)).unwrap_or_default();
        // Programme pages ("Tech Life") and stubs are not news stories.
        if title.split_whitespace().count() < 4 || out.iter().any(|s: &Story| s.title == title) {
            continue;
        }
        let summary_html = e.summary.map(|s| s.content).or_else(|| e.content.and_then(|c| c.body)).unwrap_or_default();
        let mut summary = text::strip_html(&summary_html);
        if summary.chars().count() > 600 {
            summary = summary.chars().take(600).collect::<String>() + "…";
        }
        let image = e
            .media
            .iter()
            .flat_map(|m| {
                m.thumbnails
                    .iter()
                    .map(|t| t.image.uri.clone())
                    .chain(m.content.iter().filter_map(|c| c.url.as_ref().map(|u| u.to_string())))
            })
            .find(|u| u.starts_with("https://"));
        let topics = topics::tag(&title, &summary).into_iter().map(String::from).collect();
        out.push(Story {
            id: story_id(&url),
            outlet_id: outlet.id.clone(),
            outlet_name: outlet.name.clone(),
            section: feed.section.clone(),
            title,
            summary,
            url,
            published: e.published.or(e.updated),
            image,
            topics,
            paywall: outlet.paywall,
        });
    }
    Ok(out)
}

/// Fetch one feed, retrying once after a short pause.
pub async fn fetch(client: &reqwest::Client, outlet: &Outlet, feed: &Feed) -> Result<Vec<Story>> {
    match fetch_once(client, outlet, feed).await {
        Ok(s) => Ok(s),
        Err(_) => {
            tokio::time::sleep(Duration::from_millis(800)).await;
            fetch_once(client, outlet, feed).await
        }
    }
}

async fn fetch_once(client: &reqwest::Client, outlet: &Outlet, feed: &Feed) -> Result<Vec<Story>> {
    let body =
        client.get(&feed.url).send().await.with_context(|| format!("{}: request failed", outlet.name))?.error_for_status()?.bytes().await?;
    parse(outlet, feed, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>T</title>
      <item><title>Flood waters rise in river towns</title><link>https://example.org/a?x=1</link>
      <description>&lt;p&gt;Rescue teams moved &lt;b&gt;hundreds&lt;/b&gt; of people.&lt;/p&gt;</description>
      <pubDate>Fri, 06 Nov 2026 08:00:00 GMT</pubDate></item>
      <item><title></title><link>https://example.org/b</link></item><item><title>Tech Life</title><link>https://example.org/c</link></item><item><title>Flood waters rise in river towns</title><link>https://example.org/d</link></item>
      <item><title>No link</title></item>
    </channel></rss>"#;

    #[test]
    fn parses_items_strips_html_and_skips_bad_entries() {
        let o = crate::sources::outlet("bbc").unwrap();
        let s = parse(o, &o.feeds[0], RSS.as_bytes()).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].summary, "Rescue teams moved hundreds of people.");
        assert_eq!(s[0].id, story_id("https://example.org/a"));
        assert!(s[0].published.is_some());
        assert!(s[0].topics.contains(&"climate".to_string()));
    }
}
