"""Collects the style picker page into tools/blender/_build/styles/pub: JPEGs of the renders, the models as
base64 text (the page host serves no .glb), the style table and SRI hashes. Run after build_styles.py."""
import json
import os
import shutil
import struct
import sys

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "..", "_build", "styles")
PUB = os.path.join(SRC, "pub")

STYLES = [
    dict(key="baseline", tab="현재", code="STYLE 00 · BASELINE", name="현재 Talon 주력함",
         desc="지금 게임에 들어 있는 420 m 주력함입니다. 같은 조명에서 나머지 넷과 비교하는 기준입니다.",
         refs="현행 빌드", accent="#9aa7b8", engine="#ff9e52", colors=["#3a3d42", "#8c1a10", "#59ccff"],
         pro="이미 포탑·리깅·등급 파츠·종족 키트가 붙어 있음", con="전투기를 늘린 형태라 거함의 규모감이 약함",
         form="", surface="", combat=""),
    dict(key="west", tab="서양식", code="STYLE 01 · WEST", name="서양식 하드서피스",
         desc="해군 전함의 논리로 지은 장갑 순양전함. 각진 성채와 장갑 벨트, 포갑판, 드러난 구조물. 사실적이고 무게감이 있습니다.",
         refs="Battlestar Galactica · Halo UNSC · The Expanse · Star Citizen Idris · Homeworld",
         accent="#f0a23c", engine="#ffb060", colors=[], assign="Kharz 주력함",
         pro="현재 아트 방향과 가장 가깝고 대규모 함대전의 무게감이 삼", con="가장 익숙한 방향이라 차별성은 낮음",
         form="각진 성채, 장갑 벨트, 포갑판, 노출된 구조와 방열판", surface="건메탈·올리브 도색, 진영색 띠, 경고 표식, 마모와 그을음",
         combat="현측 포격과 장갑 윤곽. 가까이서 가장 묵직함"),
    dict(key="japan", tab="일본식", code="STYLE 02 · JAPAN", name="일본식 기함",
         desc="애니메이션의 주인공 함. 함수의 주포구나 충각, 겹쳐 올린 포탑과 높은 함교, 흰 장갑에 원색 블록.",
         refs="우주전함 야마토 · Macross · Gundam 화이트베이스 · 은하영웅전설 · 아르카디아",
         accent="#4f8cff", engine="#7fd0ff", colors=[], assign="Veyr 주력함",
         pro="함 하나하나의 캐릭터성과 수집 욕구가 가장 강함", con="현재 세계관의 거친 톤과 거리가 있어 UI·스테이션도 손봐야 함",
         form="함수 주포구, 겹친 3연장 포탑, 탑형 함교, 사출 갑판", surface="흰 세라믹 장갑, 원색 블록, 어두운 내부 프레임, 오염 거의 없음",
         combat="흰 선체와 원색이 어두운 우주에서 가장 또렷함"),
    dict(key="global", tab="글로벌", code="STYLE 03 · GLOBAL", name="글로벌 스타일라이즈드",
         desc="썸네일 크기에서도 읽히는 크고 단순한 실루엣. 매끈한 곡면, 자신 있는 투톤 도색, 넉넉한 엔진 발광.",
         refs="Mass Effect · Star Trek · Star Wars 몬 칼라마리 · Destiny · No Man's Sky",
         accent="#35d6a8", engine="#8fe9ff", colors=[], assign="Human 주력함",
         pro="지역을 가리지 않고 호감도가 높고 스토어 썸네일에서 잘 읽힘", con="디테일이 차분해 클로즈업의 밀도는 서양식보다 낮음",
         form="크고 단순한 실루엣, 흐르는 곡면, 창문 띠", surface="밝은 본체에 넓은 진영색, 클리어코트 광택, 가벼운 마모",
         combat="실루엣과 엔진 링. 멀리서 가장 잘 구분됨"),
    dict(key="unknown", tab="미지의 우주", code="STYLE 04 · UNKNOWN", name="미지의 우주",
         desc="인간의 논리가 아닌 거함. 빛나는 핵을 감싼 거대한 흑요석 칼날, 떠 있는 듯 이어진 분절, 새겨진 회로와 발광 맥.",
         refs="Homeworld Progenitor · Halo Forerunner · Protoss 모선 · Reaper · Arrival",
         accent="#b48cff", engine="#c7a0ff", colors=[], assign="아스테르 리바이어던 (유적 수호자)",
         pro="가장 독창적이고 선구자(Precursor) 설정과 맞음. 탐험의 보상으로 강력", con="가까이 당기면 문양 텍스처가 무를 수 있음",
         form="초승달·고리·파편 기하, 함교와 창이 없음", surface="어두운 금속에 새긴 문양, 발광 맥, 상감된 진영색",
         combat="발광 핵과 맥이 어둠 속에서 떠오름"),
]


