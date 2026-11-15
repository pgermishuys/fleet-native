#!/usr/bin/env python3
"""Makes the app icons from assets/weave_logo.png: assets/icon/{icon.png,fleet-native.ico,fleet-native.icns}.
Run: python3 tools/gen-app-icon.py   (needs Pillow). The outputs are committed; CI does not run this."""
from PIL import Image, ImageDraw
import os

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(here)
out = os.path.join(root, "assets", "icon")
os.makedirs(out, exist_ok=True)

S = 1024
logo = Image.open(os.path.join(root, "assets", "weave_logo.png")).convert("RGBA")
# macOS icons keep ~10% margin around the rounded square; the mark sits in its middle.
tile = Image.new("RGBA", (S, S), (0, 0, 0, 0))
m = 100
d = ImageDraw.Draw(tile)
d.rounded_rectangle((m, m, S - m, S - m), radius=185, fill=(18, 20, 28, 255))
w = 560
h = round(logo.height * w / logo.width)
mark = logo.resize((w, h), Image.LANCZOS)
tile.alpha_composite(mark, ((S - w) // 2, (S - h) // 2))
tile.resize((512, 512), Image.LANCZOS).save(os.path.join(out, "icon.png"))

# Windows and Linux show the tile without the macOS margin, so crop it for the .ico.
crop = tile.crop((m - 20, m - 20, S - m + 20, S - m + 20))
crop.save(os.path.join(out, "fleet-native.ico"), sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
tile.save(os.path.join(out, "fleet-native.icns"), sizes=[(s, s) for s in (16, 32, 64, 128, 256, 512, 1024)])
