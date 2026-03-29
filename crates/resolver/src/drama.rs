//! Drama Node — per-chapter dramatic plan.
//!
//! Each chapter has a corresponding `DramaNode` loaded from
//! `.everlore/drama/ch03.yaml`. The Drama Node declares what the Director
//! wants to accomplish, how the pacing should flow, and what effects
//! are required vs. suggested.

use std::path::Path;

use serde::{Deserialize, Serialize};

use ledger::Op;

use crate::ResolverError;
use crate::intent::DramaticIntent;

// ══════════════════════════════════════════════════════════════════
// Data model
// ══════════════════════════════════════════════════════════════════

/// Per-chapter dramatic plan — the Director's blueprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DramaNode {
    pub chapter: String,

    #[serde(default)]
    pub dramatic_intents: Vec<DramaticIntent>,

    #[serde(default)]
    pub pacing: Pacing,

    #[serde(default)]
    pub director_notes: DirectorNotes,
}

/// Pacing curve — normalized floats summing to ~1.0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pacing {
    #[serde(default = "default_pacing")]
    pub build_up: f32,
    #[serde(default = "default_pacing")]
    pub climax: f32,
    #[serde(default = "default_pacing")]
    pub resolution: f32,
}

fn default_pacing() -> f32 {
    0.33
}

impl Default for Pacing {
    fn default() -> Self {
        Self {
            build_up: 0.4,
            climax: 0.4,
            resolution: 0.2,
        }
    }
}

/// Director's additional notes for the Author.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectorNotes {
    /// Effects that MUST appear in the chapter text.
    #[serde(default)]
    pub required_effects: Vec<Op>,

    /// Effects the Author CAN include but isn't forced to.
    #[serde(default)]
    pub suggested_effects: Vec<Op>,

    /// Key moments / beats to hit.
    #[serde(default)]
    pub highlights: Vec<String>,

    /// POV character for this chapter.
    #[serde(default)]
    pub pov: Option<String>,

    /// POV constraints (what must NOT be shown).
    #[serde(default)]
    pub pov_constraints: Vec<String>,

    /// Tone / mood.
    #[serde(default)]
    pub tone: Option<String>,

    /// Tone arc (e.g., "tense → hopeful → devastating").
    #[serde(default)]
    pub tone_arc: Option<String>,

    /// Word count target.
    #[serde(default)]
    pub word_count: Option<usize>,
}

// ══════════════════════════════════════════════════════════════════
// Loading
// ══════════════════════════════════════════════════════════════════

/// Load a DramaNode from `.everlore/drama/<chapter>.yaml`.
pub fn load_drama(everlore_dir: &Path, chapter: &str) -> Result<DramaNode, ResolverError> {
    let drama_dir = everlore_dir.join("drama");
    let path = drama_dir.join(format!("{chapter}.yaml"));

    if !path.exists() {
        // Return a minimal default drama node
        return Ok(DramaNode {
            chapter: chapter.to_string(),
            dramatic_intents: vec![],
            pacing: Pacing::default(),
            director_notes: DirectorNotes::default(),
        });
    }

    let content = std::fs::read_to_string(&path)?;
    let node: DramaNode = serde_yaml::from_str(&content)?;
    Ok(node)
}

/// List all drama node files (chapter names) in the drama directory.
pub fn list_drama_chapters(everlore_dir: &Path) -> Vec<String> {
    let drama_dir = everlore_dir.join("drama");
    if !drama_dir.exists() {
        return vec![];
    }

    let mut chapters = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&drama_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_yaml = path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml");
            if is_yaml && let Some(stem) = path.file_stem() {
                chapters.push(stem.to_string_lossy().to_string());
            }
        }
    }
    chapters.sort();
    chapters
}

