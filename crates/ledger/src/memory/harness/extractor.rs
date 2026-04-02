//! Entity/event extraction from raw novel text.
//!
//! Two character discovery strategies:
//!
//! 1. **Roster mode**: user provides a character/location name dictionary
//! 2. **Dialogue attribution mode** (default): extract names from patterns
//!    like "XX说", "XX道", "XX笑道" — structural signal, not frequency hacks
//!
//! Events are extracted at **paragraph level**: each paragraph with at least
//! one character mention becomes an event.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use super::loader::Novel;

// ══════════════════════════════════════════════════════════════════
// Roster — character/location dictionary
// ══════════════════════════════════════════════════════════════════

/// Entity roster for name matching.
#[derive(Debug, Clone, Default)]
pub struct Roster {
    /// Character names → canonical ID.
    pub characters: HashMap<String, String>,
    /// Location names → canonical ID.
    pub locations: HashMap<String, String>,
}

impl Roster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_character(&mut self, id: &str, names: &[&str]) {
        for name in names {
            self.characters.insert(name.to_string(), id.to_string());
        }
    }

    pub fn add_location(&mut self, id: &str, names: &[&str]) {
        for name in names {
            self.locations.insert(name.to_string(), id.to_string());
        }
    }

    /// Load roster from a YAML file.
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = std::fs::read_to_string(path)?;
        let raw: RosterFile = serde_yaml::from_str(&text)?;
        let mut roster = Self::new();
        for (id, names) in &raw.characters {
            for name in names {
                roster.characters.insert(name.clone(), id.clone());
            }
        }
        for (id, names) in &raw.locations {
            for name in names {
                roster.locations.insert(name.clone(), id.clone());
            }
        }
        Ok(roster)
    }

    /// Discover character names from **dialogue attribution patterns**.
    ///
    /// Scans for "XX说道：", "XX问道：" etc. The XX before a speech verb
    /// is almost always a character name in Chinese novels.
    ///
    /// Key insight: skip adverb modifiers ("冷冷**地**说" → skip "地" → take "冷冷" →
    /// that's not a name either, so structural filter catches it).
    pub fn discover_from_dialogue(novel: &Novel, min_mentions: usize) -> Vec<(String, usize)> {
        let mut counts: HashMap<String, usize> = HashMap::new();

        // Speech verbs — sorted longest-first so "说道" matches before "说"
        let speech_verbs: &[&str] = &[
            "冷笑道", "大笑道", "冷声道", "淡淡道", "沉声道",
            "低声道", "厉声道", "大喊道", "大声道", "低声说",
            "说道", "笑道", "问道", "答道", "喝道", "叹道", "怒道",
            "心中想", "暗想", "心想",
            "说", "道", "问", "答", "嚷", "吼",
        ];

        // Adverb suffixes to skip before the name
        let adverb_markers: &[char] = &['地', '着', '了', '的', '得'];

        for ch in &novel.chapters {
            for para in &ch.paragraphs {
                let chars: Vec<char> = para.chars().collect();

                for verb in speech_verbs {
                    let verb_chars: Vec<char> = verb.chars().collect();
                    let mut i = 0;
                    while i + verb_chars.len() <= chars.len() {
                        // Check if verb matches at position i
                        if chars[i..i + verb_chars.len()] == verb_chars[..] {
                            // Walk backwards from i, skipping adverb markers
                            let mut end = i;
                            if end > 0 && adverb_markers.contains(&chars[end - 1]) {
                                end -= 1;
                                // Also skip the adverb itself (e.g. "平静" before "地")
                                // We'll let the name extraction handle this
                            }

                            // Extract 2-char and 3-char candidates before `end`
                            for name_len in [2, 3] {
                                if end >= name_len {
                                    let candidate: String =
                                        chars[end - name_len..end].iter().collect();
                                    if is_likely_name(&candidate) {
                                        *counts.entry(candidate).or_default() += 1;
                                    }
                                }
                            }

                            i += verb_chars.len();
                        } else {
                            i += 1;
                        }
                    }
                }
            }
        }

        // Filter by min mentions and sort
        let mut candidates: Vec<(String, usize)> = counts
            .into_iter()
            .filter(|(_, count)| *count >= min_mentions)
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1));
        candidates
    }
}

#[derive(serde::Deserialize)]
struct RosterFile {
    #[serde(default)]
    characters: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    locations: BTreeMap<String, Vec<String>>,
}

// ══════════════════════════════════════════════════════════════════
// Event extraction
// ══════════════════════════════════════════════════════════════════

