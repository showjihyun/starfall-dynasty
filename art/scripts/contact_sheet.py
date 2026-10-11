"""Tile intermediate renders (art/textures/_tmp) into labelled preview sheets. System Python + Pillow.

Run:  python art/scripts/contact_sheet.py <out.png> <cols> <label=path> [<label=path> ...]
"""
import sys

from PIL import Image, ImageDraw


def main():
    out, cols = sys.argv[1], int(sys.argv[2])
    items = [a.split("=", 1) for a in sys.argv[3:]]
    ims = [(lab, Image.open(p).convert("RGB")) for lab, p in items]
    w, h = ims[0][1].size
    rows = (len(ims) + cols - 1) // cols
    sheet = Image.new("RGB", (w * cols, h * rows), (4, 6, 12))
    d = ImageDraw.Draw(sheet)
    for i, (lab, im) in enumerate(ims):
        x, y = (i % cols) * w, (i // cols) * h
        sheet.paste(im.resize((w, h)), (x, y))
        d.rectangle([x + 8, y + 8, x + 16 + 8 * len(lab), y + 30], fill=(0, 0, 0))
        d.text((x + 12, y + 12), lab, fill=(230, 230, 230))
    sheet.save(out)
    print("SHEET", out, sheet.size)


if __name__ == "__main__":
    main()
