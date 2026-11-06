//! Tauri shell: wires the core to the window. All logic lives in granthalaya-core.

mod secrets;

use std::collections::HashMap;
use std::path::PathBuf;

use futures::future::join_all;
use granthalaya_core::bias::{self, OutletFacts, Report};
use granthalaya_core::books::{self, Book, Taste};
use granthalaya_core::comfort::{self, Question, Settings};
use granthalaya_core::db::{EditionInfo, Hit, Store};
use granthalaya_core::edition::{self, Edition, EditionStory, Prefs};
use granthalaya_core::highlights::{self, Highlight};
use granthalaya_core::opds::{self, Catalogue, Page, Preset, Pub};
use granthalaya_core::{article, feeds, learner, sources, topics, wikidata};
use serde::Serialize;
use std::io::Write;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FsExt, OpenOptions};
use tokio::sync::Mutex;

struct App {
    store: std::sync::Mutex<Store>,
    client: feeds::Client,
    books_dir: PathBuf,
    secrets: secrets::Secrets,
    /// Only one edition build at a time.
    building: Mutex<()>,
}

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl App {
    fn db<T>(&self, f: impl FnOnce(&mut Store) -> anyhow::Result<T>) -> Res<T> {
        let mut s = self.store.lock().map_err(err)?;
        f(&mut s).map_err(err)
    }
}

const FACTS_MAX_AGE_DAYS: i64 = 30;

