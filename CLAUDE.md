# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                    # Build all crates
cargo test                     # Run all tests
cargo test -p ledger           # Test a single crate
cargo test -p ledger -- test_name  # Run a single test
cargo fmt                      # Format code
cargo clippy                   # Lint
```

The CLI binary is `target/debug/elore`.

## Architecture

Elore is a narrative compiler that manages creative writing through deterministic state, event sourcing, and a four-layer constraint system. It prevents narrative collapse in long-form storytelling by treating world state changes as structured operations.

### Card-Based Source of Truth

Cards (`cards/*.md`) are the sole source of truth. Each card is Markdown + YAML frontmatter. `.everlore/` is entirely derived build artifacts (gitignore-able). The CLI is a compiler.

```
cards/                         ← source of truth (human-editable)
  characters/*.md              ← character cards
  locations/*.md               ← location cards
  factions/*.md                ← faction cards
  secrets/*.md                 ← secret cards
  phases/{phase_id}/*.md       ← beat cards (001.md, 002.md, ...)

.everlore/                     ← build artifacts (regenerate with `elore build`)
  entities/*.json              ← entity cache
  phases/*.yaml                ← phase definitions
  beats/*.json                 ← beat cache
  history.jsonl                ← compiled from beat card effects
  state.json                   ← phase lifecycle state
  secrets.yaml                 ← secrets cache
```

Card format example:
```markdown
---
type: character
id: kian
name: "基安"
traits: [谨慎, 聪敏]
location: sector_4
relationships:
  - target: nova
    rel: 前同事
---

# 基安

前星联公司安全顾问。
```

`elore build` compiles cards → .everlore/ artifacts, then reverse-syncs effect state back to card YAML frontmatter.

### Workspace Crates (4-layer pipeline)

```
ledger (L1: Engine/Facts) → resolver (L2: Director/Intent) → executor (L3: Author/Text) → evaluator (L4: Reader/Feedback)
                                              ↑
                                     elore-cli (user interface)
```

- **ledger** — Single source of truth. Manages entities, secrets, goals, and effects. Contains card parser (`input/card.rs`), card writer (`input/card_writer.rs`), Phase constraint model, and Datalog reasoning (via Nemo).
- **resolver** — Validates dramatic intents against snapshots, builds LLM prompts from world state + drama plans.
- **executor** — Pluggable writer backends (trait-based) for text generation. Parses inline effect markers.
- **evaluator** — Beat-level quality annotations (score 1-5, tags), consistency audits.
- **elore-cli** — Clap-based CLI. Key command: `elore build` compiles cards into .everlore/ artifacts.

### Core Concepts

- **Card** — Markdown + YAML frontmatter file. The source of truth for entities, secrets, and beats.
- **Phase** — Smallest unit of creative work. Has a status machine (locked → ready → active → reviewing → approved), four-layer constraints (L1-L4), and a `phase_type` (Narrative or Worldbuilding).
- **Beat** — Atomic narrative commit: text + effects. Stored as card files in `cards/phases/{phase_id}/`.
- **Op** — One of 15 effect operations (AddTrait, RemoveTrait, Move, Reveal, RevealToReader, ResolveGoal, etc.). All state mutations flow through Ops.
- **Snapshot** — Frozen world state at a phase boundary, computed by replaying history on genesis entities.
- **Entity** — Character, location, or faction. YAML frontmatter = structured data, Markdown body = description.
- **Worldbuilding Phase** — Phase type for world construction (min entity counts, relationship density) rather than narrative writing.

### Data Flow

```
cards/*.md → elore build → .everlore/ (entity cache + history.jsonl) → Snapshot
                        ↓
              reverse sync: effects update card YAML frontmatter
```

### Four-Layer Constraints per Phase

| Layer | Concern | Hard/Soft |
|-------|---------|-----------|
| L1 Ledger | State invariants, exit assertions, worldbuilding counts | Hard blocker |
| L2 Resolver | min_effects, dramatic intents | Hard blocker |
| L3 Executor | Word count, POV, tone, tone_arc | Soft guidance |
| L4 Evaluator | min_avg_score, max_boring_beats, required_tags | Required for approval |

## Key Patterns

- **Cards as source of truth**: All world data lives in `cards/*.md`. `.everlore/` is derived.
- **Reverse sync**: Beat effects (e.g., `Move(kian, the_spire)`) automatically update entity card YAML.
- **Event sourcing**: State changes are Ops. `history.jsonl` is compiled from beat cards.
- **Rust edition 2024** with workspace-level dependency management.
- **Datalog reasoning** via Nemo for zero-token-cost logical inference. `run_reasoning(snapshot)` assembles entity/graph/goal/secret facts + rules, runs Nemo, and returns derived predicates (can_meet, enemy, danger, betrayal_opportunity, possible_reveal, info_cascade, dramatic_irony, alliance_opportunity, etc.). Integrated into `suggest`, `plan`, `validate`, and prompt generation.