/// A raw event extracted from text.
#[derive(Debug, Clone)]
pub struct RawEvent {
    pub chapter_index: usize,
    pub paragraph_index: usize,
    /// Characters present in this paragraph (canonical IDs).
    pub characters: BTreeSet<String>,
    /// Locations mentioned (canonical IDs).
    pub locations: BTreeSet<String>,
    /// Emotional valence estimate (-1.0 to 1.0).
    pub valence: f32,
    /// The raw paragraph text (for embedding).
    pub text: String,
    /// Sequence number (global ordering).
    pub seq: u32,
}

/// Extract events from a novel using a roster. One event per paragraph.
pub fn extract_events(novel: &Novel, roster: &Roster) -> Vec<RawEvent> {
    let paragraphs = novel.paragraphs();
    let mut events = Vec::new();
    let mut seq = 0u32;

    for para in &paragraphs {
        let characters = find_entities(&para.text, &roster.characters);
        let locations = find_entities(&para.text, &roster.locations);

        // Only create events for paragraphs with at least one character
        if characters.is_empty() {
            continue;
        }

        let valence = estimate_valence(&para.text);

        events.push(RawEvent {
            chapter_index: para.chapter_index,
            paragraph_index: para.paragraph_index,
            characters,
            locations,
            valence,
            text: para.text.clone(),
            seq,
        });
        seq += 1;
    }

    events
}

/// Find all entity mentions in text, returning canonical IDs.
fn find_entities(text: &str, dictionary: &HashMap<String, String>) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    // Sort by name length descending for longest-match-first
    let mut names: Vec<(&str, &str)> = dictionary
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    names.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (name, id) in &names {
        if text.contains(name) {
            found.insert(id.to_string());
        }
    }
    found
}

/// Emotional valence estimation — normalized by paragraph length.
fn estimate_valence(text: &str) -> f32 {
    let positive = [
        "笑", "喜", "乐", "爱", "好", "美", "幸", "福", "快",
        "赞", "善", "亲", "欢", "暖", "甜", "安", "宁", "和",
        "赢", "胜", "强", "成功", "突破",
    ];
    let negative = [
        "死", "杀", "哭", "怒", "恨", "悲", "伤", "痛", "苦",
        "惧", "怕", "忧", "愁", "恶", "毒", "血", "泪", "叹",
        "冷", "暗", "败", "亡", "离", "危", "困",
    ];

    let mut pos_count = 0u32;
    let mut neg_count = 0u32;
    for kw in &positive {
        pos_count += text.matches(kw).count() as u32;
    }
    for kw in &negative {
        neg_count += text.matches(kw).count() as u32;
    }

    let total = pos_count + neg_count;
    if total == 0 {
        return 0.0;
    }
    // Normalize: difference / total, so range is -1..1
    (pos_count as f32 - neg_count as f32) / total as f32
}

// ══════════════════════════════════════════════════════════════════
// Co-occurrence analysis
// ══════════════════════════════════════════════════════════════════

/// Character co-occurrence matrix.
#[derive(Debug, Clone)]
pub struct CooccurrenceMatrix {
    pub pairs: BTreeMap<(String, String), u32>,
    pub counts: BTreeMap<String, u32>,
}

pub fn build_cooccurrence(events: &[RawEvent]) -> CooccurrenceMatrix {
    let mut pairs: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();

    for event in events {
        let chars: Vec<&String> = event.characters.iter().collect();
        for c in &chars {
            *counts.entry((*c).clone()).or_default() += 1;
        }
        for i in 0..chars.len() {
            for j in (i + 1)..chars.len() {
                let (a, b) = if chars[i] < chars[j] {
                    (chars[i].clone(), chars[j].clone())
                } else {
                    (chars[j].clone(), chars[i].clone())
                };
                *pairs.entry((a, b)).or_default() += 1;
            }
        }
    }

    CooccurrenceMatrix { pairs, counts }
}

impl CooccurrenceMatrix {
    pub fn top_pairs(&self, n: usize) -> Vec<((String, String), u32)> {
        let mut sorted: Vec<_> = self.pairs.iter().map(|(k, v)| (k.clone(), *v)).collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(n);
        sorted
    }

    pub fn top_characters(&self, n: usize) -> Vec<(String, u32)> {
        let mut sorted: Vec<_> = self.counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(n);
        sorted
    }
}

// ══════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════

fn is_cjk_ideograph(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c)
        || ('\u{3400}'..='\u{4DBF}').contains(&c)
        || ('\u{F900}'..='\u{FAFF}').contains(&c)
}

