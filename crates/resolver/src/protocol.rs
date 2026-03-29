//! Inter-layer communication protocol types.
//!
//! Defines the concrete message types passed between layers.
//! These types enforce the causality constraint:
//! Intent (Director) → Facts (Engine) → Text (Author).

use ledger::input::secret::Secret;
use ledger::state::reasoning::ReasoningResult;
use ledger::{Op, Snapshot};

use crate::drama::DirectorNotes;
use crate::prompt::AuthorPrompt;
use crate::validate::Verdict;

/// Engine → Director output.
#[derive(Debug, Clone)]
pub struct EngineOutput {
    pub snapshot: Snapshot,
    pub derived_facts: ReasoningResult,
    pub secrets_state: Vec<Secret>,
}

/// Director → Author output.
#[derive(Debug, Clone)]
pub struct DirectorToAuthor {
    pub prompt: AuthorPrompt,
    pub required_effects: Vec<Op>,
    pub suggested_effects: Vec<Op>,
    pub director_notes: DirectorNotes,
    pub verdict: Verdict,
}

/// Author → Director feedback (after writing).
#[derive(Debug, Clone)]
pub struct AuthorToDirector {
    pub text: String,
    pub annotated_effects: Vec<Op>,
    pub deviations: Vec<Deviation>,
}

/// When the Author couldn't satisfy a required effect.
#[derive(Debug, Clone)]
pub struct Deviation {
    pub expected: Op,
    pub reason: String,
    pub alternative: Option<Op>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deviation_can_be_created() {
        let d = Deviation {
            expected: Op::RemoveItem {
                entity: "kian".into(),
                item: "刀".into(),
            },
            reason: "角色握紧了刀, 没有丢弃".into(),
            alternative: None,
        };
        assert_eq!(d.reason, "角色握紧了刀, 没有丢弃");
    }
}
