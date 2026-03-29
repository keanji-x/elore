# EverLore v3 — Phase-Driven Narrative Compiler

## 1. 核心定位

EverLore 不是交互式写作工具，是**叙事编译器**。

它管理的是**叙事状态图**——角色、关系、秘密、目标构成的时序属性图。
它的输入是世界定义 + 戏剧约束，输出是满足所有约束的叙事文本。

AI 和人类是对等的一等公民。工具不区分调用者身份，只提供统一的读写 API。

---

## 2. 基本概念

### 2.1 Phase（戏剧弧线）

Phase 是创作的**工作单元**。不是章节——章节是按字数切分的输出格式，是派生品。

一个 Phase 代表一段有明确戏剧目的的叙事弧线。它携带四层约束，全部满足才算完成。

```
Plan（整部作品）
  └── Phase（戏剧弧线，有四层约束）
       └── Beat（单次写作迭代，原子提交）

Chapter = 跨 Phase、按字数切分的输出段落（派生品）
```

### 2.2 Beat（原子提交）

Beat 是单次写作的原子单位，绑定**文本 + effects**：

```json
{
  "phase": "confrontation",
  "seq": 3,
  "text": "基安跪在地上，干呕了几声……",
  "effects": ["reveal(oasis_truth, kian)"],
  "word_count": 1200
}
```

为什么绑定：
- **因果清晰**：这段文字导致了这些状态变化
- **可回滚**：删掉一个 beat = 同时撤销文字和效果
- **可修订**：`{"revises": 3, ...}` 替换旧版，旧版归档

### 2.3 四层约束

每个 Phase 包含四层独立的约束，分别对应四个 crate：

| 层 | Crate | 关注点 | 检查时机 |
|----|-------|--------|---------|
| L1 · State | `ledger` | 世界一致性 | 每个 beat 后 |
| L2 · Drama | `resolver` | 剧情推进 | status 查询时 |
| L3 · Writing | `executor` | 文本产出 | status 查询时 |
| L4 · Reader | `evaluator` | 主观质量 | 标注后 |

Phase 完成 = **四层全部满足**。

---

## 3. 四层约束详细设计

### 3.1 L1 · State（Ledger 层）

**职责**：保证世界状态始终一致。

```yaml
ledger:
  # 不变量：每个 beat 后自动检查，违反则拒绝 beat
  invariants:
    - entity_alive(kian)
    - entity_alive(nova)
    - "oasis_truth 不能在 kian 之前揭示给读者"

  # 退出状态：submit 时检查，不满足则不能提交审阅
  exit_state:
    kian.location: oasis_gate
    can_meet(kian, nova): true          # Datalog 查询
    knows(kian, oasis_truth): true
```

**invariants vs exit_state 的区别**：
- invariant 是**过程约束**——写作过程中任何时刻都不能违反
- exit_state 是**结果约束**——Phase 结束时必须达到的状态

exit_state 支持 Datalog 查询，不只是简单属性比较：

```yaml
exit_state:
  dramatic_irony(oasis_truth): true     # 读者不知道但 kian 知道
  active_conflict(kian, nova): true     # 两人有活跃冲突
  reachable(wasteland, oasis_gate): true
```

### 3.2 L2 · Drama（Resolver 层）

**职责**：保证戏剧弧线完成。

```yaml
resolver:
  synopsis: "基安穿越废土接近绿洲，与诺娃对峙，发现培养皿秘密"

  # 必须发生的戏剧事件
  intents:
    - type: confrontation
      between: [kian, nova]
      satisfied_by: can_meet(kian, nova)   # 何时算满足

    - type: secret_reveal
      secret: oasis_truth
      to: kian
      satisfied_by: knows(kian, oasis_truth)

    - type: tension_build
      involving: [kian]
      min_count: 2                        # 至少 2 次张力积累

  # 量化约束
  min_effects: 8
  min_relationship_changes: 1

  # 期望紧张度曲线（标准化到 0-1）
  tension_curve: [0.3, 0.5, 0.9, 0.7]
```

`intents` 的 `satisfied_by` 字段连接到 L1 的 Datalog 查询。Resolver 不自己判断是否满足，而是**向下查询 Ledger**。

### 3.3 L3 · Writing（Executor 层）

**职责**：控制文本产出的量和节奏。

