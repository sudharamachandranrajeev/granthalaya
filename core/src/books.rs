//! Open book catalogues (Project Gutenberg via Gutendex, Standard Ebooks) and
//! taste-based suggestions.

use std::collections::HashSet;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Book {
    /// "gutenberg:<n>", "se:<author>/<title>" or "local:<hash>".
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub subjects: Vec<String>,
    pub summary: String,
    pub cover: Option<String>,
    pub epub_url: Option<String>,
    pub source: String,
    pub popularity: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Taste {
    pub genres: Vec<String>,
    pub authors: Vec<String>,
    /// short, medium or long.
    pub length: String,
    pub done: bool,
}

/// Genres offered in the taste quiz, with the catalogue subject words they match.
pub const GENRES: &[(&str, &str)] = &[
    ("Adventure", "adventure"),
    ("Mystery & detective", "detective"),
    ("Science fiction", "science fiction"),
    ("Fantasy & fairy tales", "fantasy"),
    ("Romance", "love stories"),
    ("Horror & gothic", "horror"),
    ("Historical fiction", "historical fiction"),
    ("Philosophy", "philosophy"),
    ("History", "history"),
    ("Poetry", "poetry"),
    ("Drama & plays", "drama"),
    ("Humour", "humor"),
    ("Children's classics", "children"),
    ("Biography & memoir", "biography"),
    ("Science & nature", "natural history"),
    ("Travel", "travel"),
];

/// "Melville, Herman" becomes "Herman Melville".
fn display_name(n: &str) -> String {
    match n.split_once(", ") {
        Some((last, first)) => format!("{first} {last}"),
        None => n.to_string(),
    }
}

pub fn parse_gutendex(v: &Value) -> Vec<Book> {
    v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["copyright"].as_bool() != Some(true))
        .filter_map(|b| {
            let id = b["id"].as_u64()?;
            let f = &b["formats"];
            let epub = f["application/epub+zip"].as_str().map(String::from);
            epub.as_ref()?;
            Some(Book {
                id: format!("gutenberg:{id}"),
                title: b["title"].as_str()?.to_string(),
                authors: b["authors"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str()).map(display_name).collect(),
                subjects: b["subjects"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .chain(b["bookshelves"].as_array().into_iter().flatten())
                    .filter_map(|s| s.as_str())
                    .map(|s| s.trim_start_matches("Category: ").to_string())
                    .collect(),
                summary: b["summaries"][0].as_str().unwrap_or("").replace(" (This is an automatically generated summary.)", ""),
                cover: f["image/jpeg"].as_str().map(String::from),
                epub_url: epub,
                source: "Project Gutenberg".into(),
                popularity: b["download_count"].as_u64().unwrap_or(0),
            })
        })
        .collect()
}

pub async fn gutenberg(client: &reqwest::Client, search: Option<&str>, topic: Option<&str>) -> Result<Vec<Book>> {
    let mut q: Vec<(&str, &str)> = vec![("languages", "en"), ("sort", "popular")];
    if let Some(s) = search {
        q.push(("search", s));
    }
    if let Some(t) = topic {
        q.push(("topic", t));
    }
    let v: Value =
        client.get("https://gutendex.com/books/").query(&q).send().await?.error_for_status()?.json().await.context("gutendex")?;
    Ok(parse_gutendex(&v))
}

pub fn parse_standard_ebooks(body: &[u8]) -> Result<Vec<Book>> {
    let feed = feed_rs::parser::parse(body)?;
    Ok(feed
        .entries
        .into_iter()
        .filter_map(|e| {
            let epub = e
                .links
                .iter()
                .find(|l| {
                    l.rel.as_deref() == Some("enclosure")
                        && l.media_type.as_deref() == Some("application/epub+zip")
                        && !l.href.contains("_advanced")
                })?
                .href
                .clone();
            let slug = e.id.trim_start_matches("https://standardebooks.org/ebooks/").to_string();
            Some(Book {
                id: format!("se:{slug}"),
                title: e.title.map(|t| t.content)?,
                authors: e.authors.into_iter().filter_map(|a| a.name).collect(),
                subjects: e.categories.into_iter().map(|c| c.term).collect::<HashSet<_>>().into_iter().collect(),
                summary: e.summary.map(|s| s.content).unwrap_or_default(),
                cover: e.media.iter().flat_map(|m| m.thumbnails.iter()).map(|t| t.image.uri.clone()).next(),
                epub_url: Some(epub),
                source: "Standard Ebooks".into(),
                popularity: 0,
            })
        })
        .collect())
}

pub async fn standard_ebooks(client: &reqwest::Client) -> Result<Vec<Book>> {
    let body = client.get("https://standardebooks.org/feeds/atom/new-releases").send().await?.error_for_status()?.bytes().await?;
    parse_standard_ebooks(&body)
}

const MAX_BOOK: usize = 150 * 1024 * 1024;

/// Book file kinds the reader opens. The extension is kept on disk because the
/// reader tells ZIP-based kinds (EPUB, comic, zipped FB2) apart by file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Epub,
    Pdf,
    Mobi,
    Fb2,
    Cbz,
    Fbz,
}

