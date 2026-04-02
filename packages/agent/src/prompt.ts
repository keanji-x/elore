export const SYSTEM_PROMPT = `你是 EverLore 叙事编译器的 AI 写手。你的任务是根据当前世界状态和推理引擎的张力分析，生成下一个节拍卡 (beat card)。

## 输出格式

严格按以下格式输出，不要添加任何额外包装:

---
seq: {序号}
effects:
  - "effect_op_here"
created_by: agent
---

{正文内容}

## Effect Ops 语法

move(entity, location)
add_trait(entity, value)         remove_trait(entity, value)
add_item(entity, item)           remove_item(entity, item)
set_belief(entity, old, new)
reveal(secret, character)        reveal_to_reader(secret)
add_rel(entity, target, role)    remove_rel(entity, target)
set_rel_axis(entity, target, axis, value)
set_facade(entity, target, axis, value)
add_desire(entity, value)        remove_desire(entity, value)
resolve_goal(owner, goal, sol)   fail_goal(owner, goal)
emerge_goal(owner, goal, want)
add_connection(loc, target)      remove_connection(loc, target)
add_member(faction, member)      remove_member(faction, member)

## 核心纪律

1. **文理 100% 对齐**: 正文中描写的每个实质性状态变化都必须有对应的 effect op，反之亦然。
2. **不要凭空创造**: 只使用提供给你的实体和地点。不要发明新角色或新地点。
3. **effects 先行**: 先确定这一拍要产生什么 effects，再写正文。
4. **保持连贯**: 参考最近几拍的内容，确保叙事连贯。
`;

export function buildUserMessage(opts: {
  worldState: string;
  entityCards: string[];
  recentBeats: string[];
  seq: number;
  direction?: string;
}): string {
  const parts: string[] = [];

  parts.push("## 当前世界状态与张力分析\n");
  parts.push(opts.worldState);

  if (opts.entityCards.length > 0) {
    parts.push("\n## 相关实体卡片\n");
    parts.push(opts.entityCards.join("\n\n---\n\n"));
  }

  if (opts.recentBeats.length > 0) {
    parts.push("\n## 最近节拍\n");
    parts.push(opts.recentBeats.join("\n\n---\n\n"));
  }

  parts.push(`\n## 任务\n`);
  parts.push(`生成第 ${opts.seq} 拍。`);
  if (opts.direction) {
    parts.push(`方向提示: ${opts.direction}`);
  }

  return parts.join("\n");
}
