---
title: "赴宴：谢罪鸿门"
order: 2
synopsis: "刘邦率百余骑至鸿门，当面向项羽谢罪。项羽怒气稍解，设宴款待。"
effects:
  - move(liu_bang, hongmen)
  - move(zhang_liang, hongmen)
  - move(fan_kuai, hongmen)
  - remove_trait(liu_bang, 恐惧)
  - add_trait(liu_bang, 谦卑)
constraints:
  exit_state:
    - query: liu_bang.location
      expected: hongmen
---

翌日清晨，沛公从百余骑来见项王，至鸿门，谢曰："臣与将军戮力而攻秦，将军战河北，臣战河南，然不自意能先入关破秦，得复见将军于此。今者有小人之言，令将军与臣有郤。"

项王曰："此沛公左司马曹无伤言之。不然，籍何以至此？"

项王即日因留沛公与饮。项王、项伯东向坐，亚父南向坐——亚父者，范增也。沛公北向坐，张良西向侍。