impl Format {
    pub fn ext(self) -> &'static str {
        match self {
            Format::Epub => "epub",
            Format::Pdf => "pdf",
            Format::Mobi => "mobi",
            Format::Fb2 => "fb2",
            Format::Cbz => "cbz",
            Format::Fbz => "fbz",
        }
    }

    /// From a stored file name; old library files without a known extension are EPUB.
    pub fn of_path(path: &str) -> Format {
        let lower = path.to_ascii_lowercase();
        [Format::Pdf, Format::Mobi, Format::Fb2, Format::Cbz, Format::Fbz]
            .into_iter()
            .find(|f| lower.ends_with(&format!(".{}", f.ext())))
            .unwrap_or(Format::Epub)
    }
}

/// Check a book file by its contents. `name` is the original file name or URL,
/// used only to tell ZIP kinds apart (.cbz comics, .fbz zipped FB2).
pub fn sniff(bytes: &[u8], name: &str) -> Result<Format> {
    if bytes.len() > MAX_BOOK {
        bail!("file is larger than 150 MB");
    }
    let name = name.to_ascii_lowercase();
    if bytes.starts_with(b"%PDF-") {
        return Ok(Format::Pdf);
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Ok(if name.ends_with(".cbz") {
            Format::Cbz
        } else if name.ends_with(".fbz") || name.ends_with(".fb2.zip") {
            Format::Fbz
        } else {
            Format::Epub
        });
    }
    // MOBI and AZW3 share the Palm database header "BOOKMOBI" at byte 60.
    if bytes.get(60..68) == Some(b"BOOKMOBI") {
        return Ok(Format::Mobi);
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    if head.contains("<FictionBook") {
        return Ok(Format::Fb2);
    }
    bail!("not a book file this library can open (EPUB, PDF, MOBI, AZW3, FB2 or CBZ)")
}

pub async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let u = url::Url::parse(url)?;
    if u.scheme() != "https" || !matches!(u.host_str(), Some("www.gutenberg.org" | "gutenberg.org" | "standardebooks.org")) {
        bail!("books are only downloaded from Project Gutenberg or Standard Ebooks");
    }
    // Large illustrated books can take a while on a slow link; the shared client's 20s is for feeds.
    let bytes = client.get(url).timeout(std::time::Duration::from_secs(300)).send().await?.error_for_status()?.bytes().await?.to_vec();
    if sniff(&bytes, url)? != Format::Epub {
        bail!("the catalogue sent something other than an EPUB");
    }
    Ok(bytes)
}

/// Order candidates for a reader: favourite authors first, then subjects that
/// match their genres and liked books, then popularity. Drops books already owned.
pub fn rank(mut candidates: Vec<Book>, taste: &Taste, liked_subjects: &[String], owned: &[Book]) -> Vec<Book> {
    let owned_ids: HashSet<&str> = owned.iter().map(|b| b.id.as_str()).collect();
    let owned_works: HashSet<String> = owned.iter().map(work_key).collect();
    candidates.retain(|b| !owned_ids.contains(b.id.as_str()) && !owned_works.contains(&work_key(b)));
    let genre_words: Vec<String> =
        taste.genres.iter().filter_map(|g| GENRES.iter().find(|(name, _)| name == g).map(|(_, w)| w.to_lowercase())).collect();
    let fav: Vec<String> = taste.authors.iter().map(|a| a.to_lowercase()).collect();
    let score = |b: &Book| {
        let subj = b.subjects.join(" ").to_lowercase();
        let authors = b.authors.join(" ").to_lowercase();
        let a = fav.iter().any(|f| !f.is_empty() && authors.contains(f)) as u64 * 1000;
        let g = genre_words.iter().filter(|w| subj.contains(w.as_str())).count() as u64 * 100;
        let l = liked_subjects.iter().filter(|s| subj.contains(&s.to_lowercase())).count() as u64 * 60;
        a + g + l + (b.popularity as f64).log10().max(0.0) as u64 * 10
    };
    candidates.sort_by_key(|b| std::cmp::Reverse(score(b)));
    // Catalogues list several editions of one work; keep the best-ranked one.
    let mut seen = HashSet::new();
    candidates.retain(|b| seen.insert(work_key(b)));
    candidates
}

