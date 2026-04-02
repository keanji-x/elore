//! Memory → Datalog fact generation.
//!
//! Translates memory edges and perceptions into typed Datalog facts
//! that plug into the existing reasoning engine. This bridges the
//! "intuitive" vector-based retrieval with the "logical" Datalog layer.
//!
//! Generated predicates:
//!
//! ```datalog
//! % Edge facts
//! memory(edge_id, content_node, seq).
//! mem_participant(edge_id, char_id).
//! mem_location(edge_id, loc_id).
//! mem_secret(edge_id, secret_id).
//! mem_object(edge_id, object).
//! mem_salience(edge_id, score).      % score as integer 0-100
//! mem_valence(edge_id, score).       % score as integer -100..100
//!
//! % Perception facts
//! perceives(char_id, edge_id, mode).
//!
//! % Derived (via rules in rule.rs):
//! knows_event(char, edge).
//! shared_memory(a, b, edge).
//! knowledge_gap(char, edge).
//! could_inform(a, b, edge).
//! trauma(char, edge).
//! ```

use crate::state::fact::{Arg, FactSet};

use super::edge::{MemoryEdge, Perception};

/// Generate Datalog facts from memory edges and perceptions.
pub fn collect_memory_facts(
    edges: &[MemoryEdge],
    perceptions: &[Perception],
) -> FactSet {
    let mut facts = FactSet::new();

    for edge in edges {
        // memory(edge_id, content_node, seq)
        facts.add(
            "memory",
            vec![
                Arg::Id(edge.id.clone()),
                Arg::auto(&edge.content_node),
                Arg::Int(edge.seq as i64),
            ],
        );

        // mem_participant(edge_id, char_id)
        for p in &edge.participants {
            facts.add(
                "mem_participant",
                vec![Arg::Id(edge.id.clone()), Arg::auto(p)],
            );
        }

        // mem_location(edge_id, loc_id)
        for l in &edge.locations {
            facts.add(
                "mem_location",
                vec![Arg::Id(edge.id.clone()), Arg::auto(l)],
            );
        }

        // mem_secret(edge_id, secret_id)
        for s in &edge.secrets {
            facts.add(
                "mem_secret",
                vec![Arg::Id(edge.id.clone()), Arg::auto(s)],
            );
        }

        // mem_object(edge_id, object)
        for o in &edge.objects {
            facts.add(
                "mem_object",
                vec![Arg::Id(edge.id.clone()), Arg::auto(o)],
            );
        }

        // mem_salience(edge_id, score) — as integer 0-100
        facts.add(
            "mem_salience",
            vec![
                Arg::Id(edge.id.clone()),
                Arg::Int((edge.salience * 100.0) as i64),
            ],
        );

        // mem_valence(edge_id, score) — as integer -100..100
        facts.add(
            "mem_valence",
            vec![
                Arg::Id(edge.id.clone()),
                Arg::Int((edge.emotional_valence * 100.0) as i64),
            ],
        );
    }

    // Perception facts
    for p in perceptions {
        let mode_str = match p.mode {
            super::edge::PerceptionMode::Witnessed => "witnessed",
            super::edge::PerceptionMode::Told => "told",
            super::edge::PerceptionMode::Inferred => "inferred",
            super::edge::PerceptionMode::Rumor => "rumor",
            super::edge::PerceptionMode::False => "false",
        };
        facts.add(
            "perceives",
            vec![
                Arg::auto(&p.character),
                Arg::Id(p.edge_id.clone()),
                Arg::Id(mode_str.to_string()),
            ],
        );
    }

    facts
}

/// Datalog rules for memory reasoning.
///
/// These rules are appended to the existing rule set during reasoning.
pub const MEMORY_RULES: &str = r#"
% ═══ Memory reasoning rules ═══

% Character knows an event (any reliable perception mode)
knows_event(?C, ?E) :- perceives(?C, ?E, witnessed).
knows_event(?C, ?E) :- perceives(?C, ?E, told).
knows_event(?C, ?E) :- perceives(?C, ?E, inferred).

% Shared memory — two characters both know the same event
shared_memory(?A, ?B, ?E) :-
    knows_event(?A, ?E),
    knows_event(?B, ?E),
    ?A != ?B.

% Knowledge gap — character participated but doesn't know
knowledge_gap(?C, ?E) :-
    mem_participant(?E, ?C),
    ~knows_event(?C, ?E).

% Could inform — A knows, B doesn't, they can meet, trust >= 1
could_inform(?A, ?B, ?E) :-
    knows_event(?A, ?E),
    ~knows_event(?B, ?E),
    can_meet(?A, ?B),
    trust(?A, ?B, ?T),
    ?T >= 1.

% False belief — character has a false perception
false_belief(?C, ?E) :- perceives(?C, ?E, false).

% Trauma — high salience + negative valence + character knows
trauma(?C, ?E) :-
    knows_event(?C, ?E),
    mem_salience(?E, ?S), ?S >= 80,
    mem_valence(?E, ?V), ?V <= -50.

% Informed chain — multi-hop information propagation
informed_chain(?A, ?B, ?E) :- could_inform(?A, ?B, ?E).
informed_chain(?A, ?C, ?E) :-
    could_inform(?A, ?B, ?E),
    informed_chain(?B, ?C, ?E),
    ?A != ?C.

% Unaware threat — character doesn't know about an event targeting them
unaware_threat(?C, ?E) :-
    mem_participant(?E, ?C),
    mem_valence(?E, ?V), ?V <= -30,
    ~knows_event(?C, ?E).

% Secret witness — only one character knows (excluding participants)
sole_witness(?C, ?E) :-
    knows_event(?C, ?E),
    mem_participant(?E, ?Other),
    ?C != ?Other,
    ~knows_event(?Other, ?E).
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::op::Op;
    use crate::memory::edge::{MemoryEdge, PerceptionMode};

    #[test]
    fn test_collect_memory_facts() {
        let ops = vec![
            Op::Move {
                entity: "alice".into(),
                location: "room".into(),
            },
            Op::Reveal {
                secret: "betrayal".into(),
                to: "alice".into(),
            },
        ];
        let edge = MemoryEdge::from_ops("prologue", 1, &ops);
        let perceptions = vec![Perception {
            character: "alice".into(),
            edge_id: edge.id.clone(),
            mode: PerceptionMode::Witnessed,
        }];

        let facts = collect_memory_facts(&[edge], &perceptions);
        let datalog = facts.to_datalog();

        assert!(datalog.contains("memory("));
        assert!(datalog.contains("mem_participant("));
        assert!(datalog.contains("mem_location("));
        assert!(datalog.contains("mem_secret("));
        assert!(datalog.contains("perceives(alice,"));
    }
}
