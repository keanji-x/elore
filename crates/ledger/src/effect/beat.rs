//! Beat — atomic commit unit: text + effects bundled together.
//!
//! Each beat is stored as `beats/{phase}_{seq:03}.json`.
//! Revisions are stored as `beats/{phase}_{seq:03}_r{rev}.json`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::effect::op::Op;
use crate::LedgerError;

/// A single beat: one writing iteration's output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beat {
    pub phase: String,
    pub seq: u32,

    /// If this beat revises an earlier one, its seq number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revises: Option<u32>,

    /// Revision number (0 = original)
    #[serde(default)]
    pub revision: u32,

    pub text: String,

    #[serde(default)]
    pub effects: Vec<Op>,

    pub word_count: u32,

    #[serde(default = "default_creator")]
    pub created_by: String,

    #[serde(default)]
    pub created_at: String,

    /// Reason for revision (if this is a revision)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_reason: Option<String>,
}

fn default_creator() -> String {
    "ai".to_string()
}

impl Beat {
    /// Count CJK + Latin words in text.
    pub fn count_words(text: &str) -> u32 {
        let mut count = 0u32;
        let mut in_latin = false;
        for ch in text.chars() {
            if ch.is_ascii_alphanumeric() {
                if !in_latin {
                    count += 1;
                    in_latin = true;
                }
            } else {
                in_latin = false;
                // CJK characters count as 1 word each
                if ('\u{4e00}'..='\u{9fff}').contains(&ch)
                    || ('\u{3400}'..='\u{4dbf}').contains(&ch)
                    || ('\u{f900}'..='\u{faff}').contains(&ch)
                {
                    count += 1;
                }
            }
        }
        count
    }

    /// Generate the filename for this beat.
    pub fn filename(&self) -> String {
        if self.revises.is_some() {
            format!("{}_{:03}_r{}.json", self.phase, self.seq, self.revision)
        } else {
            format!("{}_{:03}.json", self.phase, self.seq)
        }
    }

