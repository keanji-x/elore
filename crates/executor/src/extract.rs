//! Effect extraction from narrative text.
//!
//! After the Author writes a chapter, this module scans the text
//! for state changes and converts them into `Op`s.
//! Pattern: `«effect: remove_item(kian, 电磁短刀)»`

use crate::ExecutorError;
use ledger::effect::op::Op;

/// Annotation marker for effects embedded in text.
const EFFECT_OPEN: &str = "«effect:";
const EFFECT_CLOSE: &str = "»";

/// Extract annotated effects from narrative text.
pub fn extract_effects(text: &str) -> Result<Vec<Op>, ExecutorError> {
    let mut effects = Vec::new();
    let mut search_from = 0;

    while let Some(start) = text[search_from..].find(EFFECT_OPEN) {
        let abs_start = search_from + start + EFFECT_OPEN.len();
        if let Some(end) = text[abs_start..].find(EFFECT_CLOSE) {
            let abs_end = abs_start + end;
            let dsl = text[abs_start..abs_end].trim();
            let op = Op::parse(dsl).map_err(|e| ExecutorError::Extraction(e.to_string()))?;
            effects.push(op);
            search_from = abs_end + EFFECT_CLOSE.len();
        } else {
            break;
        }
    }
    Ok(effects)
}

/// Strip effect annotations from text, leaving clean narrative.
pub fn strip_annotations(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut search_from = 0;

    while let Some(start) = text[search_from..].find(EFFECT_OPEN) {
        let abs_start = search_from + start;
        result.push_str(&text[search_from..abs_start]);
        if let Some(end) = text[abs_start..].find(EFFECT_CLOSE) {
            search_from = abs_start + end + EFFECT_CLOSE.len();
        } else {
            result.push_str(&text[abs_start..]);
            return result;
        }
    }
    result.push_str(&text[search_from..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_single_effect() {
        let text = "基安把刀扔进了沙中。«effect: remove_item(kian, 电磁短刀)»";
        let effects = extract_effects(text).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            Op::RemoveItem {
                entity: "kian".into(),
                item: "电磁短刀".into()
            }
        );
    }

    #[test]
    fn extract_multiple_effects() {
        let text = "他穿过了沙暴。«effect: move(kian, oasis_gate)» 终于看见了绿洲。«effect: add_trait(kian, 目睹真相)»";
        let effects = extract_effects(text).unwrap();
        assert_eq!(effects.len(), 2);
    }

    #[test]
    fn strip_produces_clean_text() {
        let text = "基安把刀扔进了沙中。«effect: remove_item(kian, 电磁短刀)» 他继续前行。";
        let clean = strip_annotations(text);
        assert_eq!(clean, "基安把刀扔进了沙中。 他继续前行。");
        assert!(!clean.contains("effect"));
    }

    #[test]
    fn no_annotations_returns_unchanged() {
        let text = "一切风平浪静。";
        let clean = strip_annotations(text);
        assert_eq!(clean, text);
    }
}
