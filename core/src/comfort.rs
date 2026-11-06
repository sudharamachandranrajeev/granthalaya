//! Self-tuning reading comfort. The reader answers one short question now and
//! then; each answer nudges one setting for this kind of device and time of day.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// Text size in CSS pixels.
    pub font_size: f64,
    pub line_height: f64,
    /// Side margin as a share of page width.
    pub margin: f64,
    /// 0 = neutral grey paper, 1 = warm candle-light paper.
    pub warmth: f64,
    /// 0 = soft grey ink, 1 = near-black ink. Never pure black on pure white.
    pub contrast: f64,
    /// Paper lightness 0 (dim) to 1 (bright).
    pub lightness: f64,
    pub font: String,
}

impl Settings {
    pub fn default_for(device: &str, daypart: &str) -> Self {
        let night = daypart == "night";
        Settings {
            font_size: match device { "phone" => 18.0, "tablet" => 20.0, _ => 20.0 },
            line_height: 1.55,
            margin: if device == "phone" { 0.06 } else { 0.09 },
            warmth: if night { 0.65 } else { 0.25 },
            contrast: 0.7,
            lightness: if night { 0.45 } else { 0.85 },
            font: "Literata".into(),
        }
    }

    fn clamp(&mut self) {
        self.font_size = self.font_size.clamp(13.0, 34.0);
        self.line_height = self.line_height.clamp(1.2, 2.1);
        self.margin = self.margin.clamp(0.02, 0.18);
        self.warmth = self.warmth.clamp(0.0, 1.0);
        self.contrast = self.contrast.clamp(0.3, 0.95);
        self.lightness = self.lightness.clamp(0.2, 0.95);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: &'static str,
    pub prompt: &'static str,
    /// Three answers: less, just right, more.
    pub answers: [&'static str; 3],
}

pub const QUESTIONS: &[Question] = &[
    Question { id: "size", prompt: "How is the size of the text?", answers: ["Too small", "Just right", "Too large"] },
    Question { id: "brightness", prompt: "How does the page feel on your eyes?", answers: ["Too dim", "Comfortable", "Too bright"] },
    Question { id: "contrast", prompt: "Are the letters easy to make out?", answers: ["Too faint", "Easy", "Too harsh"] },
    Question { id: "spacing", prompt: "How do the lines feel?", answers: ["Cramped", "Just right", "Too spread out"] },
    Question { id: "margins", prompt: "How wide are the margins?", answers: ["Too narrow", "Just right", "Too wide"] },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub at: String,
    pub profile: String,
    pub question: String,
    pub answer: String,
    pub summary: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    pub profiles: HashMap<String, Settings>,
    pub history: Vec<Change>,
    /// "profile/question" -> consecutive "just right" answers.
    pub settled: HashMap<String, u32>,
    /// "profile/question" -> times asked.
    pub asked: HashMap<String, u32>,
}

pub fn profile_key(device: &str, daypart: &str) -> String {
    format!("{device}/{daypart}")
}

impl State {
    pub fn settings(&self, device: &str, daypart: &str) -> Settings {
        self.profiles.get(&profile_key(device, daypart)).cloned().unwrap_or_else(|| Settings::default_for(device, daypart))
    }

    /// Next question worth asking, or none when every setting is settled.
    /// Settled means two "just right" answers in a row.
    pub fn next_question(&self, device: &str, daypart: &str) -> Option<&'static Question> {
        let p = profile_key(device, daypart);
        QUESTIONS
            .iter()
            .filter(|q| self.settled.get(&format!("{p}/{}", q.id)).copied().unwrap_or(0) < 2)
            .min_by_key(|q| self.asked.get(&format!("{p}/{}", q.id)).copied().unwrap_or(0))
    }

    /// Apply an answer: 0 = less, 1 = just right, 2 = more. Returns new settings.
    pub fn answer(&mut self, device: &str, daypart: &str, question: &str, answer: usize, at: &str) -> Settings {
        let p = profile_key(device, daypart);
        let key = format!("{p}/{question}");
        *self.asked.entry(key.clone()).or_default() += 1;
        let mut s = self.settings(device, daypart);
        let before = s.clone();
        let dir = answer as f64 - 1.0; // -1, 0, +1
        match (question, answer) {
            (_, 1) => {
                *self.settled.entry(key.clone()).or_default() += 1;
            }
            ("size", _) => s.font_size -= dir * 1.5,
            ("brightness", _) => {
                s.lightness -= dir * 0.1;
                if dir > 0.0 {
                    s.warmth += 0.1;
                }
            }
            ("contrast", _) => s.contrast -= dir * 0.08,
            ("spacing", _) => s.line_height += -dir * 0.1,
            ("margins", _) => s.margin += -dir * 0.02,
            _ => {}
        }
        if answer != 1 {
            self.settled.insert(key, 0);
        }
        s.clamp();
        let summary = describe(&before, &s);
        if let Some(q) = QUESTIONS.iter().find(|q| q.id == question) {
            self.history.push(Change { at: at.into(), profile: p.clone(), question: q.prompt.into(), answer: q.answers[answer.min(2)].into(), summary });
            if self.history.len() > 200 {
                self.history.remove(0);
            }
        }
        self.profiles.insert(p, s.clone());
        s
    }
}

fn describe(a: &Settings, b: &Settings) -> String {
    let mut parts = Vec::new();
    if (a.font_size - b.font_size).abs() > 0.01 {
        parts.push(format!("text size {:.0} → {:.0}px", a.font_size, b.font_size));
    }
    if (a.lightness - b.lightness).abs() > 0.001 {
        parts.push(format!("page {}", if b.lightness < a.lightness { "dimmer" } else { "brighter" }));
    }
    if (a.warmth - b.warmth).abs() > 0.001 {
        parts.push("warmer paper".to_string());
    }
    if (a.contrast - b.contrast).abs() > 0.001 {
        parts.push(format!("ink {}", if b.contrast > a.contrast { "darker" } else { "softer" }));
    }
    if (a.line_height - b.line_height).abs() > 0.001 {
        parts.push(format!("line spacing {:.2} → {:.2}", a.line_height, b.line_height));
    }
    if (a.margin - b.margin).abs() > 0.0001 {
        parts.push(format!("margins {}", if b.margin > a.margin { "wider" } else { "narrower" }));
    }
    if parts.is_empty() { "No change".into() } else { parts.join(", ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn too_small_makes_text_bigger_and_logs_it() {
        let mut s = State::default();
        let before = s.settings("phone", "day").font_size;
        let after = s.answer("phone", "day", "size", 0, "t");
        assert!(after.font_size > before);
        assert!(s.history[0].summary.contains("text size"));
        // Other profiles are untouched.
        assert_eq!(s.settings("desktop", "day"), Settings::default_for("desktop", "day"));
    }

    #[test]
    fn too_bright_dims_and_warms_and_two_just_rights_settle() {
        let mut s = State::default();
        let b = s.settings("desktop", "night");
        let a = s.answer("desktop", "night", "brightness", 2, "t");
        assert!(a.lightness < b.lightness && a.warmth > b.warmth);
        s.answer("desktop", "night", "size", 1, "t");
        s.answer("desktop", "night", "size", 1, "t");
        let mut asked = 0;
        while let Some(q) = s.next_question("desktop", "night") {
            assert_ne!(q.id, "size");
            s.answer("desktop", "night", q.id, 1, "t");
            asked += 1;
            assert!(asked <= 8);
        }
        assert_eq!(asked, 8);
    }

    #[test]
    fn settings_stay_in_safe_ranges() {
        let mut s = State::default();
        for _ in 0..50 {
            s.answer("phone", "day", "size", 2, "t");
            s.answer("phone", "day", "contrast", 0, "t");
        }
        let x = s.settings("phone", "day");
        assert_eq!(x.font_size, 13.0);
        assert_eq!(x.contrast, 0.95);
    }
}
