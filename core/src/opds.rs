//! OPDS catalogues: the open format libraries and book sites use to list books
//! for reading apps. Reads OPDS 1.x (Atom XML) and OPDS 2.0 (JSON).
//!
//! Only links typed as catalogue pages are followed, and a reply that turns out to
//! be a web page is refused, so browsing never wanders into a site's HTML (some
//! sites ban addresses that crawl their pages).

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::books::{self, Format};

const MAX_PAGE: usize = 8 * 1024 * 1024;

/// A catalogue the reader added. The password, if any, is kept by the app in the
/// system keyring (or app-private storage on phones), never here.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Catalogue {
    pub id: String,
    pub title: String,
    pub url: String,
    /// Empty when the catalogue is open to all.
    #[serde(default)]
    pub user: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Preset {
    pub title: &'static str,
    pub url: &'static str,
    pub note: &'static str,
    pub login: bool,
}

pub const PRESETS: [Preset; 2] = [
    Preset {
        title: "Project Gutenberg",
        url: "https://www.gutenberg.org/ebooks.opds/",
        note: "Over 70,000 free public-domain books. No login.",
        login: false,
    },
    Preset {
        title: "Standard Ebooks",
        url: "https://standardebooks.org/feeds/opds",
        note: "Carefully produced public-domain books. The catalogue feed is for Patrons Circle members: \
               use your member email address as the user name and leave the password empty.",
        login: true,
    },
];

/// One page of a catalogue, ready to show.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Page {
    pub title: String,
    pub url: String,
    /// Ungrouped content, then named groups (OPDS 2.0 groups).
    pub links: Vec<NavLink>,
    pub books: Vec<Pub>,
    pub groups: Vec<Group>,
    pub next: Option<String>,
    pub prev: Option<String>,
    /// A search address with `{searchTerms}` in it, once resolved.
    pub search: Option<String>,
    /// An OpenSearch description still to be read for the search address.
    #[serde(skip)]
    pub search_description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Group {
    pub title: String,
    pub links: Vec<NavLink>,
    pub books: Vec<Pub>,
}

/// A link to another catalogue page (a shelf, a subject, a book's own page).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct NavLink {
    pub title: String,
    pub url: String,
    pub summary: String,
    pub cover: Option<String>,
}

/// A book with at least one file this library can open.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Pub {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub summary: String,
    pub cover: Option<String>,
    /// One per format, best first.
    pub downloads: Vec<Download>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Download {
    pub url: String,
    /// epub, pdf, mobi, fb2, fbz or cbz.
    pub format: String,
    /// Size in bytes when the catalogue says; used to pick the fuller edition.
    #[serde(default, skip_serializing)]
    pub size: u64,
}

fn format_of(media_type: &str) -> Option<Format> {
    let t = media_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    Some(match t.as_str() {
        "application/epub+zip" => Format::Epub,
        "application/pdf" => Format::Pdf,
        "application/x-mobipocket-ebook" | "application/vnd.amazon.ebook" | "application/x-mobi8-ebook" => Format::Mobi,
        "application/x-fictionbook+xml" | "application/fb2" | "text/fb2+xml" => Format::Fb2,
        "application/fb2+zip" | "application/x-zip-compressed-fb2" => Format::Fbz,
        "application/vnd.comicbook+zip" | "application/x-cbz" => Format::Cbz,
        _ => return None,
    })
}

fn is_catalogue_type(t: &str) -> bool {
    let t = t.to_ascii_lowercase();
    t.starts_with("application/atom+xml") || t.starts_with("application/opds+json")
}

fn is_acquisition(rel: &str) -> bool {
    rel == "http://opds-spec.org/acquisition" || rel == "http://opds-spec.org/acquisition/open-access"
}

