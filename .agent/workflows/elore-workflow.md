---
description: EverLore 小说创作工作流 — 从项目初始化到完整章节
---

# EverLore 创作工作流

基于 EverLore v2 (`ledger → resolver → executor → evaluator`) 实际开发经验总结的最佳实践。

---

## 前置: 安装 CLI

```bash
cargo install --path crates/elore-cli
# 或直接使用 debug build:
alias elore="$(pwd)/target/debug/elore"
cargo build -p elore-cli
```

---

## Phase 0: 项目初始化

### 1. 创建项目目录并初始化

```bash
mkdir my_novel && cd my_novel
elore init
```

会生成:
```
.everlore/
  entities/       ← 所有实体 JSON/YAML
  drama/          ← 每章 drama node
  history.jsonl   ← append-only 事件日志
drafts/           ← 生成的 prompt 和草稿
```

### 2. 创建实体 scaffold

```bash
# 主角
elore new character kian --name "基安"
elore new character nova --name "诺娃"

# 地点
elore new location wasteland --name "废土"
elore new location oasis_gate --name "绿洲之门"

# 势力（可选）
elore new faction council --name "长老会"
```

### 3. 手动填充实体数据

打开 `.everlore/entities/kian.json`, 填写 BDI:

```json
{
  "type": "character",
  "id": "kian",
  "name": "基安",
  "traits": ["拾荒者", "坚韧"],
  "beliefs": ["水源是生存的根本"],
  "desires": ["找到活水源"],
  "intentions": [],
  "location": "wasteland",
  "relationships": [{"target": "nova", "rel": "wary"}],
  "inventory": ["电磁短刀", "旧式防毒面具"],
  "tags": ["active"]
}
```

> **最佳实践**: `beliefs` 写角色**当前相信为真**的命题; `desires` 写**驱动行动**的欲望; `intentions` 写**本章计划的行动**。

### 4. 添加秘密 (可选)

编辑 `.everlore/entities/secrets.yaml`:

```yaml
secrets:
  - id: oasis_truth
    content: 绿洲的能源来自地下活人培养皿
    known_by: []
    revealed_to_reader: false
    dramatic_function: reversal
```

秘密的 `dramatic_function`:
- `reversal` — 反转
- `suspense` — 悬疑
- `foreshadowing` — 伏笔
- `misdirection` — 误导

---

## Phase 1: 章节规划 (每章开始前)

### 5. 查看当前态势

```bash
elore status      # 项目概览
elore plan        # 悬念 / 冲突 / 信息不对称
```

`elore plan` 输出类似:
```
悬念 (未解目标):
  ● kian/survive_drought: 找到干净水源 — 荒野污染严重

信息不对称:
  oasis_truth — Suspense [已知: 无人 | 读者: ✗]
```

### 6. 确认当前快照

```bash
elore snapshot ch01
```

检查:
- ✓ 角色位置是否符合上章结尾
- ✓ 物品 / 特质是否正确
- ✓ 秘密状态是否反映剧情进展

### 7. 编写 Drama Node

创建 `.everlore/drama/ch01.yaml`:

```yaml
chapter: ch01
dramatic_intents:
  - type: confrontation
    between: [kian, nova]
    at: oasis_gate
  - type: secret_reveal
    secret: oasis_truth
    to: [kian]
    reveal_to_reader: true
pacing:
  build_up: 0.4
  climax: 0.4
  resolution: 0.2
director_notes:
  pov: kian
  tone: "紧张, 充满未知"
  tone_arc: "好奇 → 恐惧 → 震撼"
  word_count: 3000
  highlights:
    - "基安穿越沙暴接近绿洲"
    - "诺娃城墙上瞄准基安"
    - "基安偷看到培养皿"
  required_effects:
    - op: move
      entity: kian
      location: oasis_gate
  suggested_effects:
    - op: remove_item
      entity: kian
      item: 旧式防毒面具
```

`dramatic_intents` 可选类型:
- `confrontation` — 正面冲突 (需要所有参与者在同一地点)
- `secret_reveal` — 秘密揭露
- `reversal` — 命运反转
- `suspense_resolution` — 悬念解决 (需要 goal 存在)
- `goal_emergence` — 新目标出现
- `character_development` — 角色成长
- `tension_build` — 张力积累 (无前置条件)

### 8. 验证 Drama Intent

```bash
elore validate ch01
```

如果不通过, 常见原因:
- 角色不在指定地点 → 先用 `history add` 提交 move effect
- 秘密已被揭露 → 修改 drama intent
- 角色不存在 → 检查 entity ID 拼写

---

## Phase 2: 生成 Author Prompt

### 9. 生成 Prompt

```bash
elore write ch01 --pov kian
# → drafts/ch01_prompt.md
```

或使用 `run` 一键完成所有检查 + 生成:

```bash
elore run ch01 --pov kian
```

### 10. 审阅 Prompt