```yaml
executor:
  # 字数约束
  words: [5000, 8000]             # [min, max]
  per_beat: [800, 2000]           # 单个 beat 的字数范围

  # 写作计划（AI 或人类制定，可在运行中修改）
  writing_plan:
    - beat: "沙暴穿越"
      target_words: 1500
      effects: ["move(kian, oasis_gate)"]
      guidance: "第一人称内心独白，强调感官细节"

    - beat: "城墙对峙"
      target_words: 2000
      effects: []
      guidance: "切换到紧张的对话节奏"

    - beat: "发现培养皿"
      target_words: 2000
      effects: ["reveal(oasis_truth, kian)"]
      guidance: "视觉冲击，少用对话，多用动作和心理"

  # 风格指引
  pov: kian
  tone: "紧张, 充满未知"
  tone_arc: "好奇 → 恐惧 → 震撼"
```

writing_plan 不是死板的。AI 可以在运行中调整：
```bash
elore add phase-update '{"layer":"executor","writing_plan":[...]}'
# → 提交给 L2 验证：新计划是否仍覆盖所有 intents
# → 提交给 L1 验证：新 effects 是否仍满足 exit_state
```

### 3.4 L4 · Reader（Evaluator 层）

**职责**：主观质量反馈。AI agent 和人类都可以标注。

```yaml
evaluator:
  constraints:
    min_avg_score: 3.0             # beat 平均分 ≥ 3（1-5 分制）
    max_boring_beats: 0            # 不允许有 score ≤ 2 的 beat
    required_tags: [twist]         # 必须至少有一个 beat 被标记为 twist

  # 标注在运行时累积
  annotations: []
```

标注格式：

```json
{
  "beat": 2,
  "by": "human",
  "tags": ["boring", "dialogue_too_long"],
  "score": 2,
  "note": "对话节奏太慢，基安的犹豫感不够"
}
```

```json
{
  "beat": 3,
  "by": "ai_reader",
  "tags": ["twist", "exciting", "visual"],
  "score": 5,
  "note": "培养皿的揭示用视觉而非对话，冲击力很强"
}
```

可用标签（非穷举）：

| 标签 | 含义 |
|------|------|
| `exciting` | 让人想继续读 |
| `boring` | 节奏拖沓 |
| `twist` | 有效的转折 |
| `forced` | 转折生硬 |
| `emotional` | 引起情感共鸣 |
| `confusing` | 读不懂 |
| `visual` | 画面感强 |
| `dialogue_good` | 对话出彩 |
| `dialogue_bad` | 对话不自然 |
| `pacing_fast` | 节奏太快 |
| `pacing_slow` | 节奏太慢 |

---

## 4. Phase 状态机

```
                add phase
                    ↓
               ┌─ locked ←── depends_on 未满足
               ↓
              ready ←── depends_on 全部 approved
               ↓
             checkout
               ↓
     ┌───→  active  ←────────────────────┐
     │        ↓                          │
     │    add beat / add note            │
     │        ↓                          │
     │    status (四层检查)              │
     │        ↓                          │
     │    全部 ✓？                       │
     │      NO → 继续 ──────────────────→┘
     │      YES ↓
     │      submit
     │        ↓
     │    reviewing
     │        ↓
     │    approve → approved (永久锁定)
     └─── reject ──→ 返回 active
```

**状态转换规则**：

| 转换 | 条件 |
|------|------|
| locked → ready | 所有 depends_on phases 都 approved |
| ready → active | `elore checkout <phase>` |
| active → reviewing | `elore submit`，要求四层约束全部满足 |
| reviewing → approved | `elore approve` |
| reviewing → active | `elore reject "原因"` |
| approved → (不可变) | Phase 锁定，beats 和 effects 不可修改 |

---

## 5. 层间反馈与验证链

### 5.1 正向生成

```
L1 (State)    →  构建世界快照
L2 (Drama)    →  编译 prompt（状态 + 剧情约束 + 剩余目标）
L3 (Writing)  →  生成文本（beat）
L4 (Reader)   →  标注质量
```

### 5.2 反向修订

当任何层触发修改时，变更必须**逐层向上提交验证**：

```
L4 标记 beat boring
  → L3 重写 beat（新文本 + 可能不同的 effects）
    → L2 重新验证：新 effects 是否仍覆盖 intents？
      → L1 重新验证：世界状态是否仍一致？
        → 全部通过 → 接受修改
        → 任何一层失败 → 拒绝修改，报告冲突
```

