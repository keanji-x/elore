---
seq: 1
effects:
  - "Move(char_id, location_id)"
  - "AddTrait(char_id, 新特质)"
  - "Reveal(secret_id, char_id)"
created_by: human
---

节拍的叙事文本。在 cards/phases/{phase_id}/ 下创建编号文件 (001.md, 002.md, ...)。

支持的 effects:
  Move(entity, location)         AddTrait(entity, value)
  RemoveTrait(entity, value)     AddItem(entity, item)
  RemoveItem(entity, item)       SetBelief(entity, old, new)
  Reveal(secret, character)      RevealToReader(secret)
  AddRel(entity, target, rel)    RemoveRel(entity, target)
  AddDesire(entity, value)       RemoveDesire(entity, value)
  ResolveGoal(owner, goal, sol)  FailGoal(owner, goal)
  EmergeGoal(owner, goal, want)
  AddConnection(loc, target)     RemoveConnection(loc, target)
  AddMember(faction, member)     RemoveMember(faction, member)

<!-- 删除此模板文件后运行 elore build -->
