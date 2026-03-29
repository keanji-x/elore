//! Scoring — quantitative quality metrics for generated chapters.

/// Quality scores for a generated chapter.
#[derive(Debug, Clone)]
pub struct ChapterScore {
    /// 0–1: How many required effects were reflected.
    pub effect_coverage: f32,
    /// 0–1: How many dramatic intents were addressed.
    pub intent_coverage: f32,
    /// 0–1: Entity consistency (no phantom references).
    pub consistency: f32,
}

impl ChapterScore {
    /// Overall quality score (weighted average).
    pub fn overall(&self) -> f32 {
        self.effect_coverage * 0.4 + self.intent_coverage * 0.4 + self.consistency * 0.2
    }

    /// Calculate effect coverage from counts.
    pub fn from_counts(
        effects_found: usize,
        effects_required: usize,
        intents_addressed: usize,
        intents_total: usize,
        audit_errors: usize,
        audit_total: usize,
    ) -> Self {
        Self {
            effect_coverage: if effects_required == 0 {
                1.0
            } else {
                effects_found as f32 / effects_required as f32
            },
            intent_coverage: if intents_total == 0 {
                1.0
            } else {
                intents_addressed as f32 / intents_total as f32
            },
            consistency: if audit_total == 0 {
                1.0
            } else {
                1.0 - (audit_errors as f32 / audit_total as f32)
            },
        }
    }

    pub fn render(&self) -> String {
        format!(
            "效果覆盖: {:.0}% | 意图覆盖: {:.0}% | 一致性: {:.0}% | 综合: {:.0}%",
            self.effect_coverage * 100.0,
            self.intent_coverage * 100.0,
            self.consistency * 100.0,
            self.overall() * 100.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_score() {
        let s = ChapterScore::from_counts(3, 3, 2, 2, 0, 5);
        assert!((s.overall() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn partial_score() {
        let s = ChapterScore::from_counts(1, 2, 1, 2, 1, 4);
        assert!(s.overall() < 1.0);
        assert!(s.overall() > 0.0);
    }

    #[test]
    fn zero_requirements_is_perfect() {
        let s = ChapterScore::from_counts(0, 0, 0, 0, 0, 0);
        assert!((s.overall() - 1.0).abs() < f32::EPSILON);
    }
}
