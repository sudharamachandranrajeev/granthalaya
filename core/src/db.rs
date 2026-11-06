//! SQLite store. One file in the app data folder.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};

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
];

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
        tx.commit()?;
        Ok(())
    }

    pub fn edition(&self, date: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT json FROM editions WHERE date=?1", [date], |r| r.get(0)).optional()?)
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
            .query_row("SELECT body, report FROM stories WHERE id=?1 AND body IS NOT NULL AND report IS NOT NULL", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    pub fn set_story_body(&self, id: &str, body: &str, report: &str) -> Result<()> {
        self.conn.execute("UPDATE stories SET body=?2, report=?3 WHERE id=?1", params![id, body, report])?;
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
        Ok(self.conn.query_row("SELECT json, fetched_at FROM outlet_facts WHERE qid=?1", [qid], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)
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
        let mut st = self.conn.prepare(
            "SELECT json, file, progress, location, rating, last_read FROM books ORDER BY COALESCE(last_read, added_at) DESC",
        )?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
