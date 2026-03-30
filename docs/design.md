# Elore — 核心设计

## 1. 设计目标

Elore 的目标不是“自动把小说写漂亮”。
它的目标是把长篇叙事创作里最容易失控的部分收束成一个可靠流程：

1. 当前到底在写哪一段叙事任务
2. 这段文本到底让世界发生了什么变化
3. 这段任务到底是否真的完成
4. 在长期创作中，谁知道什么、谁在哪里、关系如何变化，能否始终保持一致

所以 Elore 的核心价值不是文风生成，而是：

**把叙事过程从“靠脑子硬撑”变成“可读、可查、可审阅、可回放的状态系统”。**

---

## 2. 核心定位

Elore 是一个 **phase-first 的叙事操作系统**。

它不是：
- chapter-first 的写作工具
- prompt 集合
- metadata 装饰层
- 全自动故事发现引擎

它是：
- 一个用 phase 管理创作任务的系统
- 一个用 beat 提交文本和状态变化的系统
- 一个用 snapshot 保持 continuity 的系统
- 一个用 status / review 管理完成度的系统

如果用一句话概括：

**Elore 不负责替作者产生灵感，它负责让作者不那么容易把故事写崩。**

---

## 3. 设计原则

### 3.1 Phase-first

创作控制流只围绕 `phase`。

原因很简单：
- chapter 更像输出切片
- phase 才是稳定的创作任务单元
- status / submit / approve 只有绑定 phase 才有清晰语义

因此：
- planning 用 phase
- writing 用 phase
- review 用 phase
- completion 用 phase

chapter 只应该出现在导出和阅读层，而不是创作控制层。

### 3.2 状态优先于文本

文本可以精彩，也可以平淡。
但无论文本如何，系统首先要知道：

- 谁移动了
- 谁知道了什么
- 谁和谁关系变了
- 谁身上新增了什么状态
- 哪些目标推进或失败了

如果一段文本不能被映射到这些变化，它对 Elore 来说就没有完成结构化提交。

### 3.3 结构必须参与创作，而不是事后补录

如果用户可以先把小说写完，再机械补 effect，那么结构层就会退化成外壳。

所以 Elore 的设计必须强制结构参与创作过程：
- beat 绑定 effects
- phase 定义先于提交
- submit 依赖真实检查
- note/score/tags 回流修改

### 3.4 完成度必须可信

系统绝不能在“什么都没检查”的时候返回“完成”。

因此：
- 空约束不能算完成
- 未标注不能算通过 L4
- 无法自动验证的约束要显式暴露
- submit 必须由评估 gate 住

### 3.5 一套命令，一套心智模型

CLI 不应该同时承载 chapter-first 和 phase-first 两套模型。

如果命令模型混乱，用户会立刻失去信任：
- 不知道该相信文档还是帮助信息
- 不知道当前是哪个版本的工作流
- 不知道哪些能力真的可依赖

因此顶层命令必须服务于单一工作流。

### 3.6 系统必须诚实

方向对但还没实现，不等于已经成熟。

Elore 必须区分：
- 已经可依赖的能力
- 有部分落地但还不能完全自动验证的能力
- 只是未来方向的能力

比如 synopsis-only 推导，如果还只是保守默认值，就必须明确标记为 derived，而不是假装成完整约束。

---

## 4. 核心概念

### 4.1 Project

Project 是整个作品或创作仓库。

它包含：
- entities
- secrets
- goals
- phases
- history
- annotations
- 当前 active phase

Project 不是文本容器。
文本属于 beats，导出文档属于 `gen`。

### 4.2 Phase

Phase 是唯一的工作单元。

一个 phase 代表一段有明确叙事功能的弧线，比如：
- 相遇
- 对峙
- 揭露
- 高潮
- 结局

它必须回答的问题是：
- 这段叙事要完成什么任务
- 结束时世界要变成什么样
- 至少要发生哪些变化
- 文本大概要写到什么程度
- 审阅通过的最低标准是什么

Phase 不是章节标题。
它更像“当前创作合同”。

### 4.3 Beat

Beat 是最小原子提交。

它绑定：
- `text`
- `effects`
- `word_count`
- `revision metadata`

