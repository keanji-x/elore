# EverLore v2 架构：四层叙事引擎

## 核心命题

> **故事 = 角色 × 欲望 × 冲突**
> 
> 唯一的可编辑表面是角色 JSON。一切剧情围绕角色展开。
> 角色定义自己的环境视图、动机、期望行为。角色 JSON 就是 ground truth。

---

## 总体架构

```
                    ┌─────────────────────────────┐
                    │        Character JSON        │ ← 唯一人工编辑入口
                    │   (initial state + goals)    │
                    └──────────────┬──────────────┘
                                   │
    ═══════════════════════════════════════════════════════════
    ║                    Layer 1: Engine                      ║
    ║                   (State / 事实层)                       ║
    ║                                                         ║
    ║   graph[N] = fold(initial_entities, effects[1..N])      ║
    ║   + Datalog reasoning → derived facts                   ║
    ║                                                         ║
    ║   输入: character JSON + effect logs                     ║
    ║   输出: snapshot (世界快照) + derived facts              ║
    ║   记忆: 无 (纯确定性 replay)                             ║
    ║   成本: 零 token                                        ║
    ═══════════════════════════════════════════════════════════
                    │
                    │  snapshot + derived_facts + effect_list
                    │            ▲
                    │            │ dramatic_intent + expected constraints
                    ▼            │
    ═══════════════════════════════════════════════════════════
    ║                   Layer 2: Director                     ║
    ║                   (Drama / 意图层)                       ║
    ║                                                         ║
    ║   validate(snapshot, dramatic_intent)                    ║
    ║   construct prompt with directing notes                 ║
    ║                                                         ║
    ║   输入: Layer 1 snapshot                                 ║
    ║   输出: validated prompt + effect_list + director_notes  ║
    ║   记忆: 无 (纯校验函数)                                  ║
    ║   成本: 低 token (意图校验)                               ║
    ═══════════════════════════════════════════════════════════
                    │
                    │  prompt + effect_list + director_notes
                    │            ▲
                    │            │ finished text + annotated effects
                    ▼            │
    ═══════════════════════════════════════════════════════════
    ║                    Layer 3: Author                      ║
    ║                   (Text / 文本层)                        ║
    ║                                                         ║
    ║   text = author(prompt)   ← 纯函数，无记忆              ║
    ║                                                         ║
    ║   输入: prompt                                           ║
    ║   输出: text + annotated_effects                         ║
    ║   记忆: 无 (same input → same output → skip)            ║
    ║   成本: 高 token (文本生成，但可 memoize)                ║
    ═══════════════════════════════════════════════════════════
                    │
                    │  text
                    │            ▲
                    │            │ feedback (扩写/修改/不合理/精彩)
                    ▼            │
    ═══════════════════════════════════════════════════════════
    ║                    Layer 4: Reader                      ║
    ║                  (Memory / 读者层)                       ║
    ║                                                         ║
    ║   唯一有记忆的层：长期记忆 + 短期记忆                      ║
    ║   跨章节一致性校验 + 阅读体验反馈                          ║
    ║                                                         ║
    ║   输入: text + 历史记忆                                   ║
    ║   输出: feedback (路由到正确的层)                          ║
    ║   记忆: 有 (唯一有状态的层)                                ║
    ║   成本: 中 token (阅读理解)                               ║
    ═══════════════════════════════════════════════════════════
```

---

## Layer 1: Engine (事实层)

### 职责

维护世界状态的 **单一真相源 (Single Source of Truth)**。
所有状态变更通过 effects 表达，所有查询通过 Datalog 推理。

### 数据模型

```
WorldState = fold(GenesisEntities, Effects[1..N])
```

#### 角色 JSON (Genesis State)

角色 JSON 是唯一的人工编辑入口。它定义了角色的初始状态，是不可变的创世纪快照。

```json
{
  "type": "character",
  "id": "kian",
  "name": "基安",
  "traits": ["废土拾荒者", "极其渴望水源"],
  "location": "wasteland",
  "inventory": ["旧式防毒面具", "电磁短刀"],
  "beliefs": ["绿洲的财阀藏匿了世界上最后一条干净的地下水脉"],
  "relationships": [],
  "tags": ["ch01", "ch02"]
}
```

