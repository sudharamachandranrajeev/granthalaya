//! Highlights and notes. Each one is stored the moment it is made, with its whole
//! text and the book's title and author, so an export is never clipped and still
//! works after the book has left the shelf.

use serde::{Deserialize, Serialize};

pub const COLORS: [&str; 4] = ["yellow", "green", "blue", "pink"];
/// A selection longer than this is not a highlight but a copy of the book.
pub const MAX_TEXT: usize = 20_000;
pub const MAX_NOTE: usize = 10_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Highlight {
    /// Same book and passage give the same id, so marking it again updates it.
    #[serde(default)]
    pub id: String,
    pub book_id: String,
    pub book_title: String,
    pub book_author: String,
    /// EPUB CFI of the passage.
    pub cfi: String,
    pub text: String,
    #[serde(default)]
    pub note: String,
    pub color: String,
    #[serde(default)]
    pub chapter: String,
    /// Fraction of the book, 0..1, for ordering.
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

impl Highlight {
    /// Check and tidy one coming from the reader. Text is kept whole.
    pub fn checked(mut self) -> anyhow::Result<Self> {
        if self.book_id.is_empty() || self.cfi.is_empty() {
            anyhow::bail!("a highlight needs a book and a place in it");
        }
        if self.text.trim().is_empty() {
            anyhow::bail!("a highlight needs some text");
        }
        if self.text.chars().count() > MAX_TEXT {
            anyhow::bail!("that selection is too long to keep as a highlight");
        }
        if self.note.chars().count() > MAX_NOTE {
            anyhow::bail!("that note is too long");
        }
        if !COLORS.contains(&self.color.as_str()) {
            self.color = COLORS[0].into();
        }
        self.progress = self.progress.clamp(0.0, 1.0);
        self.id = id_for(&self.book_id, &self.cfi);
        Ok(self)
    }
}

pub fn id_for(book_id: &str, cfi: &str) -> String {
    // FNV-1a, 64-bit: stable across runs and platforms.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in book_id.bytes().chain([0]).chain(cfi.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("h{h:016x}")
}

/// Plain Markdown: one section per book, passages in reading order as quotes,
/// notes under them. Works in any notes app.
pub fn to_markdown(list: &[Highlight]) -> String {
    let mut books: Vec<(&str, Vec<&Highlight>)> = Vec::new();
    for h in list {
        match books.iter_mut().find(|(id, _)| *id == h.book_id) {
            Some((_, v)) => v.push(h),
            None => books.push((&h.book_id, vec![h])),
        }
    }
    let mut out = String::new();
    for (_, mut hs) in books {
        hs.sort_by(|a, b| a.progress.total_cmp(&b.progress));
        let first = hs[0];
        out.push_str(&format!("# {}\n\n", one_line(&first.book_title)));
        if !first.book_author.is_empty() {
            out.push_str(&format!("*{}*\n\n", one_line(&first.book_author)));
        }
        let mut chapter = "";
        for h in hs {
            if !h.chapter.is_empty() && h.chapter != chapter {
                chapter = &h.chapter;
                out.push_str(&format!("## {}\n\n", one_line(chapter)));
            }
            for line in h.text.trim().lines() {
                let line = line.trim();
                out.push_str(if line.is_empty() { ">" } else { "> " });
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
            if !h.note.trim().is_empty() {
                out.push_str(h.note.trim());
                out.push_str("\n\n");
            }
            let day = h.created_at.get(..10).unwrap_or("");
            out.push_str(&format!(
                "<sub>{} highlight{}</sub>\n\n",
                h.color,
                if day.is_empty() { String::new() } else { format!(", {day}") }
            ));
        }
    }
    out
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(book: &str, cfi: &str, text: &str, progress: f64) -> Highlight {
        Highlight {
            book_id: book.into(),
            book_title: format!("Title {book}"),
            book_author: "A. Writer".into(),
            cfi: cfi.into(),
            text: text.into(),
            color: "green".into(),
            chapter: "One".into(),
            progress,
            created_at: "2026-11-06T10:00:00Z".into(),
            ..Default::default()
        }
    }

    #[test]
    fn checks_and_keys_by_passage() {
        let a = h("b1", "epubcfi(/6/4!/4/2,/1:0,/1:5)", "Hello", 0.1).checked().unwrap();
        let b = h("b1", "epubcfi(/6/4!/4/2,/1:0,/1:5)", "Hello again", 0.1).checked().unwrap();
        assert_eq!(a.id, b.id);
        assert_ne!(a.id, h("b2", &a.cfi, "Hello", 0.1).checked().unwrap().id);
        let odd = Highlight { color: "purple".into(), ..h("b1", "c", "x", 3.0) }.checked().unwrap();
        assert_eq!((odd.color.as_str(), odd.progress), ("yellow", 1.0));
        assert!(h("b1", "", "x", 0.0).checked().is_err());
        assert!(h("b1", "c", "  ", 0.0).checked().is_err());
        assert!(h("b1", "c", &"x".repeat(MAX_TEXT + 1), 0.0).checked().is_err());
    }

    #[test]
    fn markdown_keeps_whole_text_in_reading_order() {
        let long = "word ".repeat(2000);
        let mut later = h("b1", "c2", "Second\n\nparagraph", 0.8);
        later.note = "My thought.".into();
        let md = to_markdown(&[later, h("b1", "c1", &long, 0.2), h("b2", "c3", "Other book", 0.5)]);
        assert!(md.starts_with("# Title b1\n\n*A. Writer*\n\n## One\n\n> word"));
        assert!(md.contains(long.trim()), "text must not be clipped");
        assert!(md.find("> word").unwrap() < md.find("> Second").unwrap());
        assert!(md.contains("> Second\n>\n> paragraph\n\nMy thought.\n\n<sub>green highlight, 2026-11-06</sub>"));
        assert!(md.contains("# Title b2"));
    }
}