/// Resolve a link against the page. Only web links; plain http is upgraded when
/// the catalogue itself is https.
fn resolve(base: &Url, href: &str) -> Option<String> {
    let mut u = base.join(href.trim()).ok()?;
    match u.scheme() {
        "https" => {}
        "http" if base.scheme() == "https" => u.set_scheme("https").ok()?,
        "http" => {}
        _ => return None,
    }
    Some(u.into())
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Keep the best file per format: the largest when sizes are known (fuller
/// editions with images), otherwise the first listed.
fn best_per_format(mut found: Vec<Download>) -> Vec<Download> {
    let order = ["epub", "pdf", "mobi", "fb2", "fbz", "cbz"];
    found.sort_by_key(|d| (order.iter().position(|o| *o == d.format), std::cmp::Reverse(d.size)));
    found.dedup_by(|a, b| a.format == b.format);
    found
}

fn download_of(media_type: &str, base: &Url, href: &str, size: u64) -> Option<Download> {
    Some(Download { format: format_of(media_type)?.ext().into(), url: resolve(base, href)?, size })
}

/// Some catalogues list each edition of a book (with or without pictures) as its
/// own entry. Show one book with the best file of each kind.
fn merge_editions(list: Vec<Pub>) -> Vec<Pub> {
    let mut out: Vec<Pub> = Vec::new();
    for p in list {
        let key = |x: &Pub| (x.title.to_lowercase(), x.authors.join("|").to_lowercase());
        match out.iter_mut().find(|o| key(o) == key(&p)) {
            Some(o) => {
                let mut all = std::mem::take(&mut o.downloads);
                all.extend(p.downloads);
                o.downloads = best_per_format(all);
                o.cover = o.cover.take().or(p.cover);
                if o.summary.is_empty() {
                    o.summary = p.summary;
                }
            }
            None => out.push(p),
        }
    }
    out
}

fn pub_id(key: &str) -> String {
    format!("opds:{}", crate::feeds::story_id(key))
}

/// Parse one catalogue page. JSON is OPDS 2.0; anything else is tried as Atom.
pub fn parse(body: &[u8], page_url: &str) -> Result<Page> {
    let base = Url::parse(page_url).context("catalogue address")?;
    let first = body.iter().find(|b| !b.is_ascii_whitespace()).copied();
    let mut page = if first == Some(b'{') {
        parse_json(&serde_json::from_slice(body).context("catalogue JSON")?, &base)
    } else {
        parse_atom(body, &base)?
    };
    page.url = page_url.into();
    page.books = merge_editions(page.books);
    for g in &mut page.groups {
        g.books = merge_editions(std::mem::take(&mut g.books));
    }
    Ok(page)
}

fn parse_atom(body: &[u8], base: &Url) -> Result<Page> {
    let feed = feed_rs::parser::Builder::new()
        .base_uri(Some(base.as_str()))
        .build()
        .parse(body)
        .context("this address did not return a catalogue")?;
    let mut page = Page { title: feed.title.map(|t| one_line(&t.content)).unwrap_or_default(), ..Default::default() };
    for l in &feed.links {
        let rel = l.rel.as_deref().unwrap_or("");
        let t = l.media_type.as_deref().unwrap_or("");
        let typed_ok = t.is_empty() || is_catalogue_type(t);
        match rel {
            "next" if typed_ok => page.next = resolve(base, &l.href),
            "previous" | "prev" if typed_ok => page.prev = resolve(base, &l.href),
            "search" if t.starts_with("application/opensearchdescription+xml") => page.search_description = resolve(base, &l.href),
            "search" if is_catalogue_type(t) && l.href.contains("{searchTerms}") => page.search = resolve_template(base, &l.href),
            _ => {}
        }
    }
    for e in feed.entries {
        let title = e.title.map(|t| one_line(&t.content)).unwrap_or_default();
        let summary = e
            .summary
            .map(|s| s.content)
            .or_else(|| e.content.and_then(|c| c.body))
            .map(|s| one_line(&crate::text::strip_html(&s)))
            .unwrap_or_default();
        let cover = e
            .links
            .iter()
            .filter(|l| matches!(l.rel.as_deref(), Some("http://opds-spec.org/image/thumbnail" | "http://opds-spec.org/image")))
            .filter_map(|l| resolve(base, &l.href))
            .next();
        let found: Vec<_> = e
            .links
            .iter()
            .filter(|l| l.rel.as_deref().is_some_and(is_acquisition))
            .filter_map(|l| download_of(l.media_type.as_deref()?, base, &l.href, l.length.unwrap_or(0)))
            .collect();
        if !found.is_empty() {
            page.books.push(Pub {
                id: pub_id(if e.id.is_empty() { &found[0].url } else { &e.id }),
                title,
                authors: e.authors.into_iter().map(|a| one_line(a.name.as_deref().unwrap_or(""))).filter(|a| !a.is_empty()).collect(),
                summary,
                cover,
                downloads: best_per_format(found),
            });
            continue;
        }
        let nav = e.links.iter().find(|l| {
            !matches!(l.rel.as_deref(), Some("self" | "start" | "up" | "alternate"))
                && l.media_type.as_deref().is_some_and(is_catalogue_type)
        });
        if let Some(url) = nav.and_then(|l| resolve(base, &l.href)) {
            page.links.push(NavLink { title, url, summary, cover });
        }
    }
    Ok(page)
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => one_line(s),
        Value::Object(m) => m.get("en").or_else(|| m.values().next()).map(text_of).unwrap_or_default(),
        _ => String::new(),
    }
}

fn names(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().flat_map(names).collect(),
        Value::String(s) => vec![one_line(s)],
        Value::Object(m) => m.get("name").map(text_of).filter(|s| !s.is_empty()).into_iter().collect(),
        _ => vec![],
    }
}

