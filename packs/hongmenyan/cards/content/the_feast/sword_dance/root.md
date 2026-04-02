---
title: "项庄舞剑"
order: 2
synopsis: "项庄舞剑意在沛公，项伯以身翼蔽刘邦。"
effects:
  - reveal(sword_dance, liu_bang)
  - add_trait(liu_bang, 恐惧)
constraints:
  exit_state:
    - query: knows(liu_bang, sword_dance)
      expected: "true"
---

寿毕，曰："君王与沛公饮，军中无以为乐，请以剑舞。"项王曰："诺。"

项庄拔剑起舞。项伯亦拔剑起舞，常以身翼蔽沛公，庄不得击。