/// Save a DramaNode to `.everlore/drama/<chapter>.yaml`.
pub fn save_drama(everlore_dir: &Path, node: &DramaNode) -> Result<(), ResolverError> {
    let drama_dir = everlore_dir.join("drama");
    std::fs::create_dir_all(&drama_dir)?;

    let path = drama_dir.join(format!("{}.yaml", node.chapter));
    let yaml = serde_yaml::to_string(node)?;
    std::fs::write(&path, yaml)?;
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// Display
// ══════════════════════════════════════════════════════════════════

impl DramaNode {
    /// Render as a human-readable summary.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("═══ Drama: {} ═══\n\n", self.chapter));

        if let Some(pov) = &self.director_notes.pov {
            out.push_str(&format!("POV: {pov}\n"));
        }
        if let Some(tone) = &self.director_notes.tone {
            out.push_str(&format!("基调: {tone}\n"));
        }
        if let Some(arc) = &self.director_notes.tone_arc {
            out.push_str(&format!("情绪弧: {arc}\n"));
        }
        out.push_str(&format!(
            "节奏: build_up={:.0}% climax={:.0}% resolution={:.0}%\n\n",
            self.pacing.build_up * 100.0,
            self.pacing.climax * 100.0,
            self.pacing.resolution * 100.0,
        ));

        if !self.dramatic_intents.is_empty() {
            out.push_str("戏剧性意图:\n");
            for (i, intent) in self.dramatic_intents.iter().enumerate() {
                out.push_str(&format!("  {}. {}\n", i + 1, intent.summary()));
            }
            out.push('\n');
        }

        if !self.director_notes.required_effects.is_empty() {
            out.push_str("必须执行的 effects:\n");
            for op in &self.director_notes.required_effects {
                out.push_str(&format!("  ✓ {}\n", op.describe()));
            }
            out.push('\n');
        }

        if !self.director_notes.suggested_effects.is_empty() {
            out.push_str("建议的 effects:\n");
            for op in &self.director_notes.suggested_effects {
                out.push_str(&format!("  ~ {}\n", op.describe()));
            }
            out.push('\n');
        }

        if !self.director_notes.highlights.is_empty() {
            out.push_str("关键节拍:\n");
            for h in &self.director_notes.highlights {
                out.push_str(&format!("  ★ {h}\n"));
            }
            out.push('\n');
        }

        out
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{GoalOutcome, Timing};

    #[test]
    fn deserialize_drama_node() {
        let yaml = r#"
chapter: ch03
dramatic_intents:
  - type: confrontation
    between: [kian, nova]
    at: oasis_gate
  - type: secret_reveal
    secret: oasis_truth
    to: [kian]
    reveal_to_reader: true
pacing:
  build_up: 0.3
  climax: 0.5
  resolution: 0.2
director_notes:
  pov: nova
  tone: "紧张到绝望"
  highlights:
    - "基安第一次看到绿洲的防御墙"
    - "诺娃犹豫是否扣下扳机"
"#;
        let node: DramaNode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(node.chapter, "ch03");
        assert_eq!(node.dramatic_intents.len(), 2);
        assert_eq!(node.director_notes.pov, Some("nova".to_string()));
        assert_eq!(node.director_notes.highlights.len(), 2);
        assert!((node.pacing.climax - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn serialize_roundtrip() {
        let node = DramaNode {
            chapter: "ch01".into(),
            dramatic_intents: vec![DramaticIntent::Reversal {
                target: "kian".into(),
                trigger: "发现水源被污染".into(),
                secret: None,
                timing: Timing::Climax,
            }],
            pacing: Pacing::default(),
            director_notes: DirectorNotes {
                pov: Some("kian".into()),
                ..Default::default()
            },
        };
        let yaml = serde_yaml::to_string(&node).unwrap();
        let parsed: DramaNode = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.chapter, "ch01");
        assert_eq!(parsed.dramatic_intents.len(), 1);
    }

    #[test]
    fn default_drama_node_is_valid() {
        let node = DramaNode {
            chapter: "ch99".into(),
            dramatic_intents: vec![],
            pacing: Pacing::default(),
            director_notes: DirectorNotes::default(),
        };
        let rendered = node.render();
        assert!(rendered.contains("ch99"));
    }

    #[test]
    fn render_includes_intents() {
        let node = DramaNode {
            chapter: "ch03".into(),
            dramatic_intents: vec![
                DramaticIntent::Confrontation {
                    between: vec!["kian".into(), "nova".into()],
                    at: "oasis_gate".into(),
                    depends_on: vec![],
                },
                DramaticIntent::SuspenseResolution {
                    goal: "kian/survive_drought".into(),
                    expected_outcome: GoalOutcome::Blocked,
                },
            ],
            pacing: Pacing::default(),
            director_notes: DirectorNotes::default(),
        };
        let rendered = node.render();
        assert!(rendered.contains("kian vs nova"));
        assert!(rendered.contains("悬念解决"));
    }
}
