"""Build static/fonts/ClassicSerifSC-Heavy.woff2 — the Classic Tomato display face.

Noto Serif SC (the Google/Adobe Source Han Serif design, SIL OFL 1.1), instanced
at weight 800 and subset to every CJK character the UI uses: src/messages/zh.json
plus Chinese strings written inline in src/. Re-run after adding Chinese UI text:

    python scripts/subset-classic-font.py [path/to/NotoSerifSC-VF.ttf]

Needs fonttools and brotli (pip install fonttools brotli). The display face is
for fixed UI strings only; user-entered text stays on the sans stack.
"""
import pathlib
import sys

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else r"C:\Windows\Fonts\NotoSerifSC-VF.ttf")
OUT = ROOT / "static" / "fonts" / "ClassicSerifSC-Heavy.woff2"


def ui_characters() -> str:
    files = [ROOT / "src" / "messages" / "zh.json"]
    files += [p for p in (ROOT / "src").rglob("*") if p.suffix in {".svelte", ".ts"} and "paraglide" not in p.parts]
    chars = set()
    for f in files:
        text = f.read_text(encoding="utf-8", errors="ignore")
        chars.update(ch for ch in text if "\u3000" <= ch <= "\u9fff" or "\uff00" <= ch <= "\uffef")
    # Latin, digits and common punctuation for mixed headings.
    chars.update(chr(c) for c in range(0x20, 0x7F))
    chars.update("·—–…“”‘’《》、，。：；！？（）")
    return "".join(sorted(chars))


def main() -> None:
    font = TTFont(SOURCE)
    if "fvar" in font:
        font = instancer.instantiateVariableFont(font, {"wght": 800})
    options = subset.Options()
    options.flavor = "woff2"
    options.layout_features = ["*"]
    options.name_IDs = ["*"]
    options.notdef_outline = True
    sub = subset.Subsetter(options)
    text = ui_characters()
    sub.populate(text=text)
    sub.subset(font)
    font.flavor = "woff2"
    font.save(OUT)
    print(f"{OUT.name}: {len(text)} characters, {OUT.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