```
L3 修改 writing_plan（调整 words/pacing）
  → L2 验证：新计划是否仍覆盖所有 intents？
    → L1 验证：新 effects 是否仍满足 exit_state？
```

```
L2 修改 intent（添加/移除剧情目标）
  → L1 验证：新目标是否与现有世界状态兼容？
```

**关键原则**：任何层的修改都不能绕过上层验证。这保证了**修订不引入新的不一致**。

### 5.3 冲突检测

当两层的约束矛盾时，系统不应该让 AI 反复重试。应当检测冲突并 escalate：

```json
{
  "conflict": true,
  "layers": ["resolver", "evaluator"],
  "detail": {
    "resolver": "intent 'secret_reveal' 要求在 beat 3-4 揭露 oasis_truth",
    "evaluator": "beat 3 被标记 'forced'(score:2)，secret_reveal 太突兀"
  },
  "suggestions": [
    "在 beat 2 增加铺垫（executor.writing_plan 调整）",
    "将 secret_reveal 移到 beat 4-5（resolver.intents 调整）",
    "降低 evaluator.max_boring_beats 约束"
  ]
}
```

冲突检测触发条件：同一 beat 被修订超过 N 次（默认 3 次）。

---

## 6. Prompt 编译

`elore read prompt` 的输出不是静态模板，而是根据**当前进度动态编译**的上下文：

### 6.1 Phase 早期（进度 < 30%）

```markdown
# 写作上下文

## 当前阶段: confrontation (进度 15%)

## 世界状态
[完整 snapshot]

## 本阶段目标
- 基安与诺娃对峙
- 揭露 oasis_truth 给基安
- 至少 8 个 effects

## 写作计划
下一个 beat: "沙暴穿越"
目标字数: ~1500
风格: 第一人称内心独白，强调感官细节
```

### 6.2 Phase 中期（30% - 70%）

```markdown
## 进度
已完成 2/3 beats, 3500/5000-8000 字
已完成 intents: confrontation ✓
未完成 intents: secret_reveal

## 已发生的事
[前面 beats 的摘要]

## 下一个 beat: "发现培养皿"
目标字数: ~2000
必须包含: reveal(oasis_truth, kian)
```

### 6.3 Phase 尾声（> 70%）

```markdown
## ⚠️ 阶段即将结束

剩余字数空间: ~1500
未满足约束:
  - exit_state: knows(kian, oasis_truth) = false
  - intent: secret_reveal 未完成

请在本次 beat 中推进以上目标。
```

这就是"编译器"的含义：**同样的源文件（phase 定义 + 世界状态），根据运行时进度产生不同的编译输出**。

---

## 7. CLI 设计

### 7.1 统一读写 API

```bash
# ─── 写入 ─────────────────────────────────────────────
elore add entity  <json>        # 创建/更新实体（部分字段默认填充）
elore add phase   <json>        # 定义 phase（四层约束，可只写 synopsis）
elore add beat    <json>        # 提交写作（文本 + effects，原子）
elore add effect  <dsl>         # 单独提交状态变化（不绑定文本）
elore add secret  <json>        # 添加秘密
elore add note    <json>        # 标注 beat 质量

# ─── 读取 ─────────────────────────────────────────────
elore read snapshot [--format json]   # 当前世界状态
elore read phase    [--format json]   # 当前 phase 约束 + 进度
elore read prompt                     # 编译好的写作上下文（纯文本）
elore read beats    [--format json]   # 已写的 beats
elore read history  [--format json]   # effect 历史

# ─── 状态机 ───────────────────────────────────────────
elore checkout <phase>           # 进入 phase
elore status   [--format json]   # 四层约束满足度
elore submit                     # 提交审阅
elore approve                    # 批准锁定
elore reject   "原因"            # 打回

# ─── 产出 ─────────────────────────────────────────────
elore export [--split-words 3000]  # 按字数切章节输出
```

### 7.2 所有命令 AI/人类对等

工具不区分调用者。同一个命令，AI 加 `--format json`，人类看彩色输出：

```bash
# 人类
elore status
# → 彩色表格，emoji 进度条

# AI
elore status --format json
# → 结构化 JSON，可编程解析
```

---

## 8. AI 写作循环（伪代码）

