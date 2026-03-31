//! Prompt builder — assembles the Author's input from Snapshot + DramaNode.
//!
//! The prompt encodes world state, dramatic intent, POV constraints,
//! and a content hash for memoization (same hash → skip re-generation).

use sha2::{Digest, Sha256};

use ledger::Snapshot;
use ledger::input::entity::Entity;
use ledger::input::goal;
use ledger::input::secret::Secret;
use ledger::state::reasoning::ReasoningResult;

use crate::drama::DramaNode;

/// The complete prompt for the Author layer.
#[derive(Debug, Clone)]
pub struct AuthorPrompt {
    pub chapter: String,
    pub pov: Option<String>,
    pub rendered: String,
    pub content_hash: String,
}

impl AuthorPrompt {
    pub fn build(
        snapshot: &Snapshot,
        drama: &DramaNode,
        reasoning: Option<&ReasoningResult>,
        prev_summary: Option<&str>,
        outline: Option<&str>,
    ) -> Self {
        let rendered = render_prompt(snapshot, drama, reasoning, prev_summary, outline);
        let content_hash = {
            let mut h = Sha256::new();
            h.update(rendered.as_bytes());
            format!("{:x}", h.finalize())
        };
        Self {
            chapter: drama.chapter.clone(),
            pov: drama.director_notes.pov.clone(),
            rendered,
            content_hash,
        }
    }
}

fn render_prompt(
    snapshot: &Snapshot,
    drama: &DramaNode,
    reasoning: Option<&ReasoningResult>,
    prev_summary: Option<&str>,
    outline: Option<&str>,
) -> String {
    let pov = drama.director_notes.pov.as_deref();
    let notes = &drama.director_notes;
    let mut p = String::new();

    // Header
    p.push_str("# 小说写作上下文\n\n");
    p.push_str(&format!("当前章节: {}\n", drama.chapter));
    if let Some(v) = pov {
        p.push_str(&format!("主视角 (POV): {v}\n"));
    }
    if let Some(t) = &notes.tone {
        p.push_str(&format!("基调: {t}\n"));
    }
    if let Some(a) = &notes.tone_arc {
        p.push_str(&format!("情绪弧: {a}\n"));
    }
    if let Some(wc) = notes.word_count {
        p.push_str(&format!("目标字数: ~{wc}\n"));
    }
    p.push('\n');

    if let Some(s) = prev_summary {
        p.push_str("## 前情回顾\n\n");
        p.push_str(s);
        p.push_str("\n\n");
    }
    if let Some(o) = outline {
        p.push_str("## 本章大纲\n\n");
        p.push_str(o);
        p.push_str("\n\n");
    }

    // Dramatic intents
    if !drama.dramatic_intents.is_empty() {
        p.push_str("## 本章戏剧性目标\n\n");
        for (i, intent) in drama.dramatic_intents.iter().enumerate() {
            p.push_str(&format!("{}. {}\n", i + 1, intent.summary()));
        }
        p.push('\n');
    }

    // Highlights & pacing
    if !notes.highlights.is_empty() {
        p.push_str("## 关键节拍\n\n");
        for h in &notes.highlights {
            p.push_str(&format!("★ {h}\n"));
        }
        p.push('\n');
    }
    p.push_str(&format!(
        "## 节奏\n\n铺垫 {:.0}% → 高潮 {:.0}% → 收尾 {:.0}%\n\n",
        drama.pacing.build_up * 100.0,
        drama.pacing.climax * 100.0,
        drama.pacing.resolution * 100.0,
    ));

    // Characters
    for c in snapshot.characters() {
        render_character(&mut p, c, pov);
    }

    // Goals, conflicts, suspense
    render_goals(&mut p, snapshot, pov);

    // Secrets
    render_secrets(&mut p, &snapshot.secrets, pov);

    // Locations
    for loc in snapshot.locations() {
        let name = loc.name().unwrap_or(loc.id());
        p.push_str(&format!("- **{name}** ({})", loc.id()));
        if let Some(l) = loc.as_location() {
            if !l.properties.is_empty() {
                p.push_str(&format!(": {}", l.properties.join(", ")));
            }
        }
        p.push('\n');
    }

    // Reasoning
    if let Some(r) = reasoning
        && r.total_facts > 0
    {
        p.push_str("\n## 推理结果\n\n");
        for (pred, rows) in &r.predicates {
            if rows.is_empty() {
                continue;
            }
            p.push_str(&format!("### {pred}\n"));
            for row in rows {
                p.push_str(&format!("- {}\n", row.join(", ")));
            }
            p.push('\n');
        }
    }

    // Required/suggested effects
    if !notes.required_effects.is_empty() {
        p.push_str("## 必须体现的状态变化\n\n");
        for op in &notes.required_effects {
            p.push_str(&format!("- ✓ {}\n", op.describe()));
        }
        p.push('\n');
    }
    if !notes.suggested_effects.is_empty() {
        p.push_str("## 建议的状态变化\n\n");
        for op in &notes.suggested_effects {
            p.push_str(&format!("- ~ {}\n", op.describe()));
        }
        p.push('\n');
    }

    // Writing instructions
    p.push_str("## 写作要求\n\n");
    if let Some(v) = pov {
        p.push_str(&format!("0. 视角限制: 严格遵循 {v} 的主视角\n"));
    }
    p.push_str("1. 遵守推理结果\n2. 角色行为符合 BDI\n3. 延续前情线索\n4. 围绕戏剧性目标展开\n5. 中文, 武侠/玄幻文风\n");
    p
}

