---
description: EverLore 小说创作工作流 — Card-Based 叙事编译器
---

# EverLore 创作工作流

EverLore 是一个叙事编译器。`cards/*.md` 是唯一的 source of truth，`.everlore/` 是构建产物。
CLI 的核心命令是 `elore build`，它将 cards 编译为 `.everlore/` 缓存，并反向同步 effect 状态回 card YAML。

---

## 目录结构

```
cards/                         ← source of truth (人类可编辑)
  characters/*.md              ← 角色卡
  locations/*.md               ← 地点卡
  factions/*.md                ← 势力卡
  secrets/*.md                 ← 秘密卡
  phases/{phase_id}/*.md       ← 节拍卡 (001.md, 002.md, ...)

.everlore/                     ← 构建产物 (用 `elore build` 重新生成)
  entities/*.json              ← 实体缓存
  phases/*.yaml                ← 阶段定义
  beats/*.json                 ← 节拍缓存
  history.jsonl                ← 从节拍卡编译
  state.json                   ← 阶段生命周期状态
  secrets.yaml                 ← 秘密缓存
```

---

## Phase 0: 世界构建 (Worldbuilding)

第一个 Phase 应设为 `phase_type: worldbuilding`，专注于构造角色卡、地点卡、势力卡和秘密卡。

### 创建角色卡

在 `cards/characters/` 下创建 Markdown 文件：

```markdown
---
type: character
id: kian
name: "基安"
traits: [谨慎, 聪敏]
beliefs: [水源是生存的根本]
desires: [找到活水源]
intentions: []
location: sector_4
relationships:
  - target: nova
    role: 前同事
    trust: 1
    affinity: 1
    respect: 0
inventory: [电磁短刀]
goals:
  - id: find_water
    want: 找到干净水源
    problem: 荒野污染严重
    status: active
    conflicts_with:
      - nova/guard_oasis
tags:
  - active
---

# 基安

前星联公司安全顾问。性格沉稳，擅长分析。
```

YAML frontmatter = 结构化数据，Markdown body = 自由描述。
goals 字段驱动推理引擎的 suspense/conflict 推导。

### 创建地点卡

```markdown
---
type: location
id: the_spire
name: "尖塔"
properties: [高层建筑, 监控密布]
connections: [sector_4, undercity]
tags:
  - active
---

# 尖塔

星联公司的总部大楼，矗立在第四区的中心。
```

### 创建势力卡

```markdown
---
type: faction
id: nexus_corp
name: "星联公司"
members: [nova, vex]
rivals: [free_runners]
tags:
  - active
---

# 星联公司

members 和 rivals 驱动推理引擎的 enemy/danger 推导。
```

### 创建秘密卡

在 `cards/secrets/` 下：

```markdown
---
id: nexus_project
content: 星联的"连接计划"实际上是意识上传实验
known_by: []
revealed_to_reader: false
dramatic_function: reversal
---

这个秘密将在第二幕揭露，作为剧情反转的核心。
```

`dramatic_function` 可选: `reversal`, `suspense`, `foreshadowing`, `misdirection`

### 编译构建

```bash
elore build
```

这会：
1. 解析所有 cards → 写入 `.everlore/` 缓存
2. 从节拍卡提取 effects → 编译 `history.jsonl`
3. 反向同步：effect 产生的状态变化自动回写到 card YAML

### Worldbuilding Phase 约束

Worldbuilding phase 可以设定最低实体数量和关系密度：

```yaml
constraints:
  l1:
    min_entities:
      characters: 5
      locations: 3
      factions: 2
      secrets: 3
    min_rel_density: 1.5   # 平均每个角色至少 1.5 条关系
```

---

## Phase 1+: 叙事写作 (Narrative)

### 创建节拍卡

在 `cards/phases/{phase_id}/` 下创建编号的 Markdown 文件：

```markdown
---
seq: 1
effects:
  - "Move(kian, the_spire)"
  - "RemoveTrait(kian, 犹豫)"
  - "Reveal(nexus_project, kian)"
created_by: human
---

雨水顺着基安的衣领滑下。他站在尖塔的入口，...
```

`seq` 决定节拍在 phase 内的顺序。`effects` 是 Op DSL 字符串列表。

### 支持的 Effect Ops