fn json_nav(v: &Value, base: &Url) -> Vec<NavLink> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter(|l| l["type"].as_str().is_none_or(is_catalogue_type))
        .filter_map(|l| {
            let url = resolve(base, l["href"].as_str()?)?;
            Some(NavLink { title: text_of(&l["title"]), url, summary: String::new(), cover: None })
        })
        .collect()
}

fn json_pubs(v: &Value, base: &Url) -> Vec<Pub> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let m = &p["metadata"];
            let found: Vec<_> = p["links"]
                .as_array()?
                .iter()
                .filter(|l| {
                    l["rel"].as_str().is_some_and(is_acquisition)
                        || l["rel"].as_array().is_some_and(|r| r.iter().filter_map(Value::as_str).any(is_acquisition))
                })
                .filter_map(|l| download_of(l["type"].as_str()?, base, l["href"].as_str()?, 0))
                .collect();
            if found.is_empty() {
                return None;
            }
            let id = m["identifier"].as_str().map(String::from).unwrap_or_else(|| found[0].url.clone());
            Some(Pub {
                id: pub_id(&id),
                title: text_of(&m["title"]),
                authors: names(&m["author"]),
                summary: one_line(&crate::text::strip_html(m["description"].as_str().unwrap_or(""))),
                cover: p["images"].as_array().and_then(|i| i.iter().find_map(|x| resolve(base, x["href"].as_str()?))),
                downloads: best_per_format(found),
            })
        })
        .collect()
}