def tris(path):
    with open(path, "rb") as f:
        f.seek(12)
        n, _ = struct.unpack("<II", f.read(8))
        j = json.loads(f.read(n))
    per = [sum(j["accessors"][p["indices"]]["count"] // 3 for p in m["primitives"]) for m in j["meshes"]]
    return sum(per[n["mesh"]] for n in j["nodes"] if "mesh" in n)  # instanced turrets count each time


def to_hex(c):
    """Linear RGB tuple -> sRGB hex."""
    def s(x):
        x = max(0.0, min(1.0, x))
        return round(255 * (12.92 * x if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055))
    return "#%02x%02x%02x" % tuple(s(x) for x in c)


def main():
    os.makedirs(PUB, exist_ok=True)
    files = {}
    out = []
    for s in STYLES:
        k = s["key"]
        glb = os.path.join(SRC, k + ".glb")
        if not os.path.exists(glb) or not os.path.exists(os.path.join(SRC, k + "_battle.png")):
            print("skip", k, "(no glb)")
            continue
        import base64
        with open(os.path.join(PUB, k + ".glb.txt"), "wb") as f:  # artifacts do not serve .glb
            f.write(base64.b64encode(open(glb, "rb").read()))
        for kind, w in (("hero", 1600), ("battle", 1920)):
            im = Image.open(os.path.join(SRC, "%s_%s.png" % (k, kind))).convert("RGB")
            if im.width > w:
                im = im.resize((w, round(im.height * w / im.width)), Image.LANCZOS)
            im.save(os.path.join(PUB, "%s_%s.jpg" % (k, kind)), quality=88)
        s = dict(s, tris=tris(glb))
        if k != "baseline":  # the style's own colours, from its module (text scan: the modules import bpy)
            import ast
            import re
            src = open(os.path.join(HERE, "..", "styles", k + ".py"), encoding="utf-8").read()
            def pick(name):
                m = re.search(r"['\"]?%s['\"]?\s*[:=]\s*(\([^)]*\))" % name, src)
                return ast.literal_eval(m.group(1)) if m else None
            cols = [c for c in (pick("paint"), pick("livery"), pick("glow"), pick("engine")) if c]
            s["colors"] = [to_hex(c) for c in cols]
            if pick("engine"):
                s["engine"] = to_hex(tuple(min(1.0, x * 1.0) for x in pick("engine")))
        out.append(s)
        print(k, s["tris"], "tris", "%.1f MB" % (os.path.getsize(glb) / 1e6), s["colors"])
    html = open(os.path.join(HERE, "index.html"), encoding="utf-8").read()
    a, b = html.index("/*STYLES*/"), html.index("/*END*/")
    # Inside a <script>: nothing in the data may close the tag.
    payload = json.dumps(out, ensure_ascii=False).replace("<", "\\u003c").replace(">", "\\u003e").replace("&", "\\u0026")
    html = html[:a] + payload + html[b + len("/*END*/"):]
    # Every remote script carries its hash in the page source; a new one without it fails here.
    import re
    for tag in re.findall(r'<(?:script|link)[^>]*(?:src|href)="https?://[^"]+"[^>]*>', html):
        assert "fonts.googleapis.com" in tag or 'integrity="sha384-' in tag, "remote resource without SRI: " + tag
    open(os.path.join(PUB, "index.html"), "w", encoding="utf-8").write(html)
    print("pub:", sorted(os.listdir(PUB)))


if __name__ == "__main__":
    main()