```
Move(entity, location)
AddTrait(entity, value)
RemoveTrait(entity, value)
AddItem(entity, item)
RemoveItem(entity, item)
SetBelief(entity, old, new)
Reveal(secret, character)
RevealToReader(secret)
ResolveGoal(owner, goal_id, solution)
FailGoal(owner, goal_id)
EmergeGoal(owner, goal_id, want)
AddConnection(location, target)
RemoveConnection(location, target)
AddMember(faction, member)
RemoveMember(faction, member)
```

### 四层约束

每个 Phase 可以设定四层约束：

| 层 | 关注 | 硬/软 |
|---|------|-------|
| L1 Ledger | 状态不变量、exit assertions、worldbuilding 计数 | 硬性阻断 |
| L2 Resolver | min_effects、戏剧意图 | 硬性阻断 |
| L3 Executor | 字数、POV、语调、语调弧 | 软性指导 |
| L4 Evaluator | min_avg_score、max_boring_beats、required_tags | 审批必须 |

### 数据流

```
cards/*.md → elore build → .everlore/ (缓存 + history.jsonl) → Snapshot
                        ↓
              反向同步: effects 自动更新 card YAML frontmatter
```

---

## 推理引擎

`elore suggest` 和 `elore plan` 内置 Datalog 推理引擎（via Nemo），从当前世界快照自动推导叙事可能性，零 LLM token 消耗。

### 推导谓词

| 谓词 | 含义 | 示例 |
|------|------|------|
| `can_meet(A, B)` | 同一地点的角色 | 苍幽与顾长风都在太乙宗 |
| `enemy(A, B)` | 敌对势力成员 | 需要 faction 填写 members/rivals |
| `danger(A, B)` | 敌人相遇 | enemy + can_meet |
| `betrayal_opportunity(P, V, S)` | 持有秘密 + 能见面 | 苍幽可对顾长风发动（秘密: cangyou_plan） |
| `possible_reveal(S, I, U)` | 知情者与不知情者同处一地 | 苍幽可向顾长风透露秘密 |
| `info_cascade(S, B, T)` | 秘密传播路径 | cangyou_plan: 苍幽 → 顾长风 |
| `dramatic_irony(S, C)` | 读者知道但角色不知道 | 林默对 cangyou_plan 一无所知 |
| `alliance_opportunity(A, B, W)` | 共同欲望 | 两个角色都渴望同一件事 |
| `suspense(Owner, Goal)` | 活跃目标无解法 | 需要 goals 数据 |
| `active_conflict(OA, GA, OB, GB)` | 冲突双方都活跃 | 需要 goals + conflicts_with |
| `goal_conflict_encounter(OA, GA, OB, GB)` | 冲突双方能见面 | active_conflict + can_meet |
| `orphaned_secret(S)` | 读者知道但无角色知晓 | 需要安排角色发现此秘密 |

### 数据要求

推理引擎的输出质量取决于 cards 的填充程度：

- **角色卡**: `location`、`relationships`、`desires`、`goals` 越丰富，推导越多
- **秘密卡**: `known_by`、`revealed_to_reader` 驱动信息不对称推理
- **势力卡**: `members`、`rivals` 驱动敌对关系推理（空则 enemy/danger 为空集）
- **目标数据**: `goals` 在角色卡中定义，`conflicts_with` + `blocked_by` 驱动冲突推理

### 工作流集成

推理引擎自动接入以下命令：

| 命令 | 使用方式 |
|------|----------|
| `elore suggest` | 完整展示所有叙事可能性 + L1/L2 约束建议 |
| `elore plan` | 态势板末尾追加推理摘要 |
| `elore validate` | 用 `can_meet` 验证戏剧意图的物理可行性 |
| prompt 生成 | "推理结果" 段落注入 LLM prompt |

---

## 命令速查

```bash
elore init                         # 初始化项目
elore build                        # 编译 cards → .everlore/
elore status                       # 项目概览
elore plan                         # 悬念/冲突/信息不对称 + 推理摘要
elore suggest                      # 推理引擎叙事可能性 + 约束建议
```

---

## 关键原则

1. **Cards 是唯一真相源** — 永远不要手动编辑 `.everlore/` 下的文件
2. **CLI 是编译器** — `elore build` 把 cards 编译成缓存和历史
3. **反向同步** — 节拍卡里的 effects 会自动更新实体卡的 YAML
4. **事件溯源** — 所有状态变化都是 Ops，history.jsonl 从节拍卡编译
5. **先构建世界** — 第一个 Phase 用 Worldbuilding 类型，确保实体和关系足够丰富
6. **推理驱动写作** — `elore suggest` 让 Datalog 引擎推导可能事件，再决定写什么