#### 欲望树 (Goal Tree, YAML)

欲望树驱动叙事因果。递归结构：`want → problem → solution → children`。

```yaml
type: character
id: kian
goals:
  - id: survive_drought
    want: 找到干净水源生存下去
    problem: 荒野污染严重，水资源枯竭
    solution: 潜入财阀的最后绿洲
    status: active           # background | active | blocked | resolved | failed
    children:
      - id: infiltrate_oasis
        want: 潜入绿洲核心区
        problem: 绿洲外围有致命的安保系统
        solution: null        # null = 悬念
        status: active
        conflicts_with: [nova/harvest_intruder]
```

#### Effect Log (history.jsonl)

所有状态变更记录在 append-only 的事件日志中。角色 JSON **永远不被 effects 修改**。

```jsonl
{"chapter":"ch03","seq":1,"op":"remove_item","entity":"kian","item":"旧式防毒面具"}
{"chapter":"ch03","seq":2,"op":"add_item","entity":"nova","item":"旧式防毒面具"}
{"chapter":"ch03","seq":3,"op":"add_trait","entity":"kian","value":"被追踪"}
```

支持的 Effect 操作：

| 操作 | 语法 | 影响字段 |
|------|------|---------|
| `add_trait` | `add_trait(entity, trait)` | `traits` |
| `remove_trait` | `remove_trait(entity, trait)` | `traits` |
| `add_item` | `add_item(entity, item)` | `inventory` |
| `remove_item` | `remove_item(entity, item)` | `inventory` |
| `move` | `move(entity, location)` | `location` |
| `add_rel` | `add_rel(entity, target, rel)` | `relationships` |
| `remove_rel` | `remove_rel(entity, target)` | `relationships` |
| `set_belief` | `set_belief(entity, old, new)` | `beliefs` |
| `resolve_goal` | `resolve_goal(owner, goal_id, solution)` | `goals[].status` |
| `fail_goal` | `fail_goal(owner, goal_id)` | `goals[].status` |
| `emerge_goal` | `emerge_goal(owner, goal_id, want, ...)` | `goals[]` |

#### 信息披露层 (Secrets / known_by)

> [!IMPORTANT]
> 当前 EverLore v1 缺失的关键维度。控制"谁在什么时候知道什么"。

```yaml
secrets:
  - id: oasis_truth
    content: 绿洲的能源来自活人培养皿
    known_by: []                  # 当前无人知晓
    revealed_to_reader: false     # 读者也不知道 → 悬疑
    dramatic_function: reversal   # 导演标注：这是反转用的
```

Effect 扩展：

```jsonl
{"chapter":"ch05","seq":4,"op":"reveal","secret":"oasis_truth","to":"kian"}
{"chapter":"ch06","seq":1,"op":"reveal","secret":"oasis_truth","to":"nova"}
{"chapter":"ch06","seq":2,"op":"reveal_to_reader","secret":"oasis_truth"}
```

这使得以下叙事技巧可以被形式化：

| 技巧 | known_by 状态 | revealed_to_reader |
|------|--------------|-------------------|
| **悬疑** | 无人知晓 | false |
| **戏剧性反讽** | 角色 A 知道 | true（读者知道但角色 B 不知道） |
| **扮猪吃老虎** | 角色自己知道 | false（读者不知道） |
| **误导** | 角色错误地相信 | true（读者被同步误导） |
| **揭示/反转** | → 转为已知 | true（在关键时刻揭露） |

### Snapshot 构建

```rust
fn build_snapshot(chapter: &str) -> Snapshot {
    let mut state = load_genesis_entities();     // 加载角色 JSON
    let effects = load_effects_up_to(chapter);   // 从 history.jsonl 读取

    for effect in effects {
        state.apply(effect);                     // 确定性 fold
    }

    let datalog_facts = translate_to_datalog(&state);  // 翻译为 Datalog
    let derived = run_reasoning(datalog_facts);         // 推理

    Snapshot {
        entities: state,
        derived_facts: derived,   // can_meet, danger, suspense, active_conflict...
        secrets: state.secrets,   // 信息披露状态
    }
}
```

