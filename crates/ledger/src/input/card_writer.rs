//! Card writer — update card YAML frontmatter while preserving Markdown body.
//!
//! After effects are applied, the source card files' YAML needs to reflect
//! the new state (e.g., location changed after a Move effect).

use std::path::Path;

use crate::LedgerError;
use crate::input::card::split_frontmatter;
use crate::input::entity::Entity;
use crate::input::secret::Secret;

/// Rewrite YAML frontmatter in a raw Markdown string, preserving the body.
pub fn rewrite_frontmatter(raw: &str, new_yaml: &str) -> String {
    if let Some((_old_yaml, body)) = split_frontmatter(raw) {
        let mut out = format!("---\n{new_yaml}---\n");
        if !body.is_empty() {
            out.push('\n');
            out.push_str(body);
            // Ensure trailing newline
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    } else {
        // No frontmatter — prepend it
        let mut out = format!("---\n{new_yaml}---\n");
        if !raw.is_empty() {
            out.push('\n');
            out.push_str(raw);
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }
}

/// Update an entity card file's YAML frontmatter in-place, preserving the body.
pub fn update_entity_card(path: &Path, entity: &Entity) -> Result<(), LedgerError> {
    let raw = std::fs::read_to_string(path)?;

    // Serialize entity without description (description lives in the body)
    let mut yaml_entity = entity.clone();
    yaml_entity.set_description(None);
    let new_yaml = serde_yaml::to_string(&yaml_entity)?;

    let updated = rewrite_frontmatter(&raw, &new_yaml);
    std::fs::write(path, updated)?;
    Ok(())
}

/// Update a secret card file's YAML frontmatter in-place, preserving the body.
pub fn update_secret_card(path: &Path, secret: &Secret) -> Result<(), LedgerError> {
    let raw = std::fs::read_to_string(path)?;
    let new_yaml = serde_yaml::to_string(secret)?;
    let updated = rewrite_frontmatter(&raw, &new_yaml);
    std::fs::write(path, updated)?;
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::entity::Relationship;

    #[test]
    fn rewrite_preserves_body() {
        let raw = "---\nid: kian\nlocation: sector_4\n---\n\n# 基安\n\n前星联安全顾问。\n";
        let updated = rewrite_frontmatter(raw, "id: kian\nlocation: the_spire\n");
        assert!(updated.contains("location: the_spire"));
        assert!(updated.contains("# 基安"));
        assert!(updated.contains("前星联安全顾问"));
    }

    #[test]
    fn rewrite_no_existing_frontmatter() {
        let raw = "# Just markdown\n";
        let updated = rewrite_frontmatter(raw, "id: kian\n");
        assert!(updated.starts_with("---\n"));
        assert!(updated.contains("id: kian"));
        assert!(updated.contains("# Just markdown"));
    }

    #[test]
    fn update_entity_card_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kian.md");

        // Write initial card
        std::fs::write(
            &path,
            "---\ntype: character\nid: kian\nlocation: sector_4\n---\n\n# 基安\n\n安全顾问。\n",
        )
        .unwrap();

        // Update with new location
        use crate::input::entity::Character;
        let entity = Entity::Character(Character {
            id: "kian".into(),
            name: None,
            traits: vec![],
            beliefs: vec![],
            desires: vec![],
            intentions: vec![],
            location: Some("the_spire".into()),
            relationships: vec![],
            inventory: vec![],
            goals: vec![],
            tags: vec![],
            description: Some("安全顾问。".into()), // should NOT appear in YAML
        });

        update_entity_card(&path, &entity).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("the_spire"), "location should be updated");
        assert!(!content.contains("sector_4"), "old location should be gone");
        assert!(content.contains("# 基安"), "body should be preserved");
        assert!(content.contains("安全顾问"), "body text should be preserved");
        // description should NOT be in frontmatter
        assert!(!content.contains("description:"));
    }

    #[test]
    fn update_entity_card_with_relationships() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kian.md");

        std::fs::write(&path, "---\ntype: character\nid: kian\n---\n# Kian\n").unwrap();

        use crate::input::entity::Character;
        let entity = Entity::Character(Character {
            id: "kian".into(),
            name: None,
            traits: vec!["brave".into()],
            beliefs: vec![],
            desires: vec![],
            intentions: vec![],
            location: Some("the_spire".into()),
            relationships: vec![Relationship {
                target: "nova".into(),
                rel: "ally".into(),
            }],
            inventory: vec![],
            goals: vec![],
            tags: vec![],
            description: None,
        });

        update_entity_card(&path, &entity).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("brave"));
        assert!(content.contains("nova"));
        assert!(content.contains("ally"));
        assert!(content.contains("# Kian"));
    }
}
