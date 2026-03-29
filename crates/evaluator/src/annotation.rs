//! Annotation — quality feedback on individual beats.
//!
//! Both AI and human can annotate beats with tags, scores, and notes.

use std::io::{BufRead, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// A quality annotation on a single beat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub beat: u32,
    #[serde(default = "default_by")]
    pub by: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub score: u8,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_by() -> String {
    "human".to_string()
}

/// Load annotations for a phase from `annotations/{phase}.jsonl`.
pub fn load_annotations(annotations_dir: &Path, phase: &str) -> Vec<Annotation> {
    let path = annotations_dir.join(format!("{phase}.jsonl"));
    if !path.exists() {
        return Vec::new();
    }
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = std::io::BufReader::new(file);
    reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Annotation>(&line).ok())
        .collect()
}

/// Append an annotation to `annotations/{phase}.jsonl`.
pub fn add_annotation(
    annotations_dir: &Path,
    phase: &str,
    ann: &Annotation,
) -> Result<(), crate::EvaluatorError> {
    std::fs::create_dir_all(annotations_dir)?;
    let path = annotations_dir.join(format!("{phase}.jsonl"));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let line = serde_json::to_string(ann)?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// Calculate average score across annotations.
pub fn avg_score(annotations: &[Annotation]) -> f32 {
    if annotations.is_empty() {
        return 0.0;
    }
    let sum: u32 = annotations.iter().map(|a| a.score as u32).sum();
    sum as f32 / annotations.len() as f32
}

/// Find beats with score ≤ threshold.
pub fn low_beats(annotations: &[Annotation], threshold: u8) -> Vec<(u32, u8, Option<String>)> {
    // Get worst score per beat
    let mut worst: std::collections::BTreeMap<u32, (u8, Option<String>)> =
        std::collections::BTreeMap::new();
    for ann in annotations {
        let entry = worst
            .entry(ann.beat)
            .or_insert((ann.score, ann.note.clone()));
        if ann.score < entry.0 {
            *entry = (ann.score, ann.note.clone());
        }
    }
    worst
        .into_iter()
        .filter(|(_, (score, _))| *score <= threshold)
        .map(|(beat, (score, note))| (beat, score, note))
        .collect()
}

/// Check if any annotation has a specific tag.
pub fn has_tag(annotations: &[Annotation], tag: &str) -> bool {
    annotations
        .iter()
        .any(|a| a.tags.contains(&tag.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_serde() {
        let json = r#"{"beat":1,"score":4,"tags":["exciting"]}"#;
        let ann: Annotation = serde_json::from_str(json).unwrap();
        assert_eq!(ann.beat, 1);
        assert_eq!(ann.score, 4);
        assert_eq!(ann.by, "human");
    }

    #[test]
    fn avg_score_calculation() {
        let anns = vec![
            Annotation {
                beat: 1,
                by: "ai".into(),
                tags: vec![],
                score: 4,
                note: None,
            },
            Annotation {
                beat: 2,
                by: "ai".into(),
                tags: vec![],
                score: 2,
                note: None,
            },
        ];
        assert!((avg_score(&anns) - 3.0).abs() < 0.01);
    }

    #[test]
    fn low_beats_filter() {
        let anns = vec![
            Annotation {
                beat: 1,
                by: "h".into(),
                tags: vec![],
                score: 4,
                note: None,
            },
            Annotation {
                beat: 2,
                by: "h".into(),
                tags: vec!["boring".into()],
                score: 2,
                note: Some("太慢".into()),
            },
            Annotation {
                beat: 3,
                by: "h".into(),
                tags: vec![],
                score: 1,
                note: None,
            },
        ];
        let low = low_beats(&anns, 2);
        assert_eq!(low.len(), 2);
        assert_eq!(low[0].0, 2); // beat 2
        assert_eq!(low[1].0, 3); // beat 3
    }

    #[test]
    fn has_tag_check() {
        let anns = vec![Annotation {
            beat: 1,
            by: "h".into(),
            tags: vec!["twist".into(), "visual".into()],
            score: 5,
            note: None,
        }];
        assert!(has_tag(&anns, "twist"));
        assert!(!has_tag(&anns, "boring"));
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let ann = Annotation {
            beat: 1,
            by: "ai".into(),
            tags: vec!["exciting".into()],
            score: 5,
            note: Some("很棒".into()),
        };
        add_annotation(dir.path(), "test", &ann).unwrap();

        let loaded = load_annotations(dir.path(), "test");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].score, 5);
        assert_eq!(loaded[0].note.as_deref(), Some("很棒"));
    }
}