### Datalog 推理规则

内置规则（确定性推导，零 token）：

```datalog
% 社交：谁能相遇
can_meet(?A, ?B) :- at(?A, ?P), at(?B, ?P), ?A != ?B, character(?A), character(?B).

% 敌对：敌对势力成员
enemy(?A, ?B) :- member(?A, ?S1), member(?B, ?S2), rival(?S1, ?S2), ?A != ?B.

% 危险：敌人相遇
danger(?A, ?B) :- can_meet(?A, ?B), enemy(?A, ?B).

% 欲望引擎：悬念
suspense(?Owner, ?Goal) :- goal_status(?Owner, ?Goal, active), ~has_solution(?Owner, ?Goal).

% 欲望引擎：活跃冲突
active_conflict(?OA, ?GA, ?OB, ?GB) :-
  conflicts(?OA, ?GA, ?OB, ?GB),
  goal_status(?OA, ?GA, active),
  goal_status(?OB, ?GB, active).

% 欲望引擎：解锁
unblocked(?Owner, ?Goal) :-
  goal_status(?Owner, ?Goal, blocked),
  blocks(?BOwner, ?BGoal, ?Owner, ?Goal),
  goal_status(?BOwner, ?BGoal, failed).

% 信息不对称检测
dramatic_irony(?Secret) :-
  secret_known_by(?Secret, ?Char),
  secret_revealed_to_reader(?Secret),
  secret_not_known_by(?Secret, ?OtherChar).
```

---

## Layer 2: Director (意图层)

### 职责

设计剧情走向。操作对象是 **戏剧性意图 (dramatic intent)**，不是 effects。
不能直接修改 prompt，只能通过向 Layer 1 提交 effects 间接影响 prompt。

### 数据模型

#### Drama Node

每章一个 drama node，声明该章的戏剧性目标：

```yaml
# .everlore/drama/ch03.yaml
chapter: ch03
dramatic_intents:
  - type: confrontation
    between: [kian, nova]
    at: oasis_core
    depends_on: [kian.location, nova.location, nova.inventory]
    
  - type: reversal
    target: nova
    trigger: "发现与核心信念矛盾的事实"
    secret: oasis_truth
    timing: climax

  - type: suspense_resolution
    goal: kian/infiltrate_oasis
    expected_outcome: resolved

pacing:
  build_up: 0.5      # 前半段铺垫
  climax: 0.35        # 高潮
  resolution: 0.15    # 收尾

director_notes:
  highlights: ["净水池干涸露出培养皿"]
  tone_arc: "冰冷理性 → 信仰崩塌 → 怒火"
  pov: nova
```

#### Director Notes (给 Author 的导演指令)

```yaml
director_notes:
  required_effects:     # 必须发生的 effects
    - remove_item(kian, 电磁短刀)
    - reveal(oasis_truth, nova)
  suggested_effects:    # 建议发生，Author 可偏离
    - add_trait(nova, 动摇)
  highlights:           # 高光段落提示
    - "培养皿的视觉冲击"
    - "诺娃放下共振刃的瞬间"
  pov: nova
  pov_constraints:
    - "严格 nova 视角，禁止描写 kian 内心"
    - "nova 的义眼数据可以作为观察 kian 的手段"
  tone: "从机械的猎杀效率感，逐渐转向信仰崩塌的失控"
```

### 1↔2 通信协议

#### 1→2 (Engine → Director)

```
EngineOutput {
  snapshot_before:   Snapshot,      // 本章开始时的世界状态
  snapshot_after:    Snapshot,      // 应用 effects 后的世界状态
  effect_list:       Vec<Effect>,   // 本章发生的 effects
  derived_facts:     Vec<Fact>,     // Datalog 推导结果
  secrets_state:     Vec<Secret>,   // 当前信息披露状态
}
```

#### 2→1 (Director → Engine)