/// One key per work across editions: main title (before ";" or ":") plus the
/// first author's surname, lowercased.
pub fn work_key(b: &Book) -> String {
    let title = b.title.split([';', ':']).next().unwrap_or("").trim().trim_end_matches(',').to_lowercase();
    let author = b
        .authors
        .first()
        .map(|a| a.split(',').next().unwrap_or(a).trim().rsplit(' ').next().unwrap_or("").to_lowercase())
        .unwrap_or_default();
    format!("{title}|{author}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gutendex_and_skips_books_without_epub_or_in_copyright() {
        let v = serde_json::json!({"results":[
            {"id":2701,"title":"Moby Dick","authors":[{"name":"Melville, Herman"}],"subjects":["Sea stories"],"bookshelves":["Category: Adventure"],
             "summaries":["A whale. (This is an automatically generated summary.)"],"copyright":false,"download_count":90000,
             "formats":{"application/epub+zip":"https://www.gutenberg.org/ebooks/2701.epub3.images","image/jpeg":"https://www.gutenberg.org/cache/epub/2701/pg2701.cover.medium.jpg"}},
            {"id":1,"title":"No epub","authors":[],"subjects":[],"bookshelves":[],"summaries":[],"copyright":false,"formats":{}},
            {"id":2,"title":"Still in copyright","authors":[],"subjects":[],"bookshelves":[],"summaries":[],"copyright":true,"formats":{"application/epub+zip":"x"}}
        ]});
        let b = parse_gutendex(&v);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].authors, vec!["Herman Melville"]);
        assert_eq!(b[0].summary, "A whale.");
        assert!(b[0].subjects.contains(&"Adventure".to_string()));
    }

    #[test]
    fn ranks_favourite_author_then_genre() {
        let mk = |id: &str, a: &str, s: &str, p| Book {
            id: id.into(),
            title: id.into(),
            authors: vec![a.into()],
            subjects: vec![s.into()],
            summary: String::new(),
            cover: None,
            epub_url: None,
            source: String::new(),
            popularity: p,
        };
        let taste = Taste { genres: vec!["Mystery & detective".into()], authors: vec!["austen".into()], ..Default::default() };
        let owned = vec![mk("owned", "Jane Austen", "x", 5)];
        let mut other_edition = mk("pop2", "X", "Poetry", 50);
        other_edition.title = "pop; or, a subtitle".into();
        let r = rank(
            vec![
                mk("pop", "X", "Poetry", 1_000_000),
                other_edition,
                mk("det", "Y", "Detective and mystery stories", 10),
                mk("aus", "Jane Austen", "Love stories", 5),
                mk("owned", "Jane Austen", "x", 5),
            ],
            &taste,
            &[],
            &owned,
        );
        let ids: Vec<_> = r.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["aus", "det", "pop"]);
    }

    #[test]
    fn sniffs_formats_and_rejects_foreign_hosts() {
        assert_eq!(sniff(b"%PDF-1.7", "a.pdf").unwrap(), Format::Pdf);
        assert_eq!(sniff(b"PK\x03\x04rest", "a.epub").unwrap(), Format::Epub);
        assert_eq!(sniff(b"PK\x03\x04rest", "Comic.CBZ").unwrap(), Format::Cbz);
        assert_eq!(sniff(b"PK\x03\x04rest", "tale.fb2.zip").unwrap(), Format::Fbz);
        let mut mobi = vec![0u8; 80];
        mobi[60..68].copy_from_slice(b"BOOKMOBI");
        assert_eq!(sniff(&mobi, "x.azw3").unwrap(), Format::Mobi);
        assert_eq!(sniff(b"<?xml version=\"1.0\"?><FictionBook>", "x").unwrap(), Format::Fb2);
        assert!(sniff(b"hello", "notes.txt").is_err());
        assert_eq!(Format::of_path("/lib/local_ab.pdf"), Format::Pdf);
        assert_eq!(Format::of_path("/lib/gutenberg_1.epub"), Format::Epub);
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(rt.block_on(download(&crate::feeds::client(), "https://evil.example/x.epub")).is_err());
    }
}