fn render_character(p: &mut String, c: &Entity, pov: Option<&str>) {
    let is_pov = pov.is_none_or(|v| v == c.id());
    let name = c.name().unwrap_or(c.id());
    if is_pov {
        p.push_str(&format!("### {name} ({})\n", c.id()));
    } else {
        p.push_str(&format!("### {name} ({}) [非视角角色]\n", c.id()));
    }
    if let Some(ch) = c.as_character() {
        if !ch.traits.is_empty() {
            p.push_str(&format!("- 特质: {}\n", ch.traits.join(", ")));
        }
        if let Some(loc) = &ch.location {
            p.push_str(&format!("- 位置: {loc}\n"));
        }
        if is_pov {
            if !ch.beliefs.is_empty() {
                p.push_str(&format!("- 信念: {}\n", ch.beliefs.join(", ")));
            }
            if !ch.desires.is_empty() {
                p.push_str(&format!("- 欲望: {}\n", ch.desires.join(", ")));
            }
        } else if !ch.beliefs.is_empty() || !ch.desires.is_empty() {
            p.push_str("- BDI: [已隐藏]\n");
        }
        if !ch.inventory.is_empty() {
            p.push_str(&format!("- 物品: {}\n", ch.inventory.join(", ")));
        }
        if !ch.relationships.is_empty() {
            let rels: Vec<String> = ch
                .relationships
                .iter()
                .map(|r| {
                    let axes = format!("T{}A{}R{}", r.trust, r.affinity, r.respect);
                    format!("{}({}) [{}]", r.role, r.target, axes)
                })
                .collect();
            p.push_str(&format!("- 关系: {}\n", rels.join(", ")));
        }
    }
    p.push('\n');
}

fn render_goals(p: &mut String, snapshot: &Snapshot, pov: Option<&str>) {
    if snapshot.goal_entities.is_empty() {
        return;
    }
    let pov_goals: Vec<_> = if let Some(v) = pov {
        snapshot
            .goal_entities
            .iter()
            .filter(|ge| ge.id == v)
            .collect()
    } else {
        snapshot.goal_entities.iter().collect()
    };
    if !pov_goals.is_empty() {
        p.push_str("## 角色目标\n\n");
        for ge in &pov_goals {
            p.push_str(&goal::render_goal_tree(ge));
            p.push('\n');
        }
    }
    let conflicts = goal::find_active_conflicts(&snapshot.goal_entities);
    if !conflicts.is_empty() {
        p.push_str("## 活跃冲突\n\n");
        for (a, b) in &conflicts {
            p.push_str(&format!(
                "- ⚔ {}/{} ↔ {}/{}\n",
                a.owner, a.goal.id, b.owner, b.goal.id
            ));
        }
        p.push('\n');
    }
}