```
DirectorFeedback {
  verdict:           Accept | Reject,
  dramatic_intents:  Vec<DramaticIntent>,   // 期望的戏剧性效果
  // 注意：Director 不给出具体 effects，
  // 而是给出意图，由 Engine 搜索满足意图的 effects
}
```

#### 循环逻辑

```
loop {
    engine_output = engine.build_snapshot(chapter);
    verdict = director.validate(engine_output, drama_node);

    match verdict {
        Accept => break,   // 收敛
        Reject(intents) => {
            // Engine 根据意图搜索可行的 effects
            candidate_effects = engine.search_effects(intents);
            // 人工确认（或自动选择最优解）
            engine.apply_effects(candidate_effects);
        }
    }
}
```

**收敛条件：** 所有 `dramatic_intents` 都被 `derived_facts` 满足。

---

## Layer 3: Author (文本层)

### 职责

根据 prompt 生成文本。**纯函数，无记忆。**

```
text = author(prompt)
```

Same input → same output → skip. 这是整个系统做增量更新的关键性质。

### 数据模型

#### Prompt (Author 的唯一输入)

Prompt 由 Director 构造，包含：

```markdown
# 小说写作上下文

当前章节: ch03
主视角 (POV): nova

## 前情回顾（自动生成）
在 ch01-ch02 中发生了以下变化：
- kian: 获得特质 "被追踪"
- kian: 失去物品 "旧式防毒面具"

## 本章大纲
[Director 审核通过的场景节拍]

## 角色状态
[从 snapshot 提取，POV 过滤后的角色信息]

## 角色目标（欲望引擎）
[POV 角色的完整目标树 + 活跃冲突]

## 信息控制
[本章 POV 角色已知/未知的秘密]

## 导演指令
[高光、节奏、语调、视角约束]

## 必须发生的事件
[required_effects 的自然语言描述]

## 写作要求
[文风、视角限制、一致性规则]
```

#### Author Output

```
AuthorOutput {
  text:               String,        // 生成的文本
  annotated_effects:   Vec<Effect>,   // 文本中实际发生的 effects
  deviations:          Vec<Deviation>,// 与 required_effects 的偏离及理由
}
```

### 2↔3 通信协议

#### 2→3 (Director → Author)

```
DirectorToAuthor {
  prompt:            Prompt,
  required_effects:  Vec<Effect>,    // 必须发生
  suggested_effects: Vec<Effect>,    // 建议发生
  director_notes:    DirectorNotes,  // 高光/节奏/语调
}
```

#### 3→2 (Author → Director)

```
AuthorToDirector {
  text:               String,
  annotated_effects:   Vec<Effect>,
  deviations:          Vec<Deviation>,  // Author 的创造性偏离
}
```

#### 循环逻辑

```
loop {
    author_output = author.write(prompt);

    // Director 检查 required_effects 是否被满足
    missing = required_effects - author_output.annotated_effects;
    if missing.is_empty() {
        // 吸收 Author 的创造性偏离
        // 如果 Author 标注了 required 以外的 effects，
        // Director 将它们提交给 Engine 做一致性校验
        engine.validate_effects(author_output.annotated_effects);
        break;  // 收敛
    } else {
        // Author 遗漏了必须事件，重新指导
        prompt = director.revise_prompt(prompt, missing);
    }
}
```

**收敛条件：** `required_effects ⊆ annotated_effects` 且所有 effects 通过 Engine 一致性校验。

---

## Layer 4: Reader (读者层)

### 职责

模拟真实读者的阅读体验。**唯一有记忆的层。**

### 数据模型

#### 长期记忆 (Long-term Memory)

跨章节持久化的结构化知识：

```yaml
characters_known:
  kian:
    first_appeared: ch01
    key_traits_observed: [废土拾荒者, 极其渴望水源]
    key_events:
      - ch01: 丢失面具，潜入绿洲
      - ch02: 发现培养皿真相
    emotional_arc: 绝望 → 狂暴 → 联合

  nova:
    first_appeared: ch01
    key_traits_observed: [赛博义体化, 绝对理性]
    key_events:
      - ch02: 信仰崩塌，放下武器
    emotional_arc: 冰冷 → 震惊 → 愤怒

world_facts_known:
  - "绿洲核心区有净水池"           # ch01 获知
  - "绿洲能源来自活人培养皿"       # ch02 获知

unresolved_questions:
  - "财阀高层知不知道真相？"
  - "还有其他绿洲吗？"

emotional_state:
  tension_level: 0.85
  sympathy_towards: {kian: 0.9, nova: 0.7}
```