打开 `drafts/ch01_prompt.md`, 检查:
- ✓ POV 角色的 BDI 完整
- ✓ 非 POV 角色标注 `[非视角角色]` 且 BDI 隐藏
- ✓ 戏剧性目标逻辑正确
- ✓ 关键节拍覆盖

> **最佳实践**: prompt 里的 `## 必须体现的状态变化` 一定要告诉 LLM
> 用 `«effect: op(args)»` 标注, 否则 read 阶段无法提取。

---

## Phase 3: 写作 (Author 层)

### 11. LLM 写作 / 手动写作

将 `drafts/ch01_prompt.md` 内容发给 LLM, 或自行写作。

**Effect 标注格式** — 在叙事文本中内联:

```text
基安扯下了面具扔到地上。«effect: remove_item(kian, 旧式防毒面具)»

他穿过了城门。«effect: move(kian, oasis_gate)»

他看到了培养皿。«effect: reveal(oasis_truth, kian)»
```

支持的 effect DSL:
```
move(entity, location)
add_trait(entity, value)
remove_trait(entity, value)
add_item(entity, item)
remove_item(entity, item)
set_belief(entity, old_belief, new_belief)
reveal(secret_id, character_id)
reveal_to_reader(secret_id)
resolve_goal(owner, goal_id, solution)
fail_goal(owner, goal_id)
emerge_goal(owner, goal_id, want)
```

保存到 `drafts/ch01.md`。

---

## Phase 4: 审核与提交

### 12. 读者审核

```bash
elore read ch01
```

输出:
```
字数: 760
提取到 3 个 effects:
  • kian: 失去物品 "旧式防毒面具"
  • kian: 移动至 oasis_gate
  • 秘密 oasis_truth 揭示给 kian

═══ Audit ═══
✓ 无一致性问题
```

如有错误:
- `invalid_entity` → Effect 引用了不存在的实体 ID
- `required_effect_missing` → Drama node 中的必须 effect 没出现

### 13. 提交 Effects 到历史日志

```bash
elore history add ch01 "remove_item(kian, 旧式防毒面具)"
elore history add ch01 "move(kian, oasis_gate)"
elore history add ch01 "reveal(oasis_truth, kian)"
```

> **最佳实践**: `elore read` 输出的 effects 直接复制到 `history add`。

### 14. 验证状态演化

```bash
elore snapshot ch01   # 确认状态已正确更新
elore plan            # 查看新的态势
```

---

## Phase 5: 章节间过渡

### 15. 查看章节 diff

```bash
elore diff ch00 ch01
```

```
Δ kian (character):
  location: wasteland → oasis_gate
  - item: 旧式防毒面具
```

### 16. What-If 分析 (非线性写作 / 吃书)

修改前先做 What-If:

```bash
elore whatif ch01 --effect "add_trait(kian, 目睹真相)"
```

输出影响范围, 确认无副作用后再提交。

### 17. 规划下一章

```bash
elore status          # 确认整体状态
elore plan            # 查看新的悬念 / 冲突态势
cp .everlore/drama/ch01.yaml .everlore/drama/ch02.yaml
# 修改 ch02.yaml...
```

---

## 完整命令速查

```bash
# 初始化
elore init
elore new character <id> --name "名字"
elore new location <id> --name "名字"

# 规划
elore status
elore plan [--chapter ch03]
elore snapshot <chapter>
elore validate <chapter>
elore drama show <chapter>

# 生成
elore write <chapter> --pov <id>
elore run <chapter> --pov <id>

# 提交
elore read <chapter>
elore history add <chapter> "<effect_dsl>"
elore history list [--chapter <ch>]
elore history rollback <chapter>

# 分析
elore diff <from> <to>
elore whatif <chapter> --effect "<dsl>"
```

---

## 目录结构约定

```
my_novel/
├── .everlore/
│   ├── entities/
│   │   ├── kian.json           ← character
│   │   ├── nova.json           ← character
│   │   ├── wasteland.json      ← location
│   │   ├── kian_goals.yaml     ← goal tree (可选)
│   │   └── secrets.yaml        ← 所有秘密
│   ├── drama/
│   │   ├── ch01.yaml
│   │   └── ch02.yaml
│   └── history.jsonl           ← 自动维护, 不要手动编辑
├── drafts/
│   ├── ch01_prompt.md          ← elore write 生成
│   └── ch01.md                 ← 最终草稿 (带 effect 标注)
└── final/
    └── ch01.md                 ← elore read 清洗后的干净文本
```

---

## 常见问题

**Q: validate 说角色不在指定地点怎么办?**
先用 `history add` 提交前置 move effect, 再 validate。

**Q: 想回滚上一章的所有 effects?**
```bash
elore history rollback ch03
```

**Q: 如何处理 POV 限制?**
`director_notes.pov` 设定后, prompt 会自动隐藏非 POV 角色的 beliefs/desires。
`pov_constraints` 可以额外写"不能展示的信息"。

**Q: drama node 里的 `required_effects` 有什么用?**
Author 写完后, `elore read` 会检查这些 effect 是否出现在文本中。
没出现 → Audit 报 `required_effect_missing` 错误。
