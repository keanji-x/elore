use colored::Colorize;
use std::path::Path;

const ELORE_WORKFLOW: &str = r##"---
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
  rules/*.dl                   ← 自定义推理规则 (可选)

.everlore/                     ← 构建产物 (用 `elore build` 重新生成)
  entities/*.json              ← 实体缓存
  phases/*.yaml                ← 阶段定义
  beats/*.json                 ← 节拍缓存
  history.jsonl                ← 从节拍卡编译
  state.json                   ← 阶段生命周期状态
  secrets.yaml                 ← 秘密缓存

packs/                         ← 扩展包 (可选)
  {pack_id}/pack.yaml          ← 扩展包清单
  {pack_id}/cards/             ← 预制卡片
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
intent_targets:                  # 结构化意图 → 推导 threatens/plots_against
  - action: 寻找
    target: oasis
desire_tags: [find_water]        # 标准化标签 → 推导 alliance_opportunity
location: sector_4
relationships:
  - target: nova
    role: 前同事
    trust: 1
    affinity: 1
    respect: 0
    # facade_affinity: 2         # 可选：表面亲疏 → 推导 deceiving
    # facade_respect: 3          # 可选：表面敬畏
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

关系三轴 (-3 ~ +3)：
- **trust** (信任): 影响 would_confide, betrayal_opportunity
- **affinity** (亲疏): 影响 would_sacrifice, personal_bond, enemy 覆盖
- **respect** (敬畏): 影响 would_obey, pressure_to_harm

facade 字段（可选）表示角色**表面表现**的关系值，与真实值不同时推导出 deceiving。

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
leader: nova                     # 决策者 → 推导 must_submit
members: [nova, vex]
rivals: [free_runners]
strength: 50000                  # 可选：兵力 → 推导 power_advantage
tags:
  - active
---

# 星联公司

leader + members + rivals 驱动推理引擎的 enemy/must_submit/power_advantage 推导。
personal_bond (affinity >= 2) 可覆盖 faction enemy。
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
  - "move(kian, the_spire)"
  - "remove_trait(kian, 犹豫)"
  - "reveal(nexus_project, kian)"
created_by: human
---

雨水顺着基安的衣领滑下。他站在尖塔的入口，...
```

`seq` 决定节拍在 phase 内的顺序。`effects` 是 Op DSL 字符串列表（小写）。

### 支持的 Effect Ops

```
move(entity, location)
add_trait(entity, value)         remove_trait(entity, value)
add_item(entity, item)           remove_item(entity, item)
set_belief(entity, old, new)
reveal(secret, character)        reveal_to_reader(secret)
add_rel(entity, target, role)    remove_rel(entity, target)
add_desire(entity, value)        remove_desire(entity, value)
resolve_goal(owner, goal, sol)   fail_goal(owner, goal)
emerge_goal(owner, goal, want)
add_connection(loc, target)      remove_connection(loc, target)
add_member(faction, member)      remove_member(faction, member)
```

### 四层约束

每个 Phase 可以设定四层约束：

| 层 | 关注 | 硬/软 |
|---|------|-------|
| L1 Ledger | 状态不变量、exit assertions、worldbuilding 计数 | 硬性阻断 |
| L2 Resolver | min_effects、戏剧意图 | 硬性阻断 |
| L3 Executor | 字数、POV、语调、语调弧 | 软性指导 |
| L4 Evaluator | min_avg_score、max_boring_beats、required_tags | 审批必须 |

---

## 推理引擎

`elore suggest` 内置 Datalog 推理引擎（via Nemo），从当前世界快照自动推导叙事张力，零 LLM token 消耗。

### 推导谓词

**威胁与保护：**

| 谓词 | 含义 | 数据要求 |
|------|------|----------|
| `threatens(A, B)` | A 意图伤害 B | intent_targets |
| `plots_against(A, B)` | A 威胁 B + 持有 B 不知道的秘密 | intent_targets + secrets |
| `armed_danger(A, B)` | threatens + 能见面 | intent_targets + location |
| `active_danger(A, B)` | armed_danger 或 personal_enemy 相遇 | 同上 |
| `protector(A, B)` | A 愿牺牲自己保护 B + B 面临 armed_danger | affinity >= 3, trust >= 1 |
| `indirect_protector(A, B)` | A 经由第三人的忠诚链保护 B | 传递 would_sacrifice 链 |

**内心冲突：**

| 谓词 | 含义 | 数据要求 |
|------|------|----------|
| `pressure_to_harm(A, B, X)` | X 施压 A 对 B 动手 | would_obey + threatens |
| `pressure_to_spare(A, B, Y)` | Y 求情 A 放过 B | would_sacrifice + would_shield |
| `torn(A, B)` | A 同时被施压伤害和保护 B | 以上两者同时存在 |

**权力与欺骗：**

| 谓词 | 含义 | 数据要求 |
|------|------|----------|
| `power_advantage(F1, F2)` | F1 实力 >= 2x F2 | faction strength |
| `must_submit(A, B)` | A 的阵营远弱于 B 所在阵营 + B 是对方 leader | strength + leader |
| `deceiving(A, B)` | A 对 B 的真实 affinity <= 0 但表面 >= 2 | facade_affinity |
| `deceived(B, A)` | B 被 A 欺骗且不疑 | facade + trust >= 0 |

**信息不对称：**

| 谓词 | 含义 | 数据要求 |
|------|------|----------|
| `dramatic_irony(S, C)` | 读者知道秘密 S 但角色 C 不知道 | secrets |
| `critical_reveal(S, I, U)` | 高优先级信息传递（排除对敌人的） | secrets + trust/obey |
| `info_cascade(S, B, T)` | 秘密经信任链传播 | secrets + trust |
| `betrayal_opportunity(P, V, S)` | P 持有秘密 + 对 V 有敌意 + 能见面 | secrets + affinity |

**社交与联盟：**

| 谓词 | 含义 | 数据要求 |
|------|------|----------|
| `enemy(A, B)` | 敌对阵营成员（personal_bond 可覆盖） | factions |
| `personal_bond(A, B)` | affinity >= 2 → 覆盖阵营敌对 | relationships |
| `alliance_opportunity(A, B, T)` | 共同 desire_tag + 非敌对 | desire_tags |
| `would_confide(A, B)` | trust >= 2 + 能见面 | relationships |
| `would_obey(A, B)` | respect >= 2 | relationships |
| `would_sacrifice(A, B)` | affinity >= 3 + trust >= 1 | relationships |

**目标与悬念：**

| 谓词 | 含义 | 数据要求 |
|------|------|----------|
| `suspense(Owner, Goal)` | 活跃目标无解法 | goals |
| `active_conflict(OA, GA, OB, GB)` | 冲突双方目标都活跃 | goals + conflicts_with |
| `orphaned_secret(S)` | 读者知道但无角色知晓 | secrets |

### 数据质量 → 推理质量

推理引擎的信噪比取决于卡片数据的精度：

- **relationships** 的 trust/affinity/respect 值最关键 — 差一个数字可能导致整条推导链断裂
- **intent_targets** 驱动 threatens/armed_danger/protector 链
- **desire_tags** 用标准化 ID（非自然语言）确保 alliance 匹配
- **facade_affinity/respect** 只在角色有演技时填写
- **leader + strength** 只在需要权力对比时填写

### Snapshot Time Travel

推理引擎支持跨时间点对比。对任意两个 beat 的快照运行推理后 diff：

- **emerged (涌现)**：这一拍新产生了什么张力
- **resolved (消解)**：这一拍解决了什么张力

例：`move(liu_bang, hongmen)` 一个操作 → 17 条张力同时涌现（torn, armed_danger, must_submit...）。
`reveal(sword_dance, liu_bang)` → dramatic_irony 消解 + 新行动可能性涌现。

---

## 扩展包

预制的剧情场景，可安装到任何项目：

```bash
elore pack list                  # 列出可用扩展包
elore pack info hongmenyan       # 查看详情
elore pack install hongmenyan    # 安装到 cards/
```

扩展包 = `pack.yaml` 清单 + `cards/` 目录。安装后即为普通卡片。
搜索路径：项目内 `packs/` → 全局 `~/.elore/packs/`。

---

## 命令速查

```bash
elore init                         # 初始化项目
elore build                        # 编译 cards → .everlore/
elore status                       # 项目概览
elore plan                         # 态势面板 + 推理摘要
elore suggest                      # 推理引擎张力分析 + 约束建议
elore pack list/info/install       # 扩展包管理
```

---

## 关键原则

1. **Cards 是唯一真相源** — 永远不要手动编辑 `.everlore/` 下的文件
2. **CLI 是编译器** — `elore build` 把 cards 编译成缓存和历史
3. **反向同步** — 节拍卡里的 effects 会自动更新实体卡的 YAML
4. **事件溯源** — 所有状态变化都是 Ops，history.jsonl 从节拍卡编译
5. **先构建世界** — 第一个 Phase 用 Worldbuilding 类型，确保实体和关系足够丰富
6. **推理驱动写作** — `elore suggest` 让 Datalog 引擎推导叙事张力，再决定写什么
7. **关系精度决定推理质量** — trust/affinity/respect 的值直接影响推导链的触发
"##;

// ── Card templates ──────────────────────────────────────────────

const CHARACTER_TEMPLATE: &str = r#"---
type: character
id: example_char
name: "角色名"
traits: [勇敢, 谨慎]
beliefs: [正义终将胜利]
desires: [找到真相]
intentions: []
intent_targets: []               # 结构化意图: [{action: 刺杀, target: enemy_id}]
desire_tags: []                  # 标准化标签: [protect_ally, seek_power]
location: example_location
relationships:
  - target: another_char
    role: 同伴
    trust: 2                     # -3~+3: 信任 → would_confide, betrayal
    affinity: 2                  # -3~+3: 亲疏 → would_sacrifice, personal_bond
    respect: 1                   # -3~+3: 敬畏 → would_obey, pressure
    # facade_affinity: 3         # 可选: 表面亲疏 (与真实值不同 → deceiving)
    # facade_respect: 3          # 可选: 表面敬畏
inventory: [长剑]
goals:
  - id: example_goal
    want: 达成某个目标
    problem: 面临某个障碍
    status: active
tags:
  - active
---

# 角色名

角色的背景描述。这段文字是自由格式的 Markdown。

<!-- 删除此模板文件后运行 elore build -->
"#;

const LOCATION_TEMPLATE: &str = r#"---
type: location
id: example_location
name: "地点名"
properties: [险峻, 灵气充沛]
connections: [another_location]
tags:
  - active
---

# 地点名

地点的描述。

<!-- 删除此模板文件后运行 elore build -->
"#;

const FACTION_TEMPLATE: &str = r#"---
type: faction
id: example_faction
name: "势力名"
leader: example_char             # 决策者 → must_submit 只针对 leader
members: [example_char]
rivals: [enemy_faction]
# strength: 10000               # 可选: 兵力 → power_advantage 推导
tags:
  - active
---

# 势力名

势力的描述。
- leader + members + rivals 驱动 enemy/must_submit/power_advantage 推导
- personal_bond (affinity >= 2) 可覆盖 faction 敌对关系

<!-- 删除此模板文件后运行 elore build -->
"#;

const SECRET_TEMPLATE: &str = r#"---
id: example_secret
content: 秘密的具体内容
known_by: []
revealed_to_reader: false
dramatic_function: suspense
---

秘密的补充说明。dramatic_function 可选: reversal, suspense, foreshadowing, misdirection

known_by + revealed_to_reader 驱动推理引擎的 dramatic_irony / possible_reveal 推导。

<!-- 删除此模板文件后运行 elore build -->
"#;

const BEAT_TEMPLATE: &str = r#"---
seq: 1
effects:
  - "move(char_id, location_id)"
  - "add_trait(char_id, 新特质)"
  - "reveal(secret_id, char_id)"
created_by: human
---

节拍的叙事文本。在 cards/phases/{phase_id}/ 下创建编号文件 (001.md, 002.md, ...)。

支持的 effects:
  move(entity, location)         add_trait(entity, value)
  remove_trait(entity, value)    add_item(entity, item)
  remove_item(entity, item)      set_belief(entity, old, new)
  reveal(secret, character)      reveal_to_reader(secret)
  add_rel(entity, target, rel)   remove_rel(entity, target)
  add_desire(entity, value)      remove_desire(entity, value)
  resolve_goal(owner, goal, sol) fail_goal(owner, goal)
  emerge_goal(owner, goal, want)
  add_connection(loc, target)    remove_connection(loc, target)
  add_member(faction, member)    remove_member(faction, member)

<!-- 删除此模板文件后运行 elore build -->
"#;

/// Write a template file only if it doesn't already exist.
fn write_template(path: &Path, content: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        std::fs::write(path, content)?;
    }
    Ok(())
}

/// Initialize a new EverLore project.
pub fn run(project: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let everlore = project.join(".everlore");
    let phases = everlore.join("phases");
    let beats = everlore.join("beats");
    let annotations = everlore.join("annotations");
    let entities_cache = everlore.join("entities");

    // cards/ — source of truth
    let cards = project.join("cards");
    let cards_characters = cards.join("characters");
    let cards_locations = cards.join("locations");
    let cards_factions = cards.join("factions");
    let cards_secrets = cards.join("secrets");
    let cards_phases = cards.join("phases");

    // Create cards/ directories
    std::fs::create_dir_all(&cards_characters)?;
    std::fs::create_dir_all(&cards_locations)?;
    std::fs::create_dir_all(&cards_factions)?;
    std::fs::create_dir_all(&cards_secrets)?;
    std::fs::create_dir_all(&cards_phases)?;

    // Create .everlore/ directories (build artifacts)
    std::fs::create_dir_all(&phases)?;
    std::fs::create_dir_all(&beats)?;
    std::fs::create_dir_all(&annotations)?;
    std::fs::create_dir_all(&entities_cache)?;

    // Template files — show AI the card format
    write_template(&cards_characters.join("_template.md"), CHARACTER_TEMPLATE)?;
    write_template(&cards_locations.join("_template.md"), LOCATION_TEMPLATE)?;
    write_template(&cards_factions.join("_template.md"), FACTION_TEMPLATE)?;
    write_template(&cards_secrets.join("_template.md"), SECRET_TEMPLATE)?;
    write_template(&cards_phases.join("_template.md"), BEAT_TEMPLATE)?;

    // .everlore/.gitignore — all build artifacts
    let gitignore_path = everlore.join(".gitignore");
    if !gitignore_path.exists() {
        std::fs::write(&gitignore_path, "# Build artifacts — regenerate with `elore build`\n*\n!.gitignore\n!phases/\n!phases/**\n")?;
    }

    // .agent/workflows/elore-workflow.md — AI skill for using elore
    let workflow_dir = project.join(".agent").join("workflows");
    std::fs::create_dir_all(&workflow_dir)?;
    let workflow_path = workflow_dir.join("elore-workflow.md");
    if !workflow_path.exists() {
        std::fs::write(&workflow_path, ELORE_WORKFLOW)?;
    }

    println!("{}", "✓ EverLore 项目已初始化".green().bold());
    println!("  {} (source of truth)", cards.display());
    println!("    characters/ locations/ factions/ secrets/ phases/");
    println!("  {} (build artifacts)", everlore.display());
    println!("  {} (AI workflow)", workflow_path.display());
    Ok(())
}

/// Create a new entity card scaffold.
pub fn new_entity(
    project: &Path,
    entity_type: &str,
    id: &str,
    name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let subdir = match entity_type {
        "character" => "characters",
        "location" => "locations",
        "faction" => "factions",
        _ => {
            return Err(
                format!("未知类型: {entity_type} (可选: character, location, faction)").into(),
            );
        }
    };

    let cards_dir = project.join("cards").join(subdir);
    std::fs::create_dir_all(&cards_dir)?;

    let path = cards_dir.join(format!("{id}.md"));
    if path.exists() {
        return Err(format!("实体 {id} 已存在: {}", path.display()).into());
    }

    let display_name = name.unwrap_or(id);
    let content = match entity_type {
        "character" => format!(
            "---\ntype: character\nid: {id}\nname: \"{display_name}\"\ntraits: []\nbeliefs: []\ndesires: []\nintentions: []\nrelationships: []\ninventory: []\ntags:\n  - active\n---\n\n# {display_name}\n\n"
        ),
        "location" => format!(
            "---\ntype: location\nid: {id}\nname: \"{display_name}\"\nproperties: []\nconnections: []\ntags:\n  - active\n---\n\n# {display_name}\n\n"
        ),
        "faction" => format!(
            "---\ntype: faction\nid: {id}\nname: \"{display_name}\"\nmembers: []\nrivals: []\ntags:\n  - active\n---\n\n# {display_name}\n\n"
        ),
        _ => unreachable!(),
    };

    std::fs::write(&path, content)?;

    println!(
        "{} {} {}",
        "✓".green().bold(),
        entity_type.cyan(),
        id.bold()
    );
    println!("  → {}", path.display());
    Ok(())
}