/// Owner and funder facts for every outlet: cached, refreshed monthly, and
/// fetched one at a time to respect Wikidata's rate limits.
async fn outlet_facts(app: &App) -> HashMap<String, OutletFacts> {
    let mut out = HashMap::new();
    let mut stale: Vec<&sources::Outlet> = Vec::new();
    let mut cached_facts: HashMap<String, OutletFacts> = HashMap::new();
    for o in sources::outlets() {
        let Some(qid) = &o.qid else {
            out.insert(o.id.clone(), wikidata::base_facts(o));
            continue;
        };
        let cached = app.db(|s| s.outlet_facts(qid)).ok().flatten();
        let fresh = cached
            .as_ref()
            .and_then(|(_, at)| chrono::DateTime::parse_from_rfc3339(at).ok())
            .is_some_and(|at| chrono::Utc::now().signed_duration_since(at).num_days() < FACTS_MAX_AGE_DAYS);
        let parsed: Option<OutletFacts> = cached.as_ref().and_then(|(j, _)| serde_json::from_str(j).ok());
        match parsed {
            Some(f) if fresh => {
                out.insert(o.id.clone(), f);
            }
            other => {
                if let Some(f) = other {
                    cached_facts.insert(o.id.clone(), f);
                }
                stale.push(o);
            }
        }
    }
    if stale.is_empty() {
        return out;
    }
    match wikidata::fetch_many(&app.client, &stale).await {
        Ok(list) => {
            for (o, f) in stale.iter().zip(list) {
                if let (Some(qid), Ok(j)) = (&o.qid, serde_json::to_string(&f)) {
                    let _ = app.db(|s| s.set_outlet_facts(qid, &j));
                }
                out.insert(o.id.clone(), f);
            }
        }
        // Offline or rate-limited: keep old facts if we have them, else just the funding model.
        Err(_) => {
            for o in stale {
                let f = cached_facts.remove(&o.id).unwrap_or_else(|| wikidata::base_facts(o));
                out.insert(o.id.clone(), f);
            }
        }
    }
    out
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[tauri::command]
async fn edition_today(app: tauri::State<'_, App>, force: bool) -> Res<Edition> {
    let _guard = app.building.lock().await;
    let date = today();
    if !force && let Some(j) = app.db(|s| s.edition(&date))? {
        return serde_json::from_str(&j).map_err(err);
    }
    // Feeds and ownership facts are independent; fetch both at once.
    let ((stories, ok, failed), facts) = futures::join!(edition::gather(&app.client), outlet_facts(&app));
    if stories.is_empty() {
        return Err("Could not reach any news source. Check the internet connection and try again.".into());
    }
    let (weights, prefs, dates) = app.db(|s| Ok((s.weights()?, s.get_or_default::<Prefs>("prefs")?, s.edition_dates()?)))?;
    // Edition number: this paper's place in the reader's run of papers.
    let number = dates.iter().filter(|d| d.as_str() < date.as_str()).count() as u32 + 1;
    let mut e = edition::build(edition::BuildInput {
        stories,
        facts: &facts,
        weights: &weights,
        prefs: &prefs,
        now: chrono::Utc::now(),
        number,
        sources_ok: ok,
        sources_failed: failed,
    });
    e.date = date.clone();
    let json = serde_json::to_string(&e).map_err(err)?;
    let rows: Vec<(String, String)> =
        e.pages.iter().flat_map(|p| &p.stories).map(|s| (s.story.id.clone(), serde_json::to_string(s).unwrap_or_default())).collect();
    app.db(|s| s.save_edition(&date, &json, &rows))?;
    Ok(e)
}

#[tauri::command]
fn edition_get(app: tauri::State<'_, App>, date: String) -> Res<Option<Edition>> {
    match app.db(|s| s.edition(&date))? {
        Some(j) => serde_json::from_str(&j).map(Some).map_err(err),
        None => Ok(None),
    }
}

#[tauri::command]
fn edition_dates(app: tauri::State<'_, App>) -> Res<Vec<String>> {
    app.db(|s| s.edition_dates())
}

#[derive(Serialize)]
struct Opened {
    paragraphs: Vec<String>,
    report: Report,
    /// Why the full text is not shown, if it is not.
    note: Option<String>,
}

fn load_story(app: &App, id: &str) -> Res<EditionStory> {
    let j = app.db(|s| s.story_json(id))?.ok_or("story not found")?;
    serde_json::from_str(&j).map_err(err)
}

#[tauri::command]
async fn story_open(app: tauri::State<'_, App>, id: String) -> Res<Opened> {
    if let Some((body, report)) = app.db(|s| s.story_body(&id))? {
        return Ok(Opened {
            paragraphs: serde_json::from_str(&body).map_err(err)?,
            report: serde_json::from_str(&report).map_err(err)?,
            note: None,
        });
    }
    let es = load_story(&app, &id)?;
    if es.story.paywall {
        return Ok(Opened {
            paragraphs: vec![],
            report: es.report,
            note: Some(
                "This outlet keeps most articles behind a paywall, so Granthālaya does not fetch them. Open the original to read it there."
                    .into(),
            ),
        });
    }
    match article::fetch(&app.client, &es.story.url).await {
        Ok(paragraphs) => {
            let cluster: Vec<OutletFacts> = std::iter::once(es.report.outlet.clone())
                .chain(es.others.iter().filter_map(|t| {
                    let o = sources::outlets().iter().find(|o| o.name == t.outlet_name)?;
                    Some(
                        app.db(|s| s.outlet_facts(o.qid.as_deref().unwrap_or("")))
                            .ok()
                            .flatten()
                            .and_then(|(j, _)| serde_json::from_str(&j).ok())
                            .unwrap_or_else(|| wikidata::base_facts(o)),
                    )
                }))
                .collect();
            let text = paragraphs.join("\n");
            let report = bias::assess(bias::Input {
                title: &es.story.title,
                text: &text,
                url: &es.story.url,
                full_text: true,
                outlet: es.report.outlet.clone(),
                cluster,
            });
            let (b, r) = (serde_json::to_string(&paragraphs).map_err(err)?, serde_json::to_string(&report).map_err(err)?);
            app.db(|s| s.set_story_body(&id, &b, &r))?;
            Ok(Opened { paragraphs, report, note: None })
        }
        Err(e) => Ok(Opened {
            paragraphs: vec![],
            report: es.report,
            note: Some(format!("The full text could not be fetched ({e}). Open the original to read it.")),
        }),
    }
}

#[tauri::command]
fn story_feedback(app: tauri::State<'_, App>, id: String, vote: Option<i8>, stars: Option<u8>) -> Res<()> {
    let es = load_story(&app, &id)?;
    app.db(|s| {
        s.set_feedback(&id, vote, stars)?;
        if let Some(y) = learner::target(vote, stars) {
            let mut w = s.weights()?;
            let changed = learner::update(&mut w, &learner::features(&es.story), y);
            s.save_weights(&changed)?;
        }
        Ok(())
    })
}

#[tauri::command]
fn feedback_all(app: tauri::State<'_, App>) -> Res<granthalaya_core::db::Votes> {
    app.db(|s| s.feedback())
}

#[tauri::command]
fn prefs_get(app: tauri::State<'_, App>) -> Res<Prefs> {
    app.db(|s| s.get_or_default("prefs"))
}

#[tauri::command]
fn prefs_set(app: tauri::State<'_, App>, prefs: Prefs) -> Res<()> {
    app.db(|s| s.set("prefs", &prefs))
}

#[derive(Serialize)]
struct TopicInfo {
    id: &'static str,
    label: &'static str,
    sensitive: bool,
}

#[tauri::command]
fn topics_list() -> Vec<TopicInfo> {
    topics::TOPICS.iter().map(|t| TopicInfo { id: t.id, label: t.label, sensitive: t.sensitive }).collect()
}

#[tauri::command]
fn outlets_list() -> Vec<sources::Outlet> {
    sources::outlets().to_vec()
}

// Books.

#[tauri::command]
fn taste_get(app: tauri::State<'_, App>) -> Res<Taste> {
    app.db(|s| s.get_or_default("taste"))
}

#[tauri::command]
fn taste_set(app: tauri::State<'_, App>, taste: Taste) -> Res<()> {
    app.db(|s| s.set("taste", &taste))
}

#[tauri::command]
fn genres() -> Vec<&'static str> {
    books::GENRES.iter().map(|(g, _)| *g).collect()
}

