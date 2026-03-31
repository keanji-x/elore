---
type: character
id: liu_bang
name: "刘邦"
traits: [隐忍, 善于用人, 圆滑]
beliefs: [天下可取]
desires: [称王关中, 保全性命]
intentions: []
location: bashang
relationships:
  - target: zhang_liang
    role: 谋士
    trust: 3
    affinity: 3
    respect: 2
  - target: fan_kuai
    role: 部将
    trust: 3
    affinity: 2
    respect: 1
  - target: xiang_yu
    role: 对手
    trust: -2
    affinity: -1
    respect: 1
inventory: [白璧一双, 玉斗一双]
goals:
  - id: survive_feast
    want: 从鸿门宴全身而退
    problem: 项羽意图杀之
    status: active
  - id: claim_guanzhong
    want: 称王关中
    problem: 项羽兵力远胜
    status: background
    conflicts_with:
      - xiang_yu/dominate
tags:
  - active
---

# 刘邦

沛县亭长出身，率军先入关中，约法三章，深得民心。性格圆滑隐忍，善于听取谋士意见。