#### 短期记忆 (Short-term Memory)

当前章节阅读过程中的即时状态：

```yaml
current_chapter: ch02
reading_position: paragraph_12
current_tension: rising
recent_impressions:
  - "诺娃的战斗描写非常有压迫感"
  - "培养皿的揭露节奏恰到好处"
pending_concerns:
  - "kian 的电磁短刀破坏控制阀有点太巧了"
```

### Feedback 类型与路由

Reader 的反馈自带路由信息，分发到正确的层：

```yaml
feedback:
  - type: fact_inconsistency        # → 路由到 Layer 1
    chapter: ch07
    description: "Kian 在第三章丢了面具，但第七章又戴着了"
    severity: critical
    route_to: engine

  - type: dramatic_issue             # → 路由到 Layer 2
    chapter: ch05
    description: "诺娃的信仰崩塌来得太突然，缺少铺垫"
    suggestion: "在 ch04 加入诺娃发现第一个异常的伏笔"
    severity: major
    route_to: director

  - type: prose_quality              # → 路由到 Layer 3
    chapter: ch03
    paragraph: 7
    description: "这段打斗写得太流水账，缺少感官细节"
    severity: minor
    route_to: author

  - type: positive_signal            # → 存档为正反馈
    chapter: ch02
    paragraph: 15
    description: "培养皿揭露的视觉冲击力极强"
    route_to: archive
```

### 3↔4 通信协议

#### 3→4 (Author → Reader)

```
AuthorToReader {
  chapter:  String,
  text:     String,
}
```

#### 4→3 (Reader → Author, 或路由到其他层)

```
ReaderFeedback {
  feedbacks:  Vec<Feedback>,   // 每条 feedback 自带 route_to
}
```

#### 循环逻辑

```
loop {
    reader_feedback = reader.read(chapter, text);

    // 按路由分发
    for fb in reader_feedback {
        match fb.route_to {
            Engine   => engine_issues.push(fb),    // 上抛到 Layer 1
            Director => director_issues.push(fb),  // 上抛到 Layer 2
            Author   => author_issues.push(fb),    // 本层处理
            Archive  => positive_signals.push(fb), // 存档
        }
    }

    if author_issues.is_empty() {
        break;  // 本层收敛
    }

    // Author 根据 Reader 的纯文笔反馈修改文本
    text = author.revise(text, author_issues);
}

// 如果有上抛的问题，触发外层循环
if !engine_issues.is_empty() {
    // 回到 1↔2 循环
}
if !director_issues.is_empty() {
    // 回到 2↔3 循环
}
```

**收敛条件：** Reader 无 critical/major 反馈，或所有反馈已被路由到正确的层处理。

---

## 非线性编辑 (吃书机制)

### 核心原理

每层都是其输入的纯函数（Reader 除外）。因此：

```
Layer 1:  snapshot[N] = fold(genesis, effects[1..N])          // 确定性
Layer 2:  prompt[N]   = director(snapshot[N], drama[N])       // 确定性
Layer 3:  text[N]     = author(prompt[N])                     // 确定性 (memoizable)
Layer 4:  memory[N]   = reader(memory[N-1], text[N])          // 有状态，但可从头 replay
```

### 变更传播流程

当用户修改了 `effects[ch03]`（吃书）：