#[derive(Serialize)]
struct Shelved {
    book: Book,
    progress: f64,
    location: Option<String>,
    rating: Option<u8>,
    last_read: Option<String>,
    /// File kind for the reader: epub, pdf, mobi, fb2, cbz or fbz.
    format: &'static str,
}

fn library_rows(app: &App) -> Res<Vec<Shelved>> {
    let rows = app.db(|s| s.books())?;
    Ok(rows
        .into_iter()
        .filter_map(|(j, file, progress, location, rating, last_read)| {
            let format = books::Format::of_path(&file).ext();
            Some(Shelved { book: serde_json::from_str(&j).ok()?, progress, location, rating, last_read, format })
        })
        .collect())
}

#[tauri::command]
fn library(app: tauri::State<'_, App>) -> Res<Vec<Shelved>> {
    library_rows(&app)
}

#[tauri::command]
async fn books_suggest(app: tauri::State<'_, App>) -> Res<Vec<Book>> {
    let taste: Taste = app.db(|s| s.get_or_default("taste"))?;
    let shelf = library_rows(&app)?;
    let owned: Vec<Book> = shelf.iter().map(|b| b.book.clone()).collect();
    let liked: Vec<String> =
        shelf.iter().filter(|b| b.rating.is_some_and(|r| r >= 7)).flat_map(|b| b.book.subjects.iter().take(3).cloned()).collect();
    let topics: Vec<&str> =
        taste.genres.iter().filter_map(|g| books::GENRES.iter().find(|(n, _)| n == g).map(|(_, t)| *t)).take(4).collect();
    let mut jobs: Vec<_> = topics.iter().map(|t| books::gutenberg(&app.client, None, Some(t))).collect();
    if topics.is_empty() {
        jobs.push(books::gutenberg(&app.client, None, None));
    }
    let authors: Vec<_> = taste.authors.iter().take(3).map(|a| books::gutenberg(&app.client, Some(a), None)).collect();
    let (g, a, se) = futures::join!(join_all(jobs), join_all(authors), books::standard_ebooks(&app.client));
    let mut all: Vec<Book> = g.into_iter().chain(a).flatten().flatten().collect();
    all.extend(se.unwrap_or_default());
    if all.is_empty() {
        return Err("Could not reach the book catalogues. Check the internet connection.".into());
    }
    let mut ranked = books::rank(all, &taste, &liked, &owned);
    ranked.truncate(40);
    Ok(ranked)
}

#[tauri::command]
async fn books_search(app: tauri::State<'_, App>, query: String) -> Res<Vec<Book>> {
    books::gutenberg(&app.client, Some(query.trim()), None).await.map_err(err)
}

fn safe_name(id: &str, format: books::Format) -> String {
    let stem: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    format!("{stem}.{}", format.ext())
}

#[tauri::command]
async fn book_add(app: tauri::State<'_, App>, book: Book) -> Res<()> {
    let url = book.epub_url.clone().ok_or("this book has no EPUB download")?;
    let bytes = books::download(&app.client, &url).await.map_err(err)?;
    let path = app.books_dir.join(safe_name(&book.id, books::Format::Epub));
    std::fs::write(&path, bytes).map_err(err)?;
    let j = serde_json::to_string(&book).map_err(err)?;
    app.db(|s| s.add_book(&book.id, &j, &path.to_string_lossy()))
}

