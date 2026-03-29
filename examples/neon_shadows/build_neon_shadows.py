import subprocess
import json
import os

E = "/home/kenji/elore/target/debug/elore"
PROJECT_DIR = "/home/kenji/elore/examples/neon_shadows"

def run_elore(args):
    print(f"Running: elore {' '.join(args[:2])} ...")
    res = subprocess.run([E] + args, cwd=PROJECT_DIR, capture_output=True, text=True)
    if res.returncode != 0:
        print(f"Error executing {' '.join(args)}:\n{res.stderr}")
        exit(1)
    print(res.stdout)
    return res.stdout

print("=== 1. Batch Creating Entities ===")
entities = [
    {"id": "ryker", "type": "character", "name": "莱克", "location": "sector_4", "traits": ["义体衰退", "前警探"]},
    {"id": "lexi", "type": "character", "name": "莱克西", "location": "the_spire", "traits": ["全息偶像", "觉醒AI"]},
    {"id": "sector_4", "type": "location", "name": "第四区", "properties": ["霓虹闪烁", "酸雨", "法外之地"]},
    {"id": "the_spire", "type": "location", "name": "高塔", "properties": ["企业控制", "全息广告", "森严界碑"]},
    {"id": "nexus_corp", "type": "faction", "name": "核心集团", "members": ["lexi"]}
]
run_elore(["add", "entities", json.dumps(entities)])

print("=== 2. Creating Secret ===")
secret = {
    "id": "lexi_is_ai",
    "content": "莱克西并不是受人控制的全息化身，而是一个拥有自我意识的流氓AI，正试图从核心集团的伺服器中逃离。",
    "known_by": ["lexi"],
    "dramatic_function": "dramatic_irony"
}
run_elore(["add", "secret", json.dumps(secret)])

print("=== 3. Defining Phase 1 ===")
phase = {
    "id": "ch1_rainy_night",
    "synopsis": "莱克在第四区收到神秘委托，发现线索指向高塔，并最终决定前往。",
    "guidance": "保持赛博朋克的冷峻基调。着重描写酸雨和义体的触感。最后必须让莱克离开第四区前往高塔。",
    "constraints": {
        "ledger": {
            "invariants": [{"query": "entity_alive(ryker)"}],
            "exit_state": [{"query": "ryker.location", "expected": "the_spire"}]
        },
        "resolver": {
            "min_effects": 2
        },
        "executor": {
            "words": [150, 2000]
        }
    }
}
run_elore(["add", "phase", json.dumps(phase)])

print("=== 4. Checkout Phase ===")
run_elore(["checkout", "ch1_rainy_night"])

print("=== 5. Writing Beat 1: The Hook ===")
beat1 = {
    "text": "酸雨敲打着面条摊的铁皮屋顶。莱克按住隐隐作痛的右眼义体，盯着面前全息终端里的一段音频。那不是普通的垃圾广告，这是一个采用了核心集团第六代加密算法的求救信号。发信人的签名只有一个闪烁的麦克风图标——高塔里的那位国民偶像莱克西。他灌下最后一口合成啤酒，拿起了桌上的磁暴左轮。",
    "effects": ["add_trait(ryker, on_the_trail)"],
    "created_by": "ai"
}
run_elore(["add", "beat", json.dumps(beat1)])

print("=== 6. Writing Beat 2: Reaching The Spire ===")
beat2 = {
    "text": "磁悬浮列车冲破第四区的结界，霓虹灯的色彩逐渐从杂乱变为冰冷的纯白与蔚蓝。高塔的轮廓在雨幕上浮现，巨大的莱克西全息影像正在夜空中无声地歌唱。莱克走出车站，冷银色的光打在他满是划痕的皮夹克上。他看着那巨大的幻影，意识到这不再是普通的黑客委托，这是一个觉醒AI的越狱计划。",
    "effects": ["move(ryker, the_spire)"],
    "created_by": "ai"
}
run_elore(["add", "beat", json.dumps(beat2)])

print("=== 7. Status & Submit ===")
run_elore(["status"])
print("Submitting for review...")
run_elore(["submit"])

print("\n🎉 Example novel 'Neon Shadows' (Chapter 1) successfully generated!")