beat 必须绑定 effects，原因有三个：

1. 让文本和状态变化保持一一对应
2. 让修订和回滚保持一致
3. 强制作者或 AI 回答“这段到底推进了什么”

如果没有 effect，系统只能知道你写了很多字，不能知道你推进了多少叙事。

### 4.4 Effect

Effect 是世界状态变化的最小声明。

例如：
- `move(kian, oasis_gate)`
- `reveal(oasis_truth, kian)`
- `add_trait(kian, hunted)`
- `add_rel(kian, nova, hostile)`

Effect 不是文学描述。
它是叙事层面的结构化变化记录。

Elore 只通过 effects 理解“发生了什么”。

### 4.5 Snapshot

Snapshot 是当前世界状态的可读视图，不是草稿。

它来自：

```text
Snapshot = fold(genesis, history up to phase)
```

Snapshot 用来回答：
- 人在哪
- 谁知道哪些秘密
- 谁和谁是什么关系
- 哪些目标仍未完成
- 哪些状态已发生变化

Snapshot 是 continuity 的单一事实源。

### 4.6 Annotation

Annotation 属于审阅层，不属于评论区。

它的作用是把修改压力结构化：
- 哪个 beat 低分
- 为什么低分
- 缺少什么标签
- 哪些问题要优先返工

没有 annotation，说明 L4 还没有真正发生。

---

## 5. 数据流

Elore 的最小数据流是：

```text
Genesis -> Effects -> Snapshot -> Status/Review -> Gen
```

更完整地说：

```text
Project setup
  -> define phase
  -> checkout phase
  -> add beat(text + effects)
  -> history replay
  -> snapshot rebuild
  -> status evaluation
  -> add note / score / tags
  -> submit
  -> approve / reject
  -> gen
```

这个流程的重点不是“生成很快”，而是：

**每一步都能回答现在离完成还差什么。**

---

## 6. 四层约束模型

Elore 采用四层约束，不是因为分层本身优雅，而是因为叙事完成度本来就不是一个维度。

### 6.1 L1 · State

职责：
保证世界状态合法。

L1 处理两类约束：
- `invariants`：写作过程中任何时候都不能违反
- `exit_state`：phase 结束时必须达到

L1 是硬边界。
如果 L1 不过，系统就不能继续假装当前 beat 或 phase 合法。

### 6.2 L2 · Drama

职责：
保证这段 phase 确实推进了叙事任务，而不是只增长字数。

L2 可以包含：
- `min_effects`
- 关系变化数量
- reveal 数量
- 基础 intents

L2 的最低产品价值不是“复杂戏剧本体论”，而是防止空转。

也就是说，L2 首先要回答：

**这段写完以后，故事真的往前走了吗？**

### 6.3 L3 · Writing

职责：
控制文本产出结构。

L3 可验证部分：
- total words
- per beat words
- writing plan 覆盖度

L3 软约束：
- POV
- tone
- tone_arc
- guidance

这些软约束可以存在，但如果还不能自动判定是否满足，系统必须诚实显示，而不是静默算通过。

### 6.4 L4 · Reader

职责：
把“可读性”和“修订压力”纳入流程。

L4 最低必须支持：
- 未标注 = not checked
- 平均分阈值
- 低分 beat 数量限制
- required tags 检查

L4 不是为了追求客观审美分数。
它的作用是把“哪里该改”变成可操作事实。

---

## 7. 生命周期模型

一个 phase 的生命周期应当是：

```text
locked -> ready -> active -> reviewing -> approved
```

### 7.1 locked

前置依赖尚未满足。

### 7.2 ready

可以开始，但还没进入当前工作上下文。

### 7.3 active

当前唯一正在创作的 phase。

Elore 默认同一时间只有一个 active phase。
原因不是技术限制，而是为了抑制长篇创作中的上下文漂移。

### 7.4 reviewing

已经请求审阅，但还没最终通过。

进入 reviewing 的前提不是“作者感觉差不多了”，而是：
- beat 已提交
- status 已检查
- submit gate 已通过

### 7.5 approved

这个 phase 被视为完成，后续 phase 可以依赖它。

