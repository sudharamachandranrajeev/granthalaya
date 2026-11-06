//! SQLite store. One file in the app data folder.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::highlights::Highlight;

/// Story id to (vote, stars).
pub type Votes = HashMap<String, (Option<i8>, Option<u8>)>;

pub struct Store {
    conn: Connection,
}

const MIGRATIONS: &[&str] = &[
    // 1: initial schema.
    "CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
     CREATE TABLE weights (feature TEXT PRIMARY KEY, w REAL NOT NULL);
     CREATE TABLE editions (date TEXT PRIMARY KEY, built_at TEXT NOT NULL, json TEXT NOT NULL);
     CREATE TABLE stories (id TEXT PRIMARY KEY, json TEXT NOT NULL, body TEXT, report TEXT);
     CREATE TABLE feedback (story_id TEXT PRIMARY KEY, vote INTEGER, stars INTEGER, at TEXT NOT NULL);
     CREATE TABLE outlet_facts (qid TEXT PRIMARY KEY, json TEXT NOT NULL, fetched_at TEXT NOT NULL);
     CREATE TABLE books (id TEXT PRIMARY KEY, json TEXT NOT NULL, file TEXT NOT NULL, added_at TEXT NOT NULL,
                         progress REAL NOT NULL DEFAULT 0, location TEXT, rating INTEGER, last_read TEXT);",
    // 2: highlights and notes. Not tied to the books table: they outlive a removed book.
    "CREATE TABLE highlights (id TEXT PRIMARY KEY, book_id TEXT NOT NULL, json TEXT NOT NULL,
                              progress REAL NOT NULL, created_at TEXT NOT NULL);
     CREATE INDEX highlights_book ON highlights(book_id, progress);",
    // 3: full-text search over every story in every edition, filled from the papers already kept.
    "CREATE VIRTUAL TABLE story_search USING fts5(story_id UNINDEXED, date UNINDEXED, title, summary, outlet, body,
         tokenize = 'unicode61 remove_diacritics 2');
     -- A story's body is kept as a JSON list of paragraphs; the index gets the plain text.
     INSERT INTO story_search(story_id, date, title, summary, outlet, body)
       SELECT json_extract(s.value, '$.story.id'), e.date, json_extract(s.value, '$.story.title'),
              json_extract(s.value, '$.story.summary'), json_extract(s.value, '$.story.outlet_name'),
              (SELECT group_concat(j.value, char(10)) FROM stories st,
                json_each(CASE WHEN json_valid(st.body) THEN st.body ELSE '[]' END) j WHERE st.id = json_extract(s.value, '$.story.id'))
       FROM editions e, json_each(e.json, '$.pages') p, json_each(p.value, '$.stories') s;",
];

/// A story found in a past edition. `snippet` marks matches with \u{2} … \u{3}.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Hit {
    pub story_id: String,
    pub date: String,
    pub title: String,
    pub outlet: String,
    pub snippet: String,
}

/// One kept edition, for the archive list.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EditionInfo {
    pub date: String,
    pub number: u32,
    pub lead: String,
    pub stories: u32,
}