/// Structural filter: does this 2-3 char string look like a Chinese name?
///
/// Rules (all structural, no frequency analysis):
/// 1. All chars must be CJK ideographs
/// 2. Must not start with common function/adverb prefixes
/// 3. Must not end with common suffixes (地/着/了/的/等)
/// 4. Must not be a common pronoun/generic term
fn is_likely_name(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();

    // Must be all CJK ideographs
    if !chars.iter().all(|c| is_cjk_ideograph(*c)) {
        return false;
    }

    // Reject: starts with function word
    let bad_starts: &[char] = &[
        '不', '也', '就', '都', '又', '在', '出', '成', '化',
        '被', '把', '让', '给', '对', '向', '从', '到', '这',
        '那', '要', '会', '能', '可', '该', '得', '有', '没',
        '很', '太', '最', '更', '还', '才', '已', '正', '将',
        '一', '两', '几', '所', '每', '全', '各',
    ];
    if bad_starts.contains(&chars[0]) {
        return false;
    }

    // Reject: ends with adverb/particle suffix
    let bad_ends: &[char] = &[
        '地', '着', '了', '的', '得', '过', '们', '吗', '呢',
        '吧', '啊', '呀', '哦', '嘛', '么',
    ];
    if bad_ends.contains(chars.last().unwrap()) {
        return false;
    }

    // Reject: known non-name patterns
    let non_names: &[&str] = &[
        "众人", "那人", "此人", "有人", "何人", "他人", "旁人",
        "老者", "少年", "女子", "男子", "弟子", "长老", "老人",
        "对方", "自己", "我们", "他们", "她们", "你们",
        "什么", "如何", "怎么", "为何", "无疑", "果然",
        "竟然", "居然", "突然", "当然", "忽然",
        "随即", "随后", "然后", "接着", "终于",
        "马上", "立刻", "声大", "平静", "冷漠", "冷冷",
        "大声", "低声", "淡淡", "沉声", "毫无",
    ];
    if non_names.contains(&s) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_entities() {
        let mut dict = HashMap::new();
        dict.insert("宝玉".into(), "jia_baoyu".into());
        dict.insert("贾宝玉".into(), "jia_baoyu".into());
        dict.insert("黛玉".into(), "lin_daiyu".into());

        let found = find_entities("宝玉和黛玉在园中相遇", &dict);
        assert!(found.contains("jia_baoyu"));
        assert!(found.contains("lin_daiyu"));
    }

    #[test]
    fn test_valence_normalized() {
        // Even with many negative keywords, normalization keeps it in range
        let text = "杀死杀死杀死杀死杀死杀死笑笑";
        let v = estimate_valence(text);
        assert!(v >= -1.0 && v <= 1.0);
    }

    #[test]
    fn test_dialogue_discovery() {
        let text = "第一章 测试\n\n\
            叶凡说道：\u{201C}你好。\u{201D}\n\n\
            叶凡笑道：\u{201C}不错。\u{201D}\n\n\
            庞博问道：\u{201C}怎么了？\u{201D}\n\n\
            叶凡说：\u{201C}走吧。\u{201D}\n";
        let novel = super::super::loader::parse_novel(text, "测试");
        let candidates = Roster::discover_from_dialogue(&novel, 1);
        let names: Vec<&str> = candidates.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"叶凡"), "should find 叶凡, got: {:?}", names);
        assert!(names.contains(&"庞博"), "should find 庞博, got: {:?}", names);

        // 叶凡 should have more mentions than 庞博
        let ye_fan_count = candidates.iter().find(|(n, _)| n == "叶凡").map(|(_, c)| *c).unwrap_or(0);
        let pang_bo_count = candidates.iter().find(|(n, _)| n == "庞博").map(|(_, c)| *c).unwrap_or(0);
        assert!(ye_fan_count > pang_bo_count);
    }

    #[test]
    fn test_cooccurrence() {
        let events = vec![
            RawEvent {
                chapter_index: 0,
                paragraph_index: 0,
                characters: ["a", "b"].iter().map(|s| s.to_string()).collect(),
                locations: BTreeSet::new(),
                valence: 0.0,
                text: "test".into(),
                seq: 0,
            },
            RawEvent {
                chapter_index: 0,
                paragraph_index: 1,
                characters: ["a", "b", "c"].iter().map(|s| s.to_string()).collect(),
                locations: BTreeSet::new(),
                valence: 0.0,
                text: "test".into(),
                seq: 1,
            },
        ];
        let matrix = build_cooccurrence(&events);
        assert_eq!(matrix.pairs[&("a".into(), "b".into())], 2);
    }
}