fn render_secrets(p: &mut String, secrets: &[Secret], pov: Option<&str>) {
    if secrets.is_empty() {
        return;
    }
    let visible: Vec<_> = secrets
        .iter()
        .filter(|s| pov.is_none_or(|v| s.known_by.iter().any(|k| k == v)))
        .collect();
    if !visible.is_empty() {
        p.push_str("## POV 已知秘密\n\n");
        for s in &visible {
            p.push_str(&format!("- **{}**: {}\n", s.id, s.content));
        }
        p.push('\n');
    }
    let irony: Vec<_> = secrets
        .iter()
        .filter(|s| s.revealed_to_reader && !visible.contains(s))
        .collect();
    if !irony.is_empty() {
        p.push_str("## 戏剧性反讽 (读者知, POV 不知)\n\n");
        for s in &irony {
            p.push_str(&format!("- **{}**: {}\n", s.id, s.content));
        }
        p.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drama::{DirectorNotes, Pacing};
    use ledger::input::entity::{Character, Relationship};

    fn snap() -> Snapshot {
        Snapshot::from_parts(
            "ch03",
            vec![
                Entity::Character(Character {
                    id: "kian".into(),
                    name: Some("基安".into()),
                    traits: vec!["拾荒者".into()],
                    beliefs: vec!["绿洲有水".into()],
                    desires: vec!["找到水源".into()],
                    intentions: vec![],
                    intent_targets: vec![],
                    desire_tags: vec![],
                    location: Some("oasis_gate".into()),
                    relationships: vec![Relationship {
                        target: "nova".into(),
                        role: "wary".into(),
                        trust: -1,
                        affinity: 0,
                        respect: 0,
                        facade_affinity: None,
                        facade_respect: None,
                    }],
                    inventory: vec!["电磁短刀".into()],
                    goals: vec![],
                    tags: vec![],
                    description: None,
                }),
                Entity::Character(Character {
                    id: "nova".into(),
                    name: Some("诺娃".into()),
                    traits: vec!["猎手".into()],
                    beliefs: vec!["入侵者必灭".into()],
                    desires: vec![],
                    intentions: vec![],
                    intent_targets: vec![],
                    desire_tags: vec![],
                    location: Some("oasis_gate".into()),
                    relationships: vec![],
                    inventory: vec![],
                    goals: vec![],
                    tags: vec![],
                    description: None,
                }),
            ],
            vec![],
            vec![],
        )
    }

    fn drama() -> DramaNode {
        DramaNode {
            chapter: "ch03".into(),
            dramatic_intents: vec![crate::intent::DramaticIntent::Confrontation {
                between: vec!["kian".into(), "nova".into()],
                at: "oasis_gate".into(),
                depends_on: vec![],
            }],
            pacing: Pacing::default(),
            director_notes: DirectorNotes {
                pov: Some("kian".into()),
                tone: Some("紧张".into()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn prompt_contains_sections() {
        let p = AuthorPrompt::build(&snap(), &drama(), None, None, None);
        assert!(p.rendered.contains("当前章节: ch03"));
        assert!(p.rendered.contains("主视角 (POV): kian"));
        assert!(p.rendered.contains("基安"));
        assert!(p.rendered.contains("对峙"));
    }

    #[test]
    fn pov_hides_non_pov_bdi() {
        let p = AuthorPrompt::build(&snap(), &drama(), None, None, None);
        assert!(p.rendered.contains("绿洲有水")); // kian's belief (POV)
        assert!(!p.rendered.contains("入侵者必灭")); // nova's belief (hidden)
        assert!(p.rendered.contains("BDI: [已隐藏]"));
    }

    #[test]
    fn hash_deterministic() {
        let p1 = AuthorPrompt::build(&snap(), &drama(), None, None, None);
        let p2 = AuthorPrompt::build(&snap(), &drama(), None, None, None);
        assert_eq!(p1.content_hash, p2.content_hash);
    }

    #[test]
    fn hash_changes_with_input() {
        let p1 = AuthorPrompt::build(&snap(), &drama(), None, None, None);
        let p2 = AuthorPrompt::build(&snap(), &drama(), None, Some("前情"), None);
        assert_ne!(p1.content_hash, p2.content_hash);
    }

    #[test]
    fn outline_included() {
        let p = AuthorPrompt::build(&snap(), &drama(), None, None, Some("潜入绿洲"));
        assert!(p.rendered.contains("本章大纲"));
        assert!(p.rendered.contains("潜入绿洲"));
    }
}