/// Turn what someone typed into a safe FTS5 query: every word must appear,
/// the last one may be the start of a word.
fn fts_query(q: &str) -> Option<String> {
    let words: Vec<String> =
        q.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).take(12).map(|w| format!("\"{w}\"")).collect();
    if words.is_empty() {
        return None;
    }
    Some(words.join(" ") + "*")
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::migrate(conn)
    }

    pub fn memory() -> Result<Self> {
        Self::migrate(Connection::open_in_memory()?)
    }

    fn migrate(mut conn: Connection) -> Result<Self> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let tx = conn.transaction()?;
        for (i, m) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            tx.execute_batch(m)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        tx.commit()?;
        Ok(Self { conn })
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let v: Option<String> = self.conn.query_row("SELECT value FROM kv WHERE key=?1", [key], |r| r.get(0)).optional()?;
        Ok(match v {
            Some(s) => Some(serde_json::from_str(&s)?),
            None => None,
        })
    }

    pub fn get_or_default<T: DeserializeOwned + Default>(&self, key: &str) -> Result<T> {
        Ok(self.get(key)?.unwrap_or_default())
    }

    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kv(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    pub fn weights(&self) -> Result<HashMap<String, f64>> {
        let mut st = self.conn.prepare("SELECT feature, w FROM weights")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn save_weights(&mut self, changed: &[(String, f64)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for (f, w) in changed {
            tx.execute("INSERT INTO weights(feature,w) VALUES(?1,?2) ON CONFLICT(feature) DO UPDATE SET w=excluded.w", params![f, w])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn save_edition(&mut self, date: &str, json: &str, stories: &[(String, String)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO editions(date,built_at,json) VALUES(?1,?2,?3) ON CONFLICT(date) DO UPDATE SET built_at=excluded.built_at, json=excluded.json",
            params![date, now(), json],
        )?;
        for (id, s) in stories {
            tx.execute("INSERT INTO stories(id,json) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json", params![id, s])?;
        }
        // Reindex this day's paper (a reprint replaces it).
        tx.execute("DELETE FROM story_search WHERE date=?1", [date])?;
        tx.execute(
            "INSERT INTO story_search(story_id, date, title, summary, outlet, body)
               SELECT json_extract(s.value, '$.story.id'), e.date, json_extract(s.value, '$.story.title'),
                      json_extract(s.value, '$.story.summary'), json_extract(s.value, '$.story.outlet_name'),
                      (SELECT group_concat(j.value, char(10)) FROM stories st,
                json_each(CASE WHEN json_valid(st.body) THEN st.body ELSE '[]' END) j WHERE st.id = json_extract(s.value, '$.story.id'))
               FROM editions e, json_each(e.json, '$.pages') p, json_each(p.value, '$.stories') s WHERE e.date=?1",
            [date],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn edition(&self, date: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT json FROM editions WHERE date=?1", [date], |r| r.get(0)).optional()?)
    }

    /// Every kept edition, newest first, with its lead headline.
    pub fn editions(&self) -> Result<Vec<EditionInfo>> {
        let mut st = self.conn.prepare(
            "SELECT e.date, COALESCE(json_extract(e.json, '$.number'), 0), COALESCE(json_extract(e.json, '$.pages[0].stories[0].story.title'), ''),
                    (SELECT COUNT(*) FROM json_each(e.json, '$.pages') p, json_each(p.value, '$.stories'))
             FROM editions e ORDER BY e.date DESC",
        )?;
        Ok(st
            .query_map([], |r| Ok(EditionInfo { date: r.get(0)?, number: r.get(1)?, lead: r.get(2)?, stories: r.get(3)? }))?
            .collect::<Result<_, _>>()?)
    }

    /// Search every story in every kept edition. Best matches first, newer first on ties.
    pub fn search_stories(&self, q: &str, limit: usize) -> Result<Vec<Hit>> {
        let Some(m) = fts_query(q) else { return Ok(vec![]) };
        let mut st = self.conn.prepare(
            "SELECT story_id, date, title, outlet,
                    snippet(story_search, -1, char(2), char(3), '…', 14)
             FROM story_search WHERE story_search MATCH ?1
             ORDER BY bm25(story_search, 0, 0, 8.0, 3.0, 1.0, 1.0), date DESC LIMIT ?2",
        )?;
        Ok(st
            .query_map(params![m, limit as i64], |r| {
                Ok(Hit { story_id: r.get(0)?, date: r.get(1)?, title: r.get(2)?, outlet: r.get(3)?, snippet: r.get(4)? })
            })?
            .collect::<Result<_, _>>()?)
    }

    pub fn edition_dates(&self) -> Result<Vec<String>> {
        let mut st = self.conn.prepare("SELECT date FROM editions ORDER BY date DESC LIMIT 60")?;
        Ok(st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
    }

    pub fn story_json(&self, id: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT json FROM stories WHERE id=?1", [id], |r| r.get(0)).optional()?)
    }

    /// Cached full text and full-text bias report.
    pub fn story_body(&self, id: &str) -> Result<Option<(String, String)>> {
        Ok(self
            .conn
            .query_row("SELECT body, report FROM stories WHERE id=?1 AND body IS NOT NULL AND report IS NOT NULL", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?)
    }

    pub fn set_story_body(&self, id: &str, body: &str, report: &str) -> Result<()> {
        self.conn.execute("UPDATE stories SET body=?2, report=?3 WHERE id=?1", params![id, body, report])?;
        // Opened stories become searchable by their full text too: the paragraphs, not the JSON.
        let text = serde_json::from_str::<Vec<String>>(body).map(|p| p.join("\n")).unwrap_or_else(|_| body.to_string());
        self.conn.execute("UPDATE story_search SET body=?2 WHERE story_id=?1", params![id, text])?;
        Ok(())
    }

    pub fn set_feedback(&self, id: &str, vote: Option<i8>, stars: Option<u8>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO feedback(story_id,vote,stars,at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(story_id) DO UPDATE SET vote=excluded.vote, stars=excluded.stars, at=excluded.at",
            params![id, vote, stars, now()],
        )?;
        Ok(())
    }

    pub fn feedback(&self) -> Result<Votes> {
        let mut st = self.conn.prepare("SELECT story_id, vote, stars FROM feedback")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn feedback_count(&self) -> Result<usize> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM feedback", [], |r| r.get::<_, i64>(0))? as usize)
    }

    pub fn outlet_facts(&self, qid: &str) -> Result<Option<(String, String)>> {
        Ok(self
            .conn
            .query_row("SELECT json, fetched_at FROM outlet_facts WHERE qid=?1", [qid], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    pub fn set_outlet_facts(&self, qid: &str, json: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO outlet_facts(qid,json,fetched_at) VALUES(?1,?2,?3) ON CONFLICT(qid) DO UPDATE SET json=excluded.json, fetched_at=excluded.fetched_at",
            params![qid, json, now()],
        )?;
        Ok(())
    }

    // Books.

    pub fn add_book(&self, id: &str, json: &str, file: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO books(id,json,file,added_at) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET json=excluded.json, file=excluded.file",
            params![id, json, file, now()],
        )?;
        Ok(())
    }

    /// (json, file, progress, location, rating, last_read) for every book, most recent first.
    #[allow(clippy::type_complexity)]
    pub fn books(&self) -> Result<Vec<(String, String, f64, Option<String>, Option<u8>, Option<String>)>> {
        let mut st = self
            .conn
            .prepare("SELECT json, file, progress, location, rating, last_read FROM books ORDER BY COALESCE(last_read, added_at) DESC")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn book_file(&self, id: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT file FROM books WHERE id=?1", [id], |r| r.get(0)).optional()?)
    }

    pub fn set_progress(&self, id: &str, progress: f64, location: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET progress=?2, location=?3, last_read=?4 WHERE id=?1",
            params![id, progress.clamp(0.0, 1.0), location, now()],
        )?;
        Ok(())
    }

    pub fn rate_book(&self, id: &str, rating: Option<u8>) -> Result<()> {
        self.conn.execute("UPDATE books SET rating=?2 WHERE id=?1", params![id, rating])?;
        Ok(())
    }

    pub fn remove_book(&self, id: &str) -> Result<Option<String>> {
        let file = self.book_file(id)?;
        self.conn.execute("DELETE FROM books WHERE id=?1", [id])?;
        Ok(file)
    }

    /// Insert or update; keeps the first created_at. Returns the stored highlight.
    pub fn save_highlight(&self, h: Highlight) -> Result<Highlight> {
        let mut h = h.checked()?;
        let created: Option<String> =
            self.conn.query_row("SELECT created_at FROM highlights WHERE id=?1", [&h.id], |r| r.get(0)).optional()?;
        h.updated_at = now();
        h.created_at = created.unwrap_or_else(|| h.updated_at.clone());
        self.conn.execute(
            "INSERT INTO highlights(id,book_id,json,progress,created_at) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET json=excluded.json, progress=excluded.progress",
            params![h.id, h.book_id, serde_json::to_string(&h)?, h.progress, h.created_at],
        )?;
        Ok(h)
    }

    /// One book's highlights in reading order, or every highlight by book.
    pub fn highlights(&self, book_id: Option<&str>) -> Result<Vec<Highlight>> {
        let mut st =
            self.conn.prepare("SELECT json FROM highlights WHERE ?1 IS NULL OR book_id=?1 ORDER BY book_id, progress, created_at")?;
        let rows = st.query_map([book_id], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for j in rows {
            out.push(serde_json::from_str(&j?)?);
        }
        Ok(out)
    }

    pub fn delete_highlight(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM highlights WHERE id=?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edition_json(date: &str, stories: &[(&str, &str, &str)]) -> String {
        let list: Vec<_> = stories
            .iter()
            .map(|(id, title, summary)| serde_json::json!({"story": {"id": id, "title": title, "summary": summary, "outlet_name": "The Daily Example"}}))
            .collect();
        serde_json::json!({"date": date, "number": 7, "pages": [{"title": "Front", "stories": list}]}).to_string()
    }

    #[test]
    fn searches_past_editions() {
        let mut s = Store::memory().unwrap();
        s.save_edition(
            "2026-11-05",
            &edition_json(
                "2026-11-05",
                &[("a", "Harbour bridge reopens", "Traffic returns after repairs."), ("b", "Café owners protest", "Rent rises again.")],
            ),
            &[],
        )
        .unwrap();
        s.save_edition("2026-11-06", &edition_json("2026-11-06", &[("c", "Bridge tolls set", "A new charge for drivers.")]), &[]).unwrap();
        let hits = s.search_stories("bridge", 10).unwrap();
        assert_eq!(hits.iter().map(|h| h.story_id.as_str()).collect::<Vec<_>>().len(), 2);
        // Accents fold, the last word matches as a prefix, quotes cannot break the query.
        assert_eq!(s.search_stories("cafe prot", 10).unwrap()[0].story_id, "b");
        assert!(s.search_stories("\"OR(", 10).unwrap().is_empty());
        assert!(s.search_stories("  ", 10).unwrap().is_empty());
        let h = &s.search_stories("tolls", 10).unwrap()[0];
        assert_eq!((h.date.as_str(), h.outlet.as_str()), ("2026-11-06", "The Daily Example"));
        assert!(h.snippet.contains("\u{2}tolls\u{3}") || h.snippet.contains("\u{2}Tolls\u{3}"), "{}", h.snippet);
        // A reprint replaces that day's index; the full text of an opened story is searchable.
        s.save_edition(
            "2026-11-06",
            &edition_json("2026-11-06", &[("c", "Bridge tolls set", "A new charge.")]),
            &[("c".into(), "{}".into())],
        )
        .unwrap();
        assert_eq!(s.search_stories("tolls", 10).unwrap().len(), 1);
        s.set_story_body("c", r#"["The minister spoke.","About lighthouses."]"#, "{}").unwrap();
        let hit = &s.search_stories("lighthouses", 10).unwrap()[0];
        assert_eq!(hit.story_id, "c");
        assert!(!hit.snippet.contains(['[', '"']), "the index holds paragraphs, not JSON: {}", hit.snippet);
        let eds = s.editions().unwrap();
        assert_eq!((eds[0].date.as_str(), eds[0].number, eds[0].lead.as_str(), eds[0].stories), ("2026-11-06", 7, "Bridge tolls set", 1));
    }

    #[test]
    fn backfills_search_for_old_editions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute(
            "INSERT INTO editions(date,built_at,json) VALUES('2026-10-01','x',?1)",
            [edition_json("2026-10-01", &[("z", "Old lighthouse news", "")])],
        )
        .unwrap();
        conn.execute("INSERT INTO stories(id,json,body) VALUES('z','{}','[\"Keepers met at the quay.\"]')", []).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        let s = Store::migrate(conn).unwrap();
        assert_eq!(s.search_stories("lighthouse", 5).unwrap()[0].date, "2026-10-01");
        let hit = &s.search_stories("quay", 5).unwrap()[0];
        assert!(hit.snippet.starts_with("Keepers met"), "body indexed as plain text: {}", hit.snippet);
    }

    #[test]
    fn keeps_highlights_after_the_book_goes() {
        let s = Store::memory().unwrap();
        s.add_book("b", "{}", "/x.epub").unwrap();
        let h = Highlight {
            book_id: "b".into(),
            book_title: "T".into(),
            cfi: "c1".into(),
            text: "Hi".into(),
            color: "blue".into(),
            progress: 0.5,
            ..Default::default()
        };
        let first = s.save_highlight(h.clone()).unwrap();
        let again = s.save_highlight(Highlight { note: "n".into(), ..h.clone() }).unwrap();
        assert_eq!((first.id.clone(), first.created_at.clone()), (again.id.clone(), again.created_at.clone()));
        s.save_highlight(Highlight { cfi: "c0".into(), progress: 0.1, ..h }).unwrap();
        s.remove_book("b").unwrap();
        let all = s.highlights(Some("b")).unwrap();
        assert_eq!(all.iter().map(|x| x.cfi.as_str()).collect::<Vec<_>>(), ["c0", "c1"]);
        assert_eq!(all[1].note, "n");
        assert_eq!(s.highlights(None).unwrap().len(), 2);
        s.delete_highlight(&all[0].id).unwrap();
        assert_eq!(s.highlights(None).unwrap().len(), 1);
    }

    #[test]
    fn migrates_and_round_trips() {
        let mut s = Store::memory().unwrap();
        s.set("muted", &vec!["sport"]).unwrap();
        assert_eq!(s.get::<Vec<String>>("muted").unwrap().unwrap(), vec!["sport"]);
        s.save_weights(&[("t:science".into(), 0.4)]).unwrap();
        assert_eq!(s.weights().unwrap()["t:science"], 0.4);
        s.save_edition("2026-11-06", "{}", &[("a".into(), "{}".into())]).unwrap();
        s.set_feedback("a", Some(1), Some(9)).unwrap();
        s.set_feedback("a", Some(-1), None).unwrap();
        assert_eq!(s.feedback().unwrap()["a"], (Some(-1), None));
        s.add_book("b", "{}", "/x.epub").unwrap();
        s.set_progress("b", 1.7, "cfi").unwrap();
        assert_eq!(s.books().unwrap()[0].2, 1.0);
    }
}
