#!/usr/bin/env python3
"""Numeric screenshot comparison for the harness loop (docs/HARNESS.md).

Agents judge "did this change move the picture toward the goal?" far better from numbers than from
two images, so this prints the difference as text (the same idea as odiff, with no install needed).

  python tools/imgdiff.py BEFORE.png AFTER.png [--crop X,Y,W,H] [--out DIFF.png] [--threshold 12]
  python tools/imgdiff.py --stats IMAGE.png [--crop X,Y,W,H]

Compare mode prints:
  similarity   100% minus the share of pixels whose largest channel difference exceeds the threshold
  mean_delta   average absolute difference per channel (0..255)
  changed_bbox bounding box of the changed pixels (where to look)
and with --out writes a diff image (changed pixels in red over a dimmed copy of AFTER).

Stats mode prints brightness and colour measures of an image region, e.g. to check that an engine
plume stopped being a white blob: `white` is the share of near-white pixels, `sat` mean saturation.
"""
import argparse
import sys

import numpy as np
from PIL import Image


def load(path, crop):
    img = np.asarray(Image.open(path).convert("RGB")).astype(np.int16)
    if crop:
        x, y, w, h = crop
        img = img[y:y + h, x:x + w]
    return img


def parse_crop(s):
    if not s:
        return None
    v = [int(t) for t in s.split(",")]
    if len(v) != 4:
        raise SystemExit("--crop takes X,Y,W,H")
    return v


def stats(img):
    f = img.astype(np.float32) / 255.0
    lum = 0.2126 * f[..., 0] + 0.7152 * f[..., 1] + 0.0722 * f[..., 2]
    mx, mn = f.max(axis=2), f.min(axis=2)
    sat = np.where(mx > 1e-4, (mx - mn) / np.maximum(mx, 1e-4), 0.0)
    white = (mn > 0.85).mean()
    bright = (lum > 0.6).mean()
    mean_rgb = (f.reshape(-1, 3).mean(axis=0) * 255).round().astype(int).tolist()
    return {
        "lum_mean": round(float(lum.mean()), 4),
        "lum_p99": round(float(np.percentile(lum, 99)), 4),
        "white": round(float(white), 4),
        "bright": round(float(bright), 4),
        "sat": round(float(sat[lum > 0.15].mean()) if (lum > 0.15).any() else 0.0, 4),
        "mean_rgb": mean_rgb,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("a")
    ap.add_argument("b", nargs="?")
    ap.add_argument("--crop")
    ap.add_argument("--out")
    ap.add_argument("--threshold", type=int, default=12, help="per-channel difference that counts as a change")
    ap.add_argument("--stats", action="store_true")
    o = ap.parse_args()
    crop = parse_crop(o.crop)

    if o.stats or o.b is None:
        s = stats(load(o.a, crop))
        print(" ".join(f"{k}={v}" for k, v in s.items()))
        return

    a, b = load(o.a, crop), load(o.b, crop)
    if a.shape != b.shape:
        raise SystemExit(f"size mismatch: {a.shape[1]}x{a.shape[0]} vs {b.shape[1]}x{b.shape[0]}")
    d = np.abs(a - b)
    changed = d.max(axis=2) > o.threshold
    sim = 100.0 * (1.0 - changed.mean())
    line = f"similarity={sim:.2f}% changed={100.0 * changed.mean():.2f}% mean_delta={d.mean():.2f}"
    if changed.any():
        ys, xs = np.nonzero(changed)
        ox, oy = (crop[0], crop[1]) if crop else (0, 0)
        line += f" changed_bbox={xs.min() + ox},{ys.min() + oy},{xs.max() - xs.min() + 1},{ys.max() - ys.min() + 1}"
    sa, sb = stats(a), stats(b)
    line += f" | before white={sa['white']} lum={sa['lum_mean']} sat={sa['sat']} | after white={sb['white']} lum={sb['lum_mean']} sat={sb['sat']}"
    print(line)
    if o.out:
        vis = (b * 0.35).astype(np.uint8)
        vis[changed] = [255, 40, 40]
        Image.fromarray(vis).save(o.out)


if __name__ == "__main__":
    sys.exit(main())
