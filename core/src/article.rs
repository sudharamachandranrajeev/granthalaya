//! Full text of a story from its public page. Paywalled pages are left alone.

use anyhow::{Context, Result, bail};

use crate::text;

/// Plain-text paragraphs of the main article on a page.
pub fn extract(html: &str, url: &str) -> Result<Vec<String>> {
    let mut r = dom_smoothie::Readability::new(html, Some(url), None).context("page did not parse")?;
    let article = r.parse().context("no article found on page")?;
    let paras: Vec<String> = text::paragraphs(&article.content).into_iter().filter(|p| p.split_whitespace().count() >= 4).collect();
    if paras.is_empty() {
        bail!("no article text found");
    }
    Ok(paras)
}

pub async fn fetch(client: &reqwest::Client, url: &str) -> Result<Vec<String>> {
    let html = client.get(url).send().await?.error_for_status()?.text().await?;
    extract(&html, url)
}

#[cfg(test)]
mod tests {
    #[test]
    fn extracts_main_text_without_navigation() {
        let body = "<p>The council voted on Tuesday to rebuild the old bridge across the river after years of delay.</p>".repeat(6);
        let html = format!("<html><head><title>Bridge</title></head><body><nav><a href='/'>Home</a> <a href='/x'>Sport</a></nav><article><h1>Bridge to be rebuilt</h1>{body}</article><footer>Copyright</footer></body></html>");
        let p = super::extract(&html, "https://example.org/bridge").unwrap();
        assert!(p.iter().any(|x| x.starts_with("The council voted")), "{p:?}");
        assert!(p.iter().all(|x| !x.contains("Copyright")));
    }
}