approved 的意义是“上游语义已经稳定”。
如果 approved 还能随意漂移，整个依赖链都会失去可靠性。

---

## 8. 命令模型

顶层命令必须只服务于 phase-first 工作流。

当前应保留的核心命令是：

```text
init
new
add entity|entities|secret|phase|beat|note
read snapshot|history|phase|beats
plan
status
checkout
submit
approve
reject
gen
```

这些命令分别承担：

- `add phase`
定义工作单元

- `checkout`
进入当前工作上下文

- `add beat`
提交文本和变化

- `read snapshot`
读取事实状态

- `read phase`
读取约束定义

- `status`
读取真实完成度

- `submit / approve / reject`
管理审阅闭环

- `gen`
输出连续文本

命令的关键不是多，而是语义一致。

---

## 9. 为什么 chapter 不是一等概念

chapter 依然有价值，但只在导出层有价值。

chapter 适合：
- 作为阅读切片
- 作为平台发布单位
- 作为输出文档结构

chapter 不适合：
- 作为创作控制流单位
- 作为约束管理单位
- 作为审阅状态单位

因为 chapter 太容易被字数、节奏、平台规则左右。
phase 才真正对应“这一段叙事要完成什么”。

所以 Elore 的原则是：

**用 phase 创作，用 chapter 导出。**

---

## 10. synopsis-only 的正确理解

synopsis-only 是降低配置门槛的方法，不是魔法。

如果用户只给：

```json
{"id":"confrontation","synopsis":"基安与诺娃在门前对峙"}
```

系统应该做的是：
- 尝试生成一份最小可工作的初稿约束
- 标注来源为 `synopsis_derived`
- 告诉用户哪些部分仍需补充
- 在无法验证的地方保持保守

系统不应该做的是：
- 假装已经完整理解剧情目标
- 生成看起来成熟但其实空洞的约束
- 最后返回 `complete: true`

synopsis-only 的设计原则是：

**宁可保守，也不要装懂。**

---

## 11. 真正的创作闭环

Elore 的真实闭环不是“prompt -> 文本”。

它的闭环应该是：

```text
phase definition
  -> snapshot reading
  -> beat commits
  -> status evaluation
  -> annotation
  -> revision
  -> review state transition
  -> compiled output
```

也就是：

1. 定义当前 phase
2. 理解当前世界状态
3. 提交 beat
4. 检查是否真的推进目标
5. 审阅并标注问题
6. 修订
7. 请求通过
8. 导出文本

这个闭环一旦成立，Elore 就不再是“一个很漂亮的叙事外壳系统”，而是真正影响创作过程的工具。

---

## 12. 系统边界

Elore 不试图解决所有创作问题。

它不直接负责：
- 句子是否足够文学
- 风格是否足够独特
- 哪个转折最天才
- 哪个 reveal 最震撼

它真正负责的是：
- 保证状态一致
- 保证任务边界清晰
- 保证完成度可信
- 保证审阅可以回流

这听起来不浪漫，但它对长篇创作最关键。

因为长篇最容易死的，不是灵感，而是一致性、完成度错觉和失控的修改成本。

---

## 13. 当前实现优先级

如果按产品落地顺序，这个系统应该优先保证：

### P0
- phase-first 命令模型统一
- 空约束不再算完成
- submit 依赖真实 gate
- snapshot / status / review 形成可信闭环

### P1
- L2/L3/L4 里“已知存在但尚未自动验证”的部分显式化
- synopsis-only 推导可读、可解释

### P2
- `plan` 成为真正的创作协同面板
- 能显示当前 phase 的 blockers、未解目标、秘密不对称、下一步建议

### P3
- 更强的创作发现辅助
- 例如建议下一 beat、暴露 tension 缺口、揭示 reveal timing 风险

高级能力可以慢慢长。
但 P0 不稳，后面所有扩展都会建立在不可信的地基上。

---

## 14. 最后总结

Elore 的核心设计可以压缩成一句话：

**用 phase 定义当前工作，用 beat 提交文本与变化，用 snapshot 保持 continuity，用 status 和 review 保证这段故事真的完成。**

这就是它和一般写作工具的根本区别。