```python
# 1. 人类或 AI 定义世界和计划
elore add entity '{"id":"kian",...}'
elore add phase  '{"id":"setup","synopsis":"..."}'
elore add phase  '{"id":"confrontation","depends_on":["setup"],...}'

# 2. 进入 phase
elore checkout setup

# 3. 自动写作循环
while True:
    status = json(elore status --format json)

    if status["complete"]:
        elore submit
        break

    # 检查是否有 beat 需要修订（evaluator 反馈）
    if status["evaluator"]["low_beats"]:
        beat_id = status["evaluator"]["low_beats"][0]["beat"]
        feedback = status["evaluator"]["low_beats"][0]["note"]
        # 重写 beat，提交修订版
        prompt = elore read prompt    # prompt 会包含修订指引
        new_text = llm(prompt + f"请重写 beat {beat_id}。反馈：{feedback}")
        elore add beat '{"revises": beat_id, "text": "...", "effects": [...]}'
        continue

    # 正常写作
    prompt = elore read prompt
    text = llm(prompt)
    effects = extract_effects(text)
    elore add beat '{"text": "...", "effects": [...]}'

    # AI 自我审阅（可选）
    elore add note '{"beat": N, "by": "ai_reader", "score": 4, "tags": [...]}'

# 4. 等待人类审阅
# → elore approve / elore reject "..."

# 5. 进入下一个 phase
elore checkout confrontation
# 重复循环
```

---

## 9. 数据结构

```
.everlore/
  state.json               # 全局状态机：current_phase, 各 phase 状态
  entities/                 # 世界状态（被 effects 持续修改）
    kian.json
    nova.json
    secrets.yaml
  phases/                   # Phase 定义（四层约束）
    setup.yaml
    confrontation.yaml
    aftermath.yaml
  beats/                    # 原子提交（text + effects）
    setup_001.json
    setup_002.json
    confrontation_001.json
    confrontation_002.json
    confrontation_002_r1.json   # beat 2 的修订版 (revision 1)
  annotations/              # 质量标注
    setup.jsonl
    confrontation.jsonl
```

### 9.1 state.json

```json
{
  "current_phase": "confrontation",
  "plan": ["setup", "confrontation", "aftermath"],
  "phases": {
    "setup": {
      "status": "approved",
      "beats": 3,
      "words": 4800,
      "effects": 5,
      "approved_at": "2026-03-29T10:00:00Z"
    },
    "confrontation": {
      "status": "active",
      "beats": 2,
      "words": 3200,
      "effects": 3,
      "checked_out_at": "2026-03-29T10:30:00Z"
    },
    "aftermath": {
      "status": "locked"
    }
  }
}
```

### 9.2 Phase 定义（完整示例）

```yaml
id: confrontation
order: 2
depends_on: [setup]
synopsis: "基安与诺娃在绿洲门前对峙，意外发现培养皿秘密"

# ═══ L1 · State ═══
ledger:
  invariants:
    - entity_alive(kian)
    - entity_alive(nova)
  exit_state:
    kian.location: oasis_gate
    can_meet(kian, nova): true
    knows(kian, oasis_truth): true

# ═══ L2 · Drama ═══
resolver:
  intents:
    - type: confrontation
      between: [kian, nova]
    - type: secret_reveal
      secret: oasis_truth
      to: kian
    - type: tension_build
      involving: [kian]
      min_count: 2
  min_effects: 8
  min_relationship_changes: 1

# ═══ L3 · Writing ═══
executor:
  words: [5000, 8000]
  per_beat: [800, 2000]
  pov: kian
  tone: "紧张, 充满未知"
  tone_arc: "好奇 → 恐惧 → 震撼"
  writing_plan:
    - beat: "沙暴穿越"
      target_words: 1500
      effects: ["move(kian, oasis_gate)"]
      guidance: "第一人称内心独白，强调感官细节"
    - beat: "城墙对峙"
      target_words: 2000
      guidance: "切换到紧张的对话节奏"
    - beat: "发现培养皿"
      target_words: 2000
      effects: ["reveal(oasis_truth, kian)"]
      guidance: "视觉冲击，少用对话"

# ═══ L4 · Reader ═══
evaluator:
  min_avg_score: 3.0
  max_boring_beats: 0
  required_tags: [twist]
```

### 9.3 Beat 记录