fn parse_json(v: &Value, base: &Url) -> Page {
    let mut page = Page {
        title: text_of(&v["metadata"]["title"]),
        links: json_nav(&v["navigation"], base),
        books: json_pubs(&v["publications"], base),
        ..Default::default()
    };
    for l in v["links"].as_array().into_iter().flatten() {
        let rels: Vec<&str> = match &l["rel"] {
            Value::String(s) => vec![s],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        let Some(href) = l["href"].as_str() else { continue };
        let t = l["type"].as_str().unwrap_or("");
        let typed_ok = t.is_empty() || is_catalogue_type(t);
        if rels.contains(&"next") && typed_ok {
            page.next = resolve(base, href);
        } else if rels.contains(&"previous") && typed_ok {
            page.prev = resolve(base, href);
        } else if rels.contains(&"search") && is_catalogue_type(t) && l["templated"].as_bool() == Some(true) {
            page.search = resolve_template(base, href);
        } else if rels.contains(&"search") && t.starts_with("application/opensearchdescription+xml") {
            page.search_description = resolve(base, href);
        }
    }
    for g in v["groups"].as_array().into_iter().flatten() {
        let group = Group {
            title: text_of(&g["metadata"]["title"]),
            links: json_nav(&g["navigation"], base),
            books: json_pubs(&g["publications"], base),
        };
        if !group.links.is_empty() || !group.books.is_empty() {
            page.groups.push(group);
        }
    }
    page
}

/// Normalise a search template to use `{searchTerms}`. OPDS 2.0 uses URI
/// templates such as `/search{?query}`.
fn resolve_template(base: &Url, template: &str) -> Option<String> {
    let t = template.replace("{?query}", "?query={searchTerms}").replace("{query}", "{searchTerms}");
    // Braces must survive joining, so swap them out around it.
    let joined = resolve(base, &t.replace('{', "%7B").replace('}', "%7D"))?;
    Some(joined.replace("%7B", "{").replace("%7D", "}"))
}

/// The catalogue search address from an OpenSearch description.
pub fn parse_open_search(xml: &str, base: &str) -> Option<String> {
    let base = Url::parse(base).ok()?;
    let tag = regex::Regex::new(r"(?s)<Url\b([^>]*)>").ok()?;
    let attr = |tag: &str, name: &str| {
        regex::Regex::new(&format!(r#"\b{name}\s*=\s*["']([^"']*)["']"#))
            .ok()?
            .captures(tag)
            .map(|c| html_escape::decode_html_entities(&c[1]).into_owned())
    };
    tag.captures_iter(xml)
        .map(|c| c[1].to_string())
        .filter(|t| attr(t, "type").is_some_and(|ty| is_catalogue_type(&ty)))
        .find_map(|t| resolve_template(&base, &attr(&t, "template")?))
}

/// Put the reader's words into a search address; other optional parameters are dropped.
pub fn fill(template: &str, query: &str) -> String {
    let q: String = url::form_urlencoded::byte_serialize(query.trim().as_bytes()).collect();
    let out = template.replace("{searchTerms}", &q);
    regex::Regex::new(r"\{[^}]*\}").map(|r| r.replace_all(&out, "").into_owned()).unwrap_or(out)
}

/// A login for one catalogue. It is only ever sent to that catalogue's own site.
pub struct Login<'a> {
    pub catalogue: &'a str,
    pub user: &'a str,
    pub password: &'a str,
}

fn same_site(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme() && a.host_str() == b.host_str() && a.port_or_known_default() == b.port_or_known_default()
}

/// Plain-http logins are allowed only on this device or the home network
/// (a book server on a laptop, for example).
fn login_safe(u: &Url) -> bool {
    if u.scheme() == "https" {
        return true;
    }
    match u.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback() || ip.is_unique_local(),
        Some(url::Host::Domain(d)) => d == "localhost" || d.ends_with(".local") || d.ends_with(".lan"),
        None => false,
    }
}

async fn get(client: &reqwest::Client, url: &str, login: Option<&Login<'_>>, timeout_s: u64) -> Result<reqwest::Response> {
    let u = Url::parse(url)?;
    if !matches!(u.scheme(), "http" | "https") {
        bail!("only web addresses can be opened");
    }
    let mut req = client.get(u.clone()).timeout(std::time::Duration::from_secs(timeout_s));
    let mut sent_login = false;
    if let Some(l) = login.filter(|l| !l.user.is_empty()) {
        let home = Url::parse(l.catalogue)?;
        // Never hand the login to another site. reqwest also drops it on redirects to other hosts.
        if same_site(&home, &u) {
            if !login_safe(&u) {
                bail!("this catalogue does not use a secure (https) connection, so the login was not sent");
            }
            req = req.basic_auth(l.user, Some(l.password));
            sent_login = true;
        }
    }
    let res = req.send().await?;
    match res.status().as_u16() {
        401 if sent_login => bail!("the catalogue did not accept this login; check the user name and password in its settings"),
        401 => bail!("this catalogue asks for a login; add your user name and password in its settings"),
        403 => bail!("the catalogue would not show this page (it answered \"forbidden\")"),
        404 => bail!("the catalogue has no page at this address"),
        _ => {}
    }
    Ok(res.error_for_status()?)
}

/// Fetch and read one catalogue page, including its search address.
pub async fn browse(client: &reqwest::Client, url: &str, login: Option<&Login<'_>>) -> Result<Page> {
    let res = get(client, url, login, 30).await?;
    let final_url = res.url().to_string();
    let ctype = res.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    if ctype.starts_with("text/html") || ctype.starts_with("application/xhtml") {
        bail!("this address is a web page, not a catalogue");
    }
    if res.content_length().is_some_and(|n| n as usize > MAX_PAGE) {
        bail!("this catalogue page is too large");
    }
    let body = res.bytes().await?;
    if body.len() > MAX_PAGE {
        bail!("this catalogue page is too large");
    }
    let mut page = parse(&body, &final_url)?;
    if page.search.is_none()
        && let Some(osd) = page.search_description.take()
        && let Ok(res) = get(client, &osd, login, 20).await
        && let Ok(text) = res.text().await
    {
        page.search = parse_open_search(&text, &osd);
    }
    Ok(page)
}

/// Download one book file and check it is a kind the reader opens.
pub async fn download(client: &reqwest::Client, d: &Download, login: Option<&Login<'_>>) -> Result<(Vec<u8>, Format)> {
    let res = get(client, &d.url, login, 300).await?;
    if res.content_length().is_some_and(|n| n > 150 * 1024 * 1024) {
        bail!("file is larger than 150 MB");
    }
    let bytes = res.bytes().await?.to_vec();
    // The URL often has no extension, so name it by the format the catalogue promised.
    let format = books::sniff(&bytes, &format!("book.{}", d.format)).map_err(|e| anyhow!("{e}"))?;
    Ok((bytes, format))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATOM_ROOT: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
<id>urn:test:root</id><title>Test Library</title><updated>2026-11-06T00:00:00Z</updated>
<link rel="search" type="application/opensearchdescription+xml" href="https://books.example/osd.xml"/>
<link rel="self" type="application/atom+xml;profile=opds-catalog" href="/opds/"/>
<link rel="alternate" type="text/html" href="/books/"/>
<link rel="next" type="application/atom+xml;profile=opds-catalog" href="/opds/?start=26"/>
<entry><id>urn:test:popular</id><title>Popular</title><updated>2026-11-06T00:00:00Z</updated>
<content type="text">Our most read books.</content>
<link type="application/atom+xml;profile=opds-catalog" rel="subsection" href="/opds/popular"/></entry>
<entry><id>urn:test:web</id><title>About us</title><updated>2026-11-06T00:00:00Z</updated>
<link type="text/html" rel="alternate" href="/about"/></entry>
<entry><id>urn:test:84</id><title>Frankenstein</title><updated>2026-11-06T00:00:00Z</updated>
<author><name>Shelley, Mary</name></author>
<summary>A &lt;b&gt;monster&lt;/b&gt; story.</summary>
<link type="application/epub+zip" rel="http://opds-spec.org/acquisition" length="294175" href="/84.epub.noimages"/>
<link type="application/x-mobipocket-ebook" rel="http://opds-spec.org/acquisition" length="519544" href="/84.kindle"/>
<link type="application/epub+zip" rel="http://opds-spec.org/acquisition" length="440131" href="http://books.example/84.epub3.images"/>
<link type="application/vnd.adobe.adept+xml" rel="http://opds-spec.org/acquisition/borrow" href="/84.acsm"/>
<link type="image/jpeg" rel="http://opds-spec.org/image/thumbnail" href="/84.small.jpg"/>
</entry>
<entry><id>urn:test:84:images</id><title>Frankenstein</title><updated>2026-11-06T00:00:00Z</updated>
<author><name>Shelley, Mary</name></author>
<link type="application/x-mobipocket-ebook" rel="http://opds-spec.org/acquisition" length="9" href="/84.kindle.small"/>
<link type="application/pdf" rel="http://opds-spec.org/acquisition" length="1000" href="/84.pdf"/>
</entry>
</feed>"#;

    #[test]
    fn reads_opds1_atom() {
        let p = parse(ATOM_ROOT.as_bytes(), "https://books.example/opds/").unwrap();
        assert_eq!(p.title, "Test Library");
        assert_eq!(p.next.as_deref(), Some("https://books.example/opds/?start=26"));
        assert_eq!(p.search_description.as_deref(), Some("https://books.example/osd.xml"));
        // The HTML-only entry is not followed.
        assert_eq!(p.links.len(), 1);
        assert_eq!(p.links[0].url, "https://books.example/opds/popular");
        assert_eq!(p.links[0].summary, "Our most read books.");
        let b = &p.books[0];
        assert_eq!((b.title.as_str(), b.authors[0].as_str(), b.summary.as_str()), ("Frankenstein", "Shelley, Mary", "A monster story."));
        assert_eq!(b.cover.as_deref(), Some("https://books.example/84.small.jpg"));
        // Best EPUB (largest, upgraded to https) first, one per format, DRM loans skipped.
        assert_eq!(
            b.downloads,
            vec![
                Download { url: "https://books.example/84.epub3.images".into(), format: "epub".into(), size: 440131 },
                Download { url: "https://books.example/84.pdf".into(), format: "pdf".into(), size: 1000 },
                Download { url: "https://books.example/84.kindle".into(), format: "mobi".into(), size: 519544 },
            ]
        );
        assert!(b.id.starts_with("opds:"));
        // The second edition entry was folded into the first.
        assert_eq!(p.books.len(), 1);
    }

    #[test]
    fn reads_opds2_json() {
        let j = r#"{"metadata":{"title":"Home"},
          "links":[{"rel":"self","href":"/2.0/home.json","type":"application/opds+json"},
                   {"rel":"search","href":"/2.0/search{?query}","type":"application/opds+json","templated":true},
                   {"rel":"next","href":"/2.0/home.json?page=2","type":"application/opds+json"}],
          "navigation":[{"href":"/2.0/new.json","title":"New","type":"application/opds+json"},
                        {"href":"/about.html","title":"About","type":"text/html"}],
          "publications":[{"metadata":{"title":"Moby-Dick","author":{"name":"Herman Melville"},"identifier":"urn:isbn:1"},
             "links":[{"rel":"http://opds-spec.org/acquisition/open-access","href":"/moby.epub","type":"application/epub+zip"},
                      {"rel":"http://opds-spec.org/acquisition/buy","href":"/buy","type":"application/epub+zip"}],
             "images":[{"href":"/moby.jpg","type":"image/jpeg"}]},
            {"metadata":{"title":"Audio only"},"links":[{"rel":"http://opds-spec.org/acquisition","href":"/a.mp3","type":"audio/mpeg"}]}],
          "groups":[{"metadata":{"title":"French"},"publications":[{"metadata":{"title":"Voyage","author":["Jules Verne"]},
             "links":[{"rel":"http://opds-spec.org/acquisition","href":"/v.pdf","type":"application/pdf"}]}]},
             {"metadata":{"title":"Empty"},"navigation":[]}]}"#;
        let p = parse(j.as_bytes(), "https://test.example/2.0/home.json").unwrap();
        assert_eq!(p.title, "Home");
        assert_eq!(p.search.as_deref(), Some("https://test.example/2.0/search?query={searchTerms}"));
        assert_eq!(p.next.as_deref(), Some("https://test.example/2.0/home.json?page=2"));
        assert_eq!(p.links.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), ["New"]);
        assert_eq!(p.books.len(), 1);
        assert_eq!(p.books[0].authors, ["Herman Melville"]);
        assert_eq!(p.books[0].downloads, vec![Download { url: "https://test.example/moby.epub".into(), format: "epub".into(), size: 0 }]);
        assert_eq!(p.books[0].cover.as_deref(), Some("https://test.example/moby.jpg"));
        assert_eq!(p.groups.len(), 1);
        assert_eq!((p.groups[0].title.as_str(), p.groups[0].books[0].downloads[0].format.as_str()), ("French", "pdf"));
    }

    #[test]
    fn open_search_and_fill() {
        let osd = r#"<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
          <Url type="text/html" template="https://books.example/search?q={searchTerms}"/>
          <Url type="application/atom+xml"
               template="http://m.books.example/search.opds/?query={searchTerms}&amp;start={startIndex?}"/>
        </OpenSearchDescription>"#;
        let t = parse_open_search(osd, "https://books.example/osd.xml").unwrap();
        assert_eq!(t, "https://m.books.example/search.opds/?query={searchTerms}&start={startIndex?}");
        assert_eq!(fill(&t, " moby dick&co "), "https://m.books.example/search.opds/?query=moby+dick%26co&start=");
    }

    #[test]
    fn login_stays_home_and_private() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert!(same_site(&u("https://a.example/opds"), &u("https://a.example/books/1.epub")));
        assert!(!same_site(&u("https://a.example/opds"), &u("https://cdn.example/1.epub")));
        assert!(!same_site(&u("https://a.example/opds"), &u("http://a.example/1.epub")));
        assert!(login_safe(&u("https://a.example/")));
        assert!(login_safe(&u("http://169.254.10.20:8080/opds")));
        assert!(login_safe(&u("http://[fd00::20]:8080/opds")));
        assert!(login_safe(&u("http://nas.lan/opds")));
        assert!(login_safe(&u("http://localhost:8083/opds")));
        assert!(!login_safe(&u("http://books.example/opds")));
        assert_eq!(resolve(&u("https://a.example/x/"), "javascript:alert(1)"), None);
        assert_eq!(resolve(&u("http://nas.lan/opds/"), "b.epub").as_deref(), Some("http://nas.lan/opds/b.epub"));
    }

    #[test]
    fn refuses_web_pages_as_catalogues() {
        assert!(parse(b"<!DOCTYPE html><html><body>Hi</body></html>", "https://a.example/").is_err());
    }

    /// Live check against Project Gutenberg's catalogue: `cargo test -- --ignored opds_live`.
    #[tokio::test]
    #[ignore]
    async fn opds_live_gutenberg() {
        let c = crate::feeds::client();
        let root = browse(&c, PRESETS[0].url, None).await.unwrap();
        assert!(!root.links.is_empty(), "{root:?}");
        let search = root.search.clone().expect("search");
        let found = browse(&c, &fill(&search, "frankenstein"), None).await.unwrap();
        let first = found.links.first().expect("a result");
        let book = browse(&c, &first.url, None).await.unwrap();
        let p = book.books.first().expect("a book");
        assert_eq!(p.downloads[0].format, "epub");
        let (bytes, f) = download(&c, &p.downloads[0], None).await.unwrap();
        assert_eq!(f, Format::Epub);
        assert!(bytes.len() > 10_000);
    }
}
