# EverLore v3 — 最佳实践

> 基于一次完整创作 loop 的经验总结。项目: 废土科幻短篇《第七服务器》

---

## 一、工作流顺序

### 世界构建阶段（Genesis）

**推荐：用 `add entities` 批量创建，系统自动处理顺序。**

```bash
elore add entities '[
  {"id":"kian","type":"character","location":"oasis_gate"},
  {"id":"oasis_gate","type":"location","name":"绿洲基站"}
]'
# 顺序无所谓，系统自动拓扑排序（地点先于角色）
```

如果用 `add entity`（单条），仍需手动保证顺序：
```
地点 → 角色（带 location） → 派系（带 members）
```

秘密的 dramatic_function 取值（snake_case）：
```
"suspense" | "reversal" | "foreshadowing" | "misdirection" | "dramatic_irony"
```

---

### Phase 设计阶段

每个 Phase 设计时问三个问题：

1. 这个 Phase 结束时，世界实质性地改变了什么？→ 写 exit_state
2. 这个 Phase 要发生多少戏剧事件？→ 设 min_effects
3. 读者读完这段应该有什么感觉？→ 设 tone_arc

Phase 依赖用 depends_on 列表，approve 时自动解锁下一 Phase。

---

### Beat 写作阶段

Beat 是最小原子提交单元，一个 checkout 内可提交多个 Beat。

理想 Beat 大小：200-400 字（一个场景节拍）。太大难以标注，太小叙事张力不足。

CLI 最佳实践——用 Python 序列化 JSON（中文文本含单引号时 Shell 会出错）：

```python
import subprocess, json

payload = json.dumps({
    "text": "含有换行和引号的叙事文本",
    "effects": ["move(kian, oasis_gate)"],
    "created_by": "ai"
})
subprocess.run(["elore", "add", "beat", payload], cwd=project_dir)
```

---

### 标注阶段（L4）

Score 标准：
- 5: 超出预期，值得重读
- 4: 达标，有亮点
- 3: 合格，但平淡
- 2: 低于平均，需要注意
- 1: 失败，建议重写

推荐 Tags：tension / character / atmosphere / dialogue / reversal / foreshadowing / action / boring / redundant

---

### 提交与审阅

```bash
elore status          # 检查四层约束是否全绿
elore submit          # active → reviewing
elore approve         # reviewing → approved（自动解锁下一 Phase）
elore reject "原因"   # reviewing → active（退回修改）
```

---

## 二、已知 Gotcha

1. **单条 `add entity` 仍需拓扑顺序**：改用 `add entities` 批量创建可绕过此限制

2. Beat 序号不随失败回滚（已知 Bug），beat 文件会被删除，内容不受影响

3. ~~entity_alive 依赖 genesis 文件存在~~ **已修复**：beat 的 effects 提交后自动写入 `history.jsonl`，snapshot 会 replay 它们，L1 invariant 检查的是真实当前状态

4. ~~dramatic_function 的 enum：dramaticirony（无下划线）~~ **已修复**：现在用标准 snake_case，直接写 `"dramatic_irony"`

5. 含单引号文本必须用 Python `json.dumps()` 序列化

**调试技巧**：beat 提交后查看 `history.jsonl` 确认 effects 已写入：
```bash
cat .everlore/history.jsonl
# 期望看到: {"chapter":"phase_id","seq":100,"op":"move",...}
```

---

## 三、设计哲学

### Phase vs Chapter
- Phase = 叙事功能单元（角色弧、信息揭露、冲突节点）
- Chapter = 字数切割单元（出版格式）
- 一个 Phase 可对应任意多章节，words 约束是 guideline 不是截断

### 约束即剧本
Phase constraints 本质上是给 AI 写的叙事大纲。tone_arc、guidance、pov 都是软性写作约束，会体现在 elore read phase 的输出里。

### AI 和 Human 都是第一公民
- AI 提交 Beat，Human（或 AI）加标注
- "by": "ai" 和 "by": "human" 的标注都有效
- Human 是唯一的 approve 门控

---

## 四、完整 AI Agent 工作流

```
1. elore read phase                     # 读取 phase 定义和约束
2. elore read snapshot <id> --format json  # 读取世界状态
3. 按 writing_plan 顺序提交 Beat（用 Python）
4. elore status --format json           # 确认 complete: true
5. elore add note <json>                # AI 自评标注
6. elore submit                         # 请求 human 审阅
7. 如果 reject → 重写低分 Beat → 重新 submit
```

---

## 五、推荐 Phase 粒度参考

| Phase 类型 | min_effects | words | 关键约束 |
|-----------|-------------|-------|---------|
| 相遇 (Encounter) | 2-3 | 800-2000 | exit_state: 角色汇聚 |
| 冲突 (Conflict) | 5+ | 1500-4000 | invariant 严格 |
| 揭露 (Revelation) | 1-2 | 600-1500 | exit: knows() 断言 |
| 高潮 (Climax) | 5+ | 1500-3000 | min_avg_score: 4.0 |
| 结局 (Resolution) | 2-3 | 500-1500 | exit_state 严格定义 |

---

## 六、官方示例项目

在 `examples/neon_shadows` 目录下包含了一个完整的赛博朋克风小说示例。该项目是由 AI 扮演自动化 Agent，通过纯 CLI 命令生成的。

你可以查看 `examples/neon_shadows/build_neon_shadows.py` 这个脚本，它完整演示了如何用 Python `subprocess` 调用 `elore`，并提交 JSON 来驱动一个真正的工作流：从建立世界、分配秘密、设定 Phase 到最终提交对应的剧情 Beats。

要运行或测试它：
```bash
cd examples/neon_shadows
python3 build_neon_shadows.py
elore status
```

---
*最后更新：三个结构性 bug 修复后 (2026-03-29)*