/// Import a reader's own book file. Body: raw bytes. Headers: x-name (file name),
/// x-title, x-author (all URI-encoded).
#[tauri::command]
fn book_import(app: tauri::State<'_, App>, request: Request<'_>) -> Res<String> {
    let InvokeBody::Raw(bytes) = request.body() else { return Err("expected file bytes".into()) };
    let header = |k: &str| request.headers().get(k).and_then(|v| v.to_str().ok()).map(decode_uri).unwrap_or_default();
    let format = books::sniff(bytes, &header("x-name")).map_err(err)?;
    let title = header("x-title");
    let author = header("x-author");
    let id = format!("local:{}", feeds::story_id(&format!("{title}|{author}|{}", bytes.len())));
    let path = app.books_dir.join(safe_name(&id, format));
    std::fs::write(&path, bytes).map_err(err)?;
    let book = Book {
        id: id.clone(),
        title: if title.is_empty() { "Untitled".into() } else { title },
        authors: if author.is_empty() { vec![] } else { vec![author] },
        subjects: vec![],
        summary: String::new(),
        cover: None,
        epub_url: None,
        source: "Your file".into(),
        popularity: 0,
    };
    let j = serde_json::to_string(&book).map_err(err)?;
    app.db(|s| s.add_book(&id, &j, &path.to_string_lossy()))?;
    Ok(id)
}

fn decode_uri(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[tauri::command]
fn book_bytes(app: tauri::State<'_, App>, id: String) -> Res<Response> {
    let file = app.db(|s| s.book_file(&id))?.ok_or("book not found")?;
    let path = PathBuf::from(&file);
    if !path.starts_with(&app.books_dir) {
        return Err("book file is outside the library folder".into());
    }
    Ok(Response::new(std::fs::read(path).map_err(err)?))
}

#[tauri::command]
fn book_progress(app: tauri::State<'_, App>, id: String, progress: f64, location: String) -> Res<()> {
    app.db(|s| s.set_progress(&id, progress, &location))
}

#[tauri::command]
fn book_rate(app: tauri::State<'_, App>, id: String, rating: Option<u8>) -> Res<()> {
    app.db(|s| s.rate_book(&id, rating.map(|r| r.clamp(1, 10))))
}

#[tauri::command]
fn book_remove(app: tauri::State<'_, App>, id: String) -> Res<()> {
    if let Some(f) = app.db(|s| s.remove_book(&id))? {
        let p = PathBuf::from(f);
        if p.starts_with(&app.books_dir) {
            let _ = std::fs::remove_file(p);
        }
    }
    Ok(())
}

// The archive of past editions.

#[tauri::command]
fn archive_list(app: tauri::State<'_, App>) -> Res<Vec<EditionInfo>> {
    app.db(|s| s.editions())
}

#[tauri::command]
fn archive_search(app: tauri::State<'_, App>, query: String) -> Res<Vec<Hit>> {
    app.db(|s| s.search_stories(&query, 60))
}

// Catalogues (OPDS).

const CATALOGUES: &str = "opds";

fn catalogue(app: &App, id: &str) -> Res<Catalogue> {
    let list: Vec<Catalogue> = app.db(|s| s.get_or_default(CATALOGUES))?;
    list.into_iter().find(|c| c.id == id).ok_or_else(|| "that catalogue is no longer in your list".into())
}

fn secret_key(c: &Catalogue) -> String {
    format!("opds-{}", c.id)
}

/// The catalogue's saved password, when it has a login.
fn password(app: &App, c: &Catalogue) -> Res<Option<String>> {
    if c.user.is_empty() { Ok(None) } else { Ok(Some(app.secrets.get(&secret_key(c))?.unwrap_or_default())) }
}

fn login<'a>(c: &'a Catalogue, password: &'a Option<String>) -> Option<opds::Login<'a>> {
    password.as_deref().map(|p| opds::Login { catalogue: &c.url, user: &c.user, password: p })
}

#[tauri::command]
fn opds_presets() -> Vec<Preset> {
    opds::PRESETS.to_vec()
}

#[tauri::command]
fn opds_list(app: tauri::State<'_, App>) -> Res<Vec<Catalogue>> {
    app.db(|s| s.get_or_default(CATALOGUES))
}

/// Add or change a catalogue. It is opened once first, so a wrong address or
/// login is caught here. `password: None` keeps the saved one.
#[tauri::command]
async fn opds_save(app: tauri::State<'_, App>, url: String, title: String, user: String, password: Option<String>) -> Res<Catalogue> {
    let url = url.trim().to_string();
    let parsed = url::Url::parse(&url).map_err(|_| "that is not a web address (it should start with https://)")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("that is not a web address (it should start with https://)".into());
    }
    let mut c = Catalogue { id: format!("c{}", feeds::story_id(&url)), title: title.trim().into(), url, user: user.trim().into() };
    let key = secret_key(&c);
    let saved = if c.user.is_empty() { None } else { app.secrets.get(&key).ok().flatten() };
    let pass = password.or(saved).unwrap_or_default();
    let login = (!c.user.is_empty()).then(|| opds::Login { catalogue: &c.url, user: &c.user, password: &pass });
    let page = opds::browse(&app.client, &c.url, login.as_ref()).await.map_err(err)?;
    if c.title.is_empty() {
        c.title = if page.title.is_empty() { parsed.host_str().unwrap_or("Catalogue").into() } else { page.title.clone() };
    }
    if c.user.is_empty() {
        app.secrets.remove(&key).ok();
    } else {
        app.secrets.set(&key, &pass)?;
    }
    app.db(|s| {
        let mut list: Vec<Catalogue> = s.get_or_default(CATALOGUES)?;
        match list.iter_mut().find(|x| x.id == c.id) {
            Some(x) => *x = c.clone(),
            None => list.push(c.clone()),
        }
        s.set(CATALOGUES, &list)
    })?;
    Ok(c)
}