```json
{
  "phase": "confrontation",
  "seq": 3,
  "revises": null,
  "text": "基安跪在地上，干呕了几声。在铁栅栏的背后……",
  "effects": [
    "reveal(oasis_truth, kian)"
  ],
  "word_count": 1200,
  "created_at": "2026-03-29T11:00:00Z",
  "created_by": "ai"
}
```

修订版：

```json
{
  "phase": "confrontation",
  "seq": 3,
  "revises": 3,
  "revision": 1,
  "text": "（修订后的文本）",
  "effects": ["reveal(oasis_truth, kian)"],
  "word_count": 1400,
  "created_at": "2026-03-29T11:30:00Z",
  "created_by": "ai",
  "revision_reason": "原版 '发现' 场景太突兀，增加铺垫"
}
```

---

## 10. 约束自动推导

为降低配置负担，`elore add phase` 只需要 `id` 和 `synopsis`，四层约束可由 AI 推导：

```bash
elore add phase '{"id":"confrontation","synopsis":"基安与诺娃在绿洲门前对峙"}'
```

AI 基于 synopsis + 现有世界状态推导：

| 层 | 自动推导逻辑 |
|----|-------------|
| L1 | 从 synopsis 提取涉及角色 → 生成 invariants（角色不能死）；从 "对峙" 推断 exit_state 需要 can_meet |
| L2 | 从 "对峙" 推断 confrontation intent；从涉及角色 + 秘密推断可能的 reveal |
| L3 | 根据 Phase 在 plan 中的位置推断字数；根据 intents 数量推断 beat 数 |
| L4 | 使用全局默认值（min_avg_score: 3.0, max_boring: 0） |

人类可以覆盖任何自动推导的约束：

```bash
elore add phase-update '{"id":"confrontation","executor":{"words":[8000,12000]}}'
```

---

## 11. 风险与应对

### 11.1 约束冲突死循环

**场景**：L2 要求 reveal，L4 说 reveal 太突兀，L3 反复重写都不满足。

**应对**：beat 被修订超过 3 次 → 触发冲突检测 → 输出冲突报告 + 建议 → escalate 给人类决策。

### 11.2 配置负担

**场景**：每个 Phase 写 50 行约束太重。

**应对**：synopsis-only 模式 + AI 自动推导约束 + 合理默认值。

### 11.3 验证成本

**场景**：每个 beat 后跑完整 Datalog 推理太慢。

**应对**：增量验证——只检查本次 beat 的 effects 涉及的实体和关系，不重跑全部。缓存上次 snapshot，只 apply diff。

### 11.4 长篇小说的 Phase 数量

**场景**：50 万字小说有 100+ Phases。

**应对**：Phase 支持层级嵌套：
```yaml
- id: act1
  children: [setup, confrontation, aftermath]
- id: act2
  depends_on: [act1]
  children: [...)
```

---

## 12. 与 v2 实现的关系

| v2 组件 | v3 变化 |
|---------|---------|
| `ledger` (crate) | 不变。加 `invariant_check()`, `exit_state_check()` |
| `resolver` (crate) | DramaNode → Phase.resolver 约束。validate 逻辑不变 |
| `executor` (crate) | Writer → Beat 管理。track words/pacing |
| `evaluator` (crate) | Audit → Annotation + Score 系统 |
| `elore-cli` | 重构：checkout/status/submit/approve 状态机 |
| `history.jsonl` | 从 beats/*.json 的 effects 派生（不再手动管理） |
| `drama/ch01.yaml` | → `phases/confrontation.yaml` |
| `drafts/ch01.md` | → `beats/` 中的 text 按字数切章输出 |
| `elore run ch01` | → `elore checkout <phase>` + 自动循环 |
| `elore snapshot ch01` | → `elore read snapshot`（不再绑定章节） |

**底层引擎（entity, effect, graph, Datalog）完全保留。变的是编排层。**

---

## 13. 实现优先级

### P0 — 最小可用

1. Phase 定义 + state.json 状态机
2. `checkout / status / submit / approve / reject`
3. Beat 提交 + L1 invariant 检查
4. `read prompt` 动态编译

### P1 — 完整循环

5. L2 drama 约束检查
6. L3 writing 约束 + writing_plan
7. L4 annotation + score
8. 反向验证链

### P2 — 生产级

9. 约束自动推导（synopsis → 四层约束）
10. 冲突检测 + escalation
11. 增量验证优化
12. `elore export` 按字数切章
13. Phase 嵌套（act → phase 层级）
