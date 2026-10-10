"""Skybox post-pass: diffraction spikes + glow on the brightest stars, then the 4K Web copy.

Run:  python art/scripts/skybox_post.py      (system Python + numpy + Pillow; after skybox.py base stage)
In:   art/textures/_tmp/cradle_sky_base.png  (8192 x 4096 equirect from skybox.py)
Out:  art/textures/skybox/cradle_sky_equirect.png (8K), cradle_sky_equirect_4k.png (box-filtered 4K)

Equirect distortion: a spike of angular length s at latitude phi spans s / cos(phi) pixels
horizontally, so every star is drawn in local angular units and stretched by 1 / cos(phi).
Stars above |phi| = 65 deg are left as they are (spikes and glows would smear across the pole).
Deterministic: stars are found by thresholding + local maxima, sorted by (brightness, y, x).
Every value is clamped to CAP < 1.0, so the bloom threshold (1.0) never fires on the sky.
"""
import math
import os

import numpy as np
from PIL import Image

ART = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
BASE = os.path.join(ART, "textures", "_tmp", "cradle_sky_base.png")
OUT8 = os.path.join(ART, "textures", "skybox", "cradle_sky_equirect.png")
OUT4 = os.path.join(ART, "textures", "skybox", "cradle_sky_equirect_4k.png")

MAX_STARS = 40          # stars that get spikes
SPIKE_ROT_DEG = 22.0    # one fixed orientation for the whole sky (like a single telescope)
CAP = 0.97              # keep the sky under the bloom threshold


def find_stars(img):
    """Point sources only: a local maximum (7 px window) that stands well above its surroundings
    (local 13 px mean), so bright nebula never counts as a star. Vectorised with Pillow filters."""
    from PIL import ImageFilter
    lum = img[..., 0] * 0.2126 + img[..., 1] * 0.7152 + img[..., 2] * 0.0722
    li = Image.fromarray(np.clip(lum * 255, 0, 255).astype(np.uint8))
    lmax = np.asarray(li.filter(ImageFilter.MaxFilter(7)), dtype=np.float32) / 255.0
    lmean = np.asarray(li.filter(ImageFilter.BoxBlur(6)), dtype=np.float32) / 255.0
    l8 = np.asarray(li, dtype=np.float32) / 255.0
    mask = (l8 >= lmax) & (l8 > 0.4) & (l8 - lmean > 0.25)
    ys, xs = np.nonzero(mask)
    cands = [(float(lum[y, x]), int(y), int(x)) for y, x in zip(ys, xs)]
    cands.sort(key=lambda c: (-c[0], c[1], c[2]))
    h = img.shape[0]
    # skip the polar caps: a glow there smears across the whole row once mapped to the sphere
    cands = [c for c in cands if abs((0.5 - (c[1] + 0.5) / h) * 180.0) < 65.0]
    picked = []
    for b, y, x in cands:
        if all((y - py) ** 2 + (x - qx) ** 2 > (40 * img.shape[1] / 4096.0) ** 2 for _, py, qx in picked):
            picked.append((b, y, x))
        if len(picked) >= MAX_STARS:
            break
    return picked


def draw(img, stars):
    h, w, _ = img.shape
    rot = math.radians(SPIKE_ROT_DEG)
    cr, sr = math.cos(rot), math.sin(rot)
    for rank, (b, y, x) in enumerate(stars):
        phi = (0.5 - (y + 0.5) / h) * math.pi
        cphi = max(math.cos(phi), 0.2)
        k = 1.0 - rank / len(stars)            # brightest first
        px = w / 4096.0                        # sizes are authored for a 4K-wide panorama
        length = (10 + 46 * k * k) * px        # spike length in equator pixels
        sigma = (2.2 + 5.0 * k) * px           # glow radius
        half_y = int(length * 2.2)
        half_x = int(half_y / cphi)
        y0, y1 = max(0, y - half_y), min(h, y + half_y + 1)
        xs = (np.arange(x - half_x, x + half_x + 1)) % w  # wrap around the seam
        ys = np.arange(y0, y1)
        X, Y = np.meshgrid((np.arange(-half_x, half_x + 1) * cphi).astype(np.float32),
                           (ys - y).astype(np.float32))
        a = X * cr + Y * sr
        bb = -X * sr + Y * cr
        col = img[y, x, :3].copy()
        col = 0.55 * col / max(col.max(), 1e-3) + 0.45  # star tint, lifted toward white
        glow = np.exp(-(a * a + bb * bb) / (2 * sigma * sigma)) * (0.35 + 0.4 * k)
        add = glow
        if abs(math.degrees(phi)) < 65:
            wdt = (0.55 + 0.35 * k) * px
            spike = (np.exp(-np.abs(bb) / wdt) * np.exp(-np.abs(a) / length) +
                     np.exp(-np.abs(a) / wdt) * np.exp(-np.abs(bb) / length)) * (0.55 + 0.45 * k)
            add = add + spike
        patch = img[np.ix_(ys, xs)]
        patch[..., :3] = 1.0 - (1.0 - patch[..., :3]) * (1.0 - np.clip(add[..., None] * col, 0, 1))  # screen
        img[np.ix_(ys, xs)] = patch
    return img


def main():
    im = np.asarray(Image.open(BASE).convert("RGB"), dtype=np.float32) / 255.0
    stars = find_stars(im)
    print("STARS", len(stars), "brightest", stars[0][0] if stars else None)
    im = draw(im, stars)
    np.clip(im, 0.0, CAP, out=im)
    rng = np.random.default_rng(1701)  # dither against banding in the smooth nebula gradients
    im8 = np.clip(im * 255.0 + rng.random(im.shape, dtype=np.float32) - 0.5, 0, 255).astype(np.uint8)
    os.makedirs(os.path.dirname(OUT8), exist_ok=True)
    Image.fromarray(im8).save(OUT8, optimize=True)
    small = im.reshape(im.shape[0] // 2, 2, im.shape[1] // 2, 2, 3).mean(axis=(1, 3))
    Image.fromarray(np.clip(small * 255.0 + 0.5, 0, 255).astype(np.uint8)).save(OUT4, optimize=True)
    print("SAVED", OUT8, os.path.getsize(OUT8), OUT4, os.path.getsize(OUT4))


if __name__ == "__main__":
    main()