#[tauri::command]
fn opds_remove(app: tauri::State<'_, App>, id: String) -> Res<()> {
    if let Ok(c) = catalogue(&app, &id) {
        app.secrets.remove(&secret_key(&c)).ok();
    }
    app.db(|s| {
        let mut list: Vec<Catalogue> = s.get_or_default(CATALOGUES)?;
        list.retain(|c| c.id != id);
        s.set(CATALOGUES, &list)
    })
}

/// One page of a catalogue: its start page, a page linked from it, or a search.
#[tauri::command]
async fn opds_browse(
    app: tauri::State<'_, App>,
    id: String,
    url: Option<String>,
    query: Option<String>,
    search: Option<String>,
) -> Res<Page> {
    let c = catalogue(&app, &id)?;
    let target = match (search, query) {
        (Some(t), Some(q)) if !q.trim().is_empty() => opds::fill(&t, &q),
        _ => url.unwrap_or_else(|| c.url.clone()),
    };
    let pass = password(&app, &c)?;
    opds::browse(&app.client, &target, login(&c, &pass).as_ref()).await.map_err(err)
}

/// Download a catalogue book to the shelf. Returns its library id.
#[tauri::command]
async fn opds_get(app: tauri::State<'_, App>, id: String, book: Pub, format: String) -> Res<String> {
    let c = catalogue(&app, &id)?;
    let d = book
        .downloads
        .iter()
        .find(|d| d.format == format)
        .or(book.downloads.first())
        .cloned()
        .ok_or("this book has no file to download")?;
    let pass = password(&app, &c)?;
    let (bytes, f) = opds::download(&app.client, &d, login(&c, &pass).as_ref()).await.map_err(err)?;
    let path = app.books_dir.join(safe_name(&book.id, f));
    std::fs::write(&path, bytes).map_err(err)?;
    let shelved = Book {
        id: book.id.clone(),
        title: if book.title.is_empty() { "Untitled".into() } else { book.title },
        authors: book.authors,
        subjects: vec![],
        summary: book.summary,
        cover: book.cover,
        epub_url: book.downloads.first().map(|d| d.url.clone()),
        source: c.title,
        popularity: 0,
    };
    let j = serde_json::to_string(&shelved).map_err(err)?;
    app.db(|s| s.add_book(&shelved.id, &j, &path.to_string_lossy()))?;
    Ok(book.id)
}

// Highlights and notes.

#[tauri::command]
fn highlight_save(app: tauri::State<'_, App>, highlight: Highlight) -> Res<Highlight> {
    app.db(|s| s.save_highlight(highlight))
}