    /// Save beat to the beats directory.
    pub fn save(&self, beats_dir: &Path) -> Result<(), LedgerError> {
        std::fs::create_dir_all(beats_dir)?;
        let path = beats_dir.join(self.filename());
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| LedgerError::Parse(format!("Beat serialize: {e}")))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load all beats for a phase, returning the latest revision of each seq.
    pub fn load_phase(beats_dir: &Path, phase_id: &str) -> Vec<Beat> {
        let mut all: Vec<Beat> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(beats_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if stem.starts_with(&format!("{phase_id}_")) {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if let Ok(beat) = serde_json::from_str::<Beat>(&content) {
                                    all.push(beat);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort by (seq, revision) and keep only latest revision per seq
        all.sort_by_key(|b| (b.seq, b.revision));
        let mut latest: std::collections::BTreeMap<u32, Beat> = std::collections::BTreeMap::new();
        for beat in all {
            latest.insert(beat.seq, beat);
        }
        latest.into_values().collect()
    }

    /// Next sequence number for a phase.
    pub fn next_seq(beats_dir: &Path, phase_id: &str) -> u32 {
        let beats = Self::load_phase(beats_dir, phase_id);
        beats.iter().map(|b| b.seq).max().unwrap_or(0) + 1
    }

    /// Total word count across all beats in a phase.
    pub fn total_words(beats: &[Beat]) -> u32 {
        beats.iter().map(|b| b.word_count).sum()
    }

    /// Extract all effects from beats as a flat list.
    pub fn all_effects(beats: &[Beat]) -> Vec<Op> {
        beats.iter().flat_map(|b| b.effects.clone()).collect()
    }

    /// Convert beats to HistoryEntry format for Snapshot::build compatibility.
    pub fn to_history_entries(beats: &[Beat]) -> Vec<crate::HistoryEntry> {
        let mut entries = Vec::new();
        for beat in beats {
            for (i, op) in beat.effects.iter().enumerate() {
                entries.push(crate::HistoryEntry {
                    chapter: beat.phase.clone(),
                    seq: beat.seq * 100 + (i as u32), // deterministic ordering
                    effect: op.clone(),
                });
            }
        }
        entries
    }

    /// Convert this single beat's effects into HistoryEntry records,
    /// ready to be appended to `history.jsonl`.
    pub fn as_history_entries(&self) -> Vec<crate::HistoryEntry> {
        self.effects
            .iter()
            .enumerate()
            .map(|(i, op)| crate::HistoryEntry {
                chapter: self.phase.clone(),
                seq: self.seq * 100 + (i as u32),
                effect: op.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_count_cjk() {
        assert_eq!(Beat::count_words("基安穿过了沙暴"), 7);
    }

    #[test]
    fn word_count_mixed() {
        // kian(1) + 穿(2) + 过(3) + 了(4) + oasis(5) + 的(6) + 大(7) + 门(8)
        assert_eq!(Beat::count_words("kian 穿过了 oasis 的大门"), 8);
    }

    #[test]
    fn word_count_latin() {
        assert_eq!(Beat::count_words("hello world foo"), 3);
    }

    #[test]
    fn beat_serde_minimal() {
        let json = r#"{
            "phase": "setup",
            "seq": 1,
            "text": "沙暴如刀割",
            "word_count": 5
        }"#;
        let beat: Beat = serde_json::from_str(json).unwrap();
        assert_eq!(beat.phase, "setup");
        assert_eq!(beat.seq, 1);
        assert!(beat.revises.is_none());
        assert_eq!(beat.revision, 0);
        assert_eq!(beat.created_by, "ai");
        assert!(beat.effects.is_empty());
    }

    #[test]
    fn beat_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let beat = Beat {
            phase: "test".into(),
            seq: 1,
            revises: None,
            revision: 0,
            text: "测试文本".into(),
            effects: vec![],
            word_count: 4,
            created_by: "ai".into(),
            created_at: "2026-01-01".into(),
            revision_reason: None,
        };
        beat.save(dir.path()).unwrap();

        let loaded = Beat::load_phase(dir.path(), "test");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "测试文本");
    }

    #[test]
    fn beat_revision_latest_only() {
        let dir = tempfile::tempdir().unwrap();

        // Original beat
        let b1 = Beat {
            phase: "test".into(), seq: 1, revises: None, revision: 0,
            text: "原始".into(), effects: vec![], word_count: 2,
            created_by: "ai".into(), created_at: String::new(), revision_reason: None,
        };
        b1.save(dir.path()).unwrap();

        // Revision of beat 1
        let b1r = Beat {
            phase: "test".into(), seq: 1, revises: Some(1), revision: 1,
            text: "修订版".into(), effects: vec![], word_count: 3,
            created_by: "ai".into(), created_at: String::new(),
            revision_reason: Some("改进".into()),
        };
        b1r.save(dir.path()).unwrap();

        let loaded = Beat::load_phase(dir.path(), "test");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "修订版"); // latest revision
        assert_eq!(loaded[0].revision, 1);
    }

    #[test]
    fn beat_filename() {
        let b = Beat {
            phase: "setup".into(), seq: 3, revises: None, revision: 0,
            text: String::new(), effects: vec![], word_count: 0,
            created_by: "ai".into(), created_at: String::new(), revision_reason: None,
        };
        assert_eq!(b.filename(), "setup_003.json");

        let br = Beat {
            phase: "setup".into(), seq: 3, revises: Some(3), revision: 2,
            text: String::new(), effects: vec![], word_count: 0,
            created_by: "ai".into(), created_at: String::new(), revision_reason: None,
        };
        assert_eq!(br.filename(), "setup_003_r2.json");
    }

    #[test]
    fn next_seq_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Beat::next_seq(dir.path(), "empty"), 1);
    }
}
