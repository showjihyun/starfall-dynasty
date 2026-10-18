#!/usr/bin/env python3
"""Art-direction guard (docs/ART_DIRECTION.md): render the reference scenes and compare them with the
approved images in docs/art/reference/. A change that moves the look fails until the new image is
reviewed against the art direction and approved with --update.

  python tools/artcheck.py                 # render, compare, print one line per scene, exit 1 on drift
  python tools/artcheck.py --update        # approve: overwrite the references with fresh renders
  python tools/artcheck.py --only fleet    # one scene

Scenes are deterministic showroom renders (STARFALL_FIXED_FPS=60, UI hidden, shot at 8 s). Engine
plume flicker still varies by ~0.2% of pixels between identical runs, hence the 99.0% bar.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile

from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REF = os.path.join(ROOT, 'docs', 'art', 'reference')
CLIENT = os.path.join(ROOT, 'target', 'release', 'client.exe' if os.name == 'nt' else 'client')
MIN_SIMILARITY = 99.0

# name -> (client args, what it pins down)
SCENES = {
    'fleet': (['--showroom', '7'], 'every hull and livery, weapon volleys, shield hit, explosion — War Zone blue giant'),
    'core': (['--showroom', '0'], 'Sol Haven: yellow dwarf, safe-space palette, station'),
    'gate': (['--showroom', '9', '--look', 'gate'], 'The Deep: neutron star, ancient gate glyphs'),
    'ruin': (['--showroom', '6', '--look', 'ruin'], 'Hollow Crown: ruin glow, Wild-tier nebula'),
    'derelict': (['--showroom', '4', '--look', 'derelict'], 'Vesta Corridor: white dwarf, derelict convoy'),
    # Species styles (docs/ART_DIRECTION.md §4): the same fleet and camera, one species each.
    # Shot at 20 s, when the orbit frames a small hull (species kits: masts / ribs / horns).
    'human': (['--showroom', '7', '--hull', 'talon', '--look', 'bow', '--species', 'human', '--shot-at', '20'], 'Human: worn-industrial hulls, masts, radiators, hatch'),
    'veyr': (['--showroom', '7', '--hull', 'talon', '--look', 'bow', '--species', 'veyr', '--shot-at', '20'], 'Veyr: dark-teal nacre, violet veins, hoop ribs'),
    'kharz': (['--showroom', '7', '--hull', 'talon', '--look', 'bow', '--species', 'kharz', '--shot-at', '20'], 'Kharz: blackened iron, amber heat seams, bow horns, patch armour'),
    'capital': (['--showroom', '7', '--hull', 'bastion', '--class', 'capital', '--look', 'ship', '--species', 'veyr'], 'Bastion capital, Veyr: section-hugging hoop ribs on a boxy hull'),
}


def render(args, out):
    env = dict(os.environ, STARFALL_FIXED_FPS='60', STARFALL_HIDE_UI='1')
    at = [] if '--shot-at' in args else ['--shot-at', '8']
    # A screenshot taken before the window is ready can come back as a 1×1 image: never accept one.
    for _ in range(2):
        if os.path.exists(out):
            os.remove(out)
        subprocess.run([CLIENT, *args, '--shot', out, *at], env=env, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=180)
        if os.path.exists(out) and Image.open(out).size[0] >= 640:
            return True
    return False


def similarity(a, b):
    r = subprocess.run([sys.executable, os.path.join(ROOT, 'tools', 'imgdiff.py'), a, b], capture_output=True, text=True)
    m = re.search(r'similarity=([\d.]+)%', r.stdout)
    return (float(m.group(1)) if m else 0.0), r.stdout.strip()


def main():
    sys.stdout.reconfigure(encoding='utf-8')
    p = argparse.ArgumentParser()
    p.add_argument('--update', action='store_true')
    p.add_argument('--only')
    a = p.parse_args()
    if not os.path.exists(CLIENT):
        sys.exit(f'build the client first: cargo build --release -p client ({CLIENT})')
    os.makedirs(REF, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix='artcheck-')
    failed = 0
    for name, (args, what) in SCENES.items():
        if a.only and name != a.only:
            continue
        out = os.path.join(tmp, name + '.png')
        if not render(args, out):
            print(f'FAIL {name:9} no image rendered ({" ".join(args)})')
            failed += 1
            continue
        ref = os.path.join(REF, name + '.png')
        if a.update or not os.path.exists(ref):
            shutil.copy(out, ref)
            print(f'SET  {name:9} {ref}  — {what}')
            continue
        sim, line = similarity(ref, out)
        ok = sim >= MIN_SIMILARITY
        failed += not ok
        print(f'{"OK  " if ok else "DRIFT"} {name:9} {line}')
        if not ok:
            keep = os.path.join(tmp, name + '_new.png')
            shutil.copy(out, keep)
            print(f'      new render: {keep}  — review against docs/ART_DIRECTION.md, then --update to approve')
    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()