#[tauri::command]
fn highlights_list(app: tauri::State<'_, App>, book_id: Option<String>) -> Res<Vec<Highlight>> {
    app.db(|s| s.highlights(book_id.as_deref()))
}

#[tauri::command]
fn highlight_delete(app: tauri::State<'_, App>, id: String) -> Res<()> {
    app.db(|s| s.delete_highlight(&id))
}

/// Save highlights as Markdown where the user picks. One book, or all of them.
/// Returns false when the user cancelled.
#[tauri::command]
async fn highlights_export(handle: AppHandle, app: tauri::State<'_, App>, book_id: Option<String>) -> Res<bool> {
    let list = app.db(|s| s.highlights(book_id.as_deref()))?;
    if list.is_empty() {
        return Err("There are no highlights to export yet.".into());
    }
    let stem = match &book_id {
        Some(_) => list[0].book_title.chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect::<String>(),
        None => "granthalaya".into(),
    };
    let stem = stem.split('-').filter(|w| !w.is_empty()).collect::<Vec<_>>().join("-");
    let name = format!("{}-highlights.md", if stem.is_empty() { "book" } else { &stem });
    let dialog = handle.dialog().file().set_file_name(&name).add_filter("Markdown", &["md"]);
    let Some(target) = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file()).await.map_err(err)? else {
        return Ok(false);
    };
    let md = highlights::to_markdown(&list);
    handle
        .fs()
        .open(target, OpenOptions::new().write(true).create(true).truncate(true).clone())
        .and_then(|mut f| f.write_all(md.as_bytes()))
        .map_err(|e| format!("The notes could not be saved there ({e})."))?;
    Ok(true)
}

// Reading comfort.

#[derive(Serialize)]
struct ComfortView {
    settings: Settings,
    question: Option<Question>,
    history: Vec<comfort::Change>,
}

#[tauri::command]
fn comfort_get(app: tauri::State<'_, App>, device: String, daypart: String) -> Res<ComfortView> {
    let st: comfort::State = app.db(|s| s.get_or_default("comfort"))?;
    let history = st.history.iter().rev().take(20).cloned().collect();
    Ok(ComfortView { settings: st.settings(&device, &daypart), question: st.next_question(&device, &daypart).cloned(), history })
}

#[tauri::command]
fn comfort_answer(app: tauri::State<'_, App>, device: String, daypart: String, question: String, answer: usize) -> Res<Settings> {
    app.db(|s| {
        let mut st: comfort::State = s.get_or_default("comfort")?;
        let out = st.answer(&device, &daypart, &question, answer.min(2), &chrono::Utc::now().to_rfc3339());
        s.set("comfort", &st)?;
        Ok(out)
    })
}

#[tauri::command]
fn comfort_set(app: tauri::State<'_, App>, device: String, daypart: String, settings: Settings) -> Res<()> {
    app.db(|s| {
        let mut st: comfort::State = s.get_or_default("comfort")?;
        st.profiles.insert(comfort::profile_key(&device, &daypart), settings);
        s.set("comfort", &st)
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // For scripts and release checks: answers without opening a window.
    if std::env::args().any(|a| a == "--version") {
        println!("Granthalaya {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let books_dir = dir.join("books");
            std::fs::create_dir_all(&books_dir)?;
            let store = Store::open(&dir.join("granthalaya.db"))?;
            app.manage(App {
                store: std::sync::Mutex::new(store),
                client: feeds::client(),
                books_dir,
                secrets: secrets::Secrets::new(&dir),
                building: Mutex::new(()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            edition_today,
            edition_get,
            edition_dates,
            story_open,
            story_feedback,
            feedback_all,
            prefs_get,
            prefs_set,
            topics_list,
            outlets_list,
            taste_get,
            taste_set,
            genres,
            library,
            books_suggest,
            books_search,
            book_add,
            book_import,
            book_bytes,
            book_progress,
            book_rate,
            book_remove,
            comfort_get,
            comfort_answer,
            comfort_set,
            highlight_save,
            highlights_list,
            highlight_delete,
            highlights_export,
            opds_presets,
            opds_list,
            opds_save,
            opds_remove,
            opds_browse,
            opds_get,
            archive_list,
            archive_search
        ])
        .run(tauri::generate_context!())
        .expect("error while running Granthālaya");
}