```
修改点: effects[ch03]
         │
         ▼
Layer 1: 从 ch03 开始重新 fold → 生成 new_snapshot[ch03..ch_last]
         │
         │  对每个受影响的章节:
         ▼
         diff = new_snapshot[N] - old_snapshot[N]
         │
         ├── diff 为空 → 提前终止，后续章节不受影响
         │
         └── diff 非空 → 通知 Layer 2
                  │
                  ▼
Layer 2:  new_prompt[N] = director(new_snapshot[N], drama[N])
          │
          ├── new_prompt[N] == old_prompt[N] → skip Layer 3
          │
          └── new_prompt[N] != old_prompt[N] → 通知 Layer 3
                   │
                   ▼
Layer 3:   text[N] = author(new_prompt[N])   ← 消耗 token，但精确定位到受影响章节
                   │
                   ▼
Layer 4:   reader.update_memory(N, new_text[N])  ← 重读受影响章节
```

### 成本模型

| 步骤 | 成本 | 说明 |
|------|------|------|
| Layer 1 重新 fold | **零 token** | 纯确定性 replay |
| Layer 1 Datalog 推理 | **零 token** | Nemo 引擎本地计算 |
| Layer 1→2 diff 比较 | **零 token** | 结构化 snapshot diff |
| Layer 2 prompt 校验 | **零 token** | 字符串比较 |
| Layer 3 文本重写 | **高 token** | 但仅限 diff 命中的章节 |
| Layer 4 记忆更新 | **中 token** | 但仅重读变化的文本 |

**关键优化：** diff 提前终止 + prompt memoization，使得大部分吃书操作只需要重写 1-2 个章节的文本。

### What-If 分析

```bash
# 试试 "如果 Kian 在第 2 章就死了会怎样？"
everlore whatif ch02 --effect "kill(kian)" --dry-run

# 输出：
# ch02: snapshot changed (kian.status → dead)
# ch03: prompt changed (kian 不再出现在 participants 中)
# ch04: prompt changed (所有 kian 相关剧情需要重写)
# ch05: prompt unchanged (kian 已不在场景中)
# 
# 受影响章节: 3 / 7
# 需要重写的文本: ch02, ch03, ch04
# 预计 token 消耗: ~15000
#
# 执行? [y/N]
```

---

## 每层日志格式

### Layer 1: Effect Log

```jsonl
{"chapter":"ch03","seq":1,"op":"remove_item","entity":"kian","item":"旧式防毒面具"}
{"chapter":"ch03","seq":2,"op":"add_item","entity":"nova","item":"旧式防毒面具"}
{"chapter":"ch03","seq":3,"op":"reveal","secret":"oasis_truth","to":"kian"}
```

- 纯结构化，可精确 replay
- 每条记录是一个原子操作
- 支持插入、删除、修改（通过 seq 定位）

### Layer 2: Intent Log

```jsonl
{"chapter":"ch03","intent":"confrontation","between":["kian","nova"],"depends_on":["kian.location","nova.location"]}
{"chapter":"ch03","intent":"reversal","target":"nova","secret":"oasis_truth","timing":"climax"}
{"chapter":"ch03","pacing":{"build_up":0.5,"climax":0.35,"resolution":0.15}}
```

- 半结构化，声明依赖字段
- 依赖字段用于 diff 传播时的快速过滤：
  `if intent.depends_on ∩ diff.changed_fields == ∅ → skip`

### Layer 3: Prompt Hash Log

```jsonl
{"chapter":"ch03","prompt_hash":"a1b2c3d4","version":1,"timestamp":"2026-03-28T22:00:00Z"}
{"chapter":"ch03","prompt_hash":"e5f6g7h8","version":2,"timestamp":"2026-03-28T22:30:00Z","reason":"layer1_diff"}
```

- 只记录 prompt 的 hash —— Author 无记忆，只需判断 input 是否变了
- 版本号用于追溯重写历史

### Layer 4: Memory Log

```jsonl
{"chapter":"ch03","type":"character_update","char":"nova","observation":"信仰崩塌","emotional_impact":"high"}
{"chapter":"ch03","type":"world_fact","fact":"绿洲能源来自培养皿","confidence":1.0}
{"chapter":"ch03","type":"unresolved","question":"财阀高层是否知情？"}
{"chapter":"ch03","type":"feedback","route":"director","severity":"major","desc":"铺垫不足"}
```

- 唯一有状态的日志
- 按章节索引，支持回溯重建

---

## 嵌套收敛模型

