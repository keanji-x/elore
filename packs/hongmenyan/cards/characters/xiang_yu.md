---
type: character
id: xiang_yu
name: "项羽"
traits: [勇猛, 刚愎自用, 重情义]
beliefs: [力能扛鼎者当王天下]
desires: [号令天下, 除去隐患]
intentions: []
location: hongmen
relationships:
  - target: fan_zeng
    role: 亚父
    trust: 1
    affinity: 1
    respect: 1
  - target: xiang_bo
    role: 叔父
    trust: 2
    affinity: 3
    respect: 1
  - target: liu_bang
    role: 对手
    trust: -1
    affinity: 0
    respect: 0
inventory: []
goals:
  - id: dominate
    want: 号令天下诸侯
    status: active
    conflicts_with:
      - liu_bang/claim_guanzhong
tags:
  - active
---

# 项羽

楚国名将项燕之孙，力拔山兮气盖世。巨鹿之战大破秦军主力，威震天下。性格刚烈，优柔寡断于人情。