三个循环按**由外到内的顺序收敛**：

```
Phase 1: 1↔2 收敛 (世界状态 + 剧情意图一致)
│
│  前几章高频迭代，建立世界观和主线冲突
│  之后趋于稳定
│
└→ Phase 2: 2↔3 收敛 (文本实现了剧情意图)
   │
   │  每章 1-2 轮谈判，处理创造性偏差
   │
   └→ Phase 3: 3↔4 收敛 (文本通过读者体验检验)
      │
      │  最高频循环，每段文本需要打磨
      │
      └→ 如果 Reader 发现事实错误 → 打断，回到 Phase 1
         如果 Reader 发现剧情问题 → 打断，回到 Phase 2
```

### 成熟项目的迭代模式

```
章节数 →  1   2   3   4   5   6   7   8   9   10
1↔2 轮次:  5   4   3   2   1   1   1   1   1   1    ← 前期高，后期稳定
2↔3 轮次:  2   2   2   2   2   2   2   2   2   2    ← 恒定
3↔4 轮次:  3   3   3   3   3   3   3   3   3   3    ← 恒定
```

---

## 文件结构

```
mynovel/
├── outlines/                     # 场景大纲（可选，Director 可自动生成）
│   ├── ch01.md
│   └── ch02.md
├── drafts/                       # 最终文本输出 (Layer 3 产物)
│   ├── ch01.md
│   └── ch02.md
└── .everlore/
    ├── config.toml               # 项目配置
    │
    ├── entities/                  # Layer 1: Genesis State（唯一人工编辑入口）
    │   ├── kian.json              #   基础属性
    │   ├── kian.yaml              #   欲望树
    │   ├── nova.json
    │   ├── nova.yaml
    │   ├── oasis_core.json        #   地点
    │   └── secrets.yaml           #   信息披露定义
    │
    ├── history.jsonl              # Layer 1: Effect Log (append-only)
    │
    ├── snapshots/                 # Layer 1: 缓存的每章快照
    │   ├── ch01.json
    │   └── ch02.json
    │
    ├── drama/                     # Layer 2: Drama Nodes
    │   ├── ch01.yaml
    │   └── ch02.yaml
    │
    ├── intents.jsonl              # Layer 2: Intent Log
    │
    ├── prompts/                   # Layer 2→3: 缓存的 prompt (用于 memoize)
    │   ├── ch01.md
    │   └── ch02.md
    │
    ├── prompt_hashes.jsonl        # Layer 3: Prompt Hash Log
    │
    ├── reader/                    # Layer 4: Reader Memory
    │   ├── long_term.yaml         #   长期记忆
    │   ├── memory.jsonl           #   Memory Log
    │   └── feedback.jsonl         #   反馈日志
    │
    ├── facts.rls                  # Datalog: 自动生成的事实
    ├── rules.rls                  # Datalog: 用户自定义推理规则
    └── results/                   # Datalog: 推理结果
```

---

## 与当前 EverLore v1 的关系

| 概念 | v1 (当前) | v2 (本架构) |
|------|----------|------------|
| 状态源 | entity JSON + history.jsonl | 不变，但新增 secrets.yaml |
| 推理 | Datalog (Nemo) | 不变，新增 `dramatic_irony` 等规则 |
| 叙事生成 | `everlore narrate` (单步) | Layer 2 + 3 循环生成 |
| 视角控制 | `--pov` flag | 不变，集成到 Layer 2 的 prompt 构造 |
| 非线性编辑 | `everlore rollback` (粗粒度) | diff 传播 + prompt memoize (精细粒度) |
| 质量控制 | `everlore suggest` (图分析) | Layer 4 Reader (语义级反馈) |
| 信息控制 | 无 | secrets / known_by / revealed_at |
| 剧情设计 | 人工写 outline | Layer 2 Drama Node (结构化意图) |

> [!NOTE]
> v1 的核心（角色 JSON、欲望树、Datalog 推理、POV 过滤、Event Sourcing）全部保留。
> v2 主要是在 v1 之上叠加了：Director 意图层、Reader 记忆层、diff 传播机制、信息披露层。
