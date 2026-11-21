#!/usr/bin/env python3
"""Draw Vigía's icon and write every size the app and the bundles need.

Usage: python3 script/make-icons.py   (needs Pillow; macOS also runs iconutil for the .icns)
Writes crates/app/assets/logo/{vigia-icon.svg, vigia-icon-1024.png, vigia-icon-512.png,
AppIcon.icns, AppIcon.ico}, crates/app/assets/icons/vigia-logo.svg (the in-app mark) and
crates/app/assets/icons/vigia-status.png (the menu-bar template image).
The motif: a lighthouse keeping watch over the night, its beam sweeping right, next to a
calendar page with one day lit in amber.
"""
import pathlib, shutil, subprocess, tempfile
from PIL import Image, ImageChops, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/app/assets/logo"
ICONS = ROOT / "crates/app/assets/icons"
TOP, BOTTOM = (0x2E, 0x4F, 0x94), (0x14, 0x1E, 0x44)
CREAM, AMBER, NAVY = (0xF6, 0xF3, 0xEC), (0xF2, 0xA9, 0x3B), (0x1E, 0x2F, 0x5C)
LIGHT = (0xFF, 0xC8, 0x5A)

LAMP = (400, 392)
BEAM_R = [LAMP, (924, 215), (924, 500)]
BEAM_L = [LAMP, (100, 300), (100, 450)]
# tower: bottom 300..500 at y 820, top 338..462 at y 478
TOWER = [(300, 820), (500, 820), (462, 478), (338, 478)]
STRIPES = [(575, 625), (700, 750)]
GALLERY = (318, 452, 482, 482)
LAMP_ROOM = (352, 362, 448, 452)
ROOF = [(334, 366), (466, 366), (400, 300)]
BALL = (400, 292, 15)
BASE = (250, 806, 550, 850)
CAL = (562, 566, 862, 866)
CAL_HEAD = 658
RINGS = [640, 784]
DOTS = [(633, 730), (712, 730), (791, 730), (633, 806), (712, 806), (791, 806)]
LIT = 5


def tower_x(y):
    t = (y - 820) / (478 - 820)
    return 300 + (338 - 300) * t, 500 + (462 - 500) * t


def hexc(c):
    return f"#{c[0]:02X}{c[1]:02X}{c[2]:02X}"


def poly(pts):
    return " ".join(f"{x:.0f},{y:.0f}" for x, y in pts)


def svg(mark=False):
    p = []
    fill = (lambda c: "#000") if mark else hexc
    if not mark:
        p.append('<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">'
                 f'<stop offset="0" stop-color="{hexc(TOP)}"/><stop offset="1" stop-color="{hexc(BOTTOM)}"/></linearGradient>'
                 '<clipPath id="r"><rect x="100" y="100" width="824" height="824" rx="185"/></clipPath></defs>')
        p.append('<rect x="100" y="100" width="824" height="824" rx="185" fill="url(#bg)"/>')
        p.append(f'<g clip-path="url(#r)"><polygon points="{poly(BEAM_R)}" fill="{hexc(LIGHT)}" fill-opacity="0.55"/>'
                 f'<polygon points="{poly(BEAM_L)}" fill="{hexc(LIGHT)}" fill-opacity="0.25"/></g>')
    else:
        p.append(f'<polygon points="{poly([LAMP, (900, 250), (900, 470)])}"/>')
    p.append(f'<polygon points="{poly(TOWER)}" fill="{fill(CREAM)}"/>')
    if not mark:
        for a, b in STRIPES:
            (a0, a1), (b0, b1) = tower_x(a), tower_x(b)
            p.append(f'<polygon points="{poly([(a0, a), (a1, a), (b1, b), (b0, b)])}" fill="{hexc(NAVY)}"/>')
    x0, y0, x1, y1 = GALLERY
    p.append(f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" rx="6" fill="{fill(CREAM)}"/>')
    x0, y0, x1, y1 = LAMP_ROOM
    p.append(f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" fill="{fill(AMBER)}"/>')
    p.append(f'<polygon points="{poly(ROOF)}" fill="{fill(CREAM)}"/>')
    p.append(f'<circle cx="{BALL[0]}" cy="{BALL[1]}" r="{BALL[2]}" fill="{fill(CREAM)}"/>')
    x0, y0, x1, y1 = BASE
    p.append(f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" rx="20" fill="{fill(CREAM)}"/>')
    x0, y0, x1, y1 = CAL
    if mark:
        p.append(f'<rect x="{x0 + 18}" y="{y0 + 18}" width="{x1 - x0 - 36}" height="{y1 - y0 - 36}" rx="34" fill="none" stroke="#000" stroke-width="36"/>')
        p.append(f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{CAL_HEAD - y0}" rx="40"/>')
        for i in (2, 5):
            x, y = DOTS[i]
            p.append(f'<circle cx="{x - 20}" cy="{y}" r="24"/>')
    else:
        p.append(f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" rx="46" fill="{hexc(CREAM)}"/>')
        p.append(f'<path d="M{x0} {CAL_HEAD} V{y0 + 46} Q{x0} {y0} {x0 + 46} {y0} H{x1 - 46} Q{x1} {y0} {x1} {y0 + 46} V{CAL_HEAD} Z" fill="{hexc(AMBER)}"/>')
        for x in RINGS:
            p.append(f'<rect x="{x - 14}" y="{y0 - 30}" width="28" height="72" rx="14" fill="{hexc(NAVY)}"/>')
        for i, (x, y) in enumerate(DOTS):
            if i == LIT:
                p.append(f'<circle cx="{x}" cy="{y}" r="30" fill="{hexc(AMBER)}"/>')
            else:
                p.append(f'<circle cx="{x}" cy="{y}" r="19" fill="{hexc(NAVY)}" fill-opacity="0.75"/>')
    if mark:
        return '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="230 250 690 640">' + "".join(p) + "</svg>\n"
    return '<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">' + "".join(p) + "</svg>\n"


def draw(size=1024, ss=4):
    S = size * ss
    k = S / 1024
    sc = lambda pts: [(x * k, y * k) for x, y in pts]
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    grad = Image.new("RGBA", (S, S))
    gd = ImageDraw.Draw(grad)
    for y in range(S):
        t = min(max((y / k - 100) / 824, 0), 1)
        gd.line([(0, y), (S, y)], fill=tuple(int(TOP[i] + (BOTTOM[i] - TOP[i]) * t) for i in range(3)) + (255,))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([100 * k, 100 * k, 924 * k, 924 * k], radius=185 * k, fill=255)
    img.paste(grad, (0, 0), mask)
    beams = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    bd = ImageDraw.Draw(beams)
    bd.polygon(sc(BEAM_R), fill=LIGHT + (140,))
    bd.polygon(sc(BEAM_L), fill=LIGHT + (64,))
    beams.putalpha(ImageChops.multiply(beams.getchannel("A"), mask))
    img = Image.alpha_composite(img, beams)
    d = ImageDraw.Draw(img)
    d.polygon(sc(TOWER), fill=CREAM + (255,))
    for a, b in STRIPES:
        (a0, a1), (b0, b1) = tower_x(a), tower_x(b)
        d.polygon(sc([(a0, a), (a1, a), (b1, b), (b0, b)]), fill=NAVY + (255,))
    x0, y0, x1, y1 = GALLERY
    d.rounded_rectangle([x0 * k, y0 * k, x1 * k, y1 * k], radius=6 * k, fill=CREAM + (255,))
    x0, y0, x1, y1 = LAMP_ROOM
    d.rectangle([x0 * k, y0 * k, x1 * k, y1 * k], fill=AMBER + (255,))
    d.polygon(sc(ROOF), fill=CREAM + (255,))
    bx, by, br = BALL
    d.ellipse([(bx - br) * k, (by - br) * k, (bx + br) * k, (by + br) * k], fill=CREAM + (255,))
    x0, y0, x1, y1 = BASE
    d.rounded_rectangle([x0 * k, y0 * k, x1 * k, y1 * k], radius=20 * k, fill=CREAM + (255,))
    x0, y0, x1, y1 = CAL
    d.rounded_rectangle([x0 * k, y0 * k, x1 * k, y1 * k], radius=46 * k, fill=CREAM + (255,))
    d.rounded_rectangle([x0 * k, y0 * k, x1 * k, (CAL_HEAD + 46) * k], radius=46 * k, fill=AMBER + (255,))
    d.rectangle([x0 * k, CAL_HEAD * k, x1 * k, (CAL_HEAD + 50) * k], fill=CREAM + (255,))
    for x in RINGS:
        d.rounded_rectangle([(x - 14) * k, (y0 - 30) * k, (x + 14) * k, (y0 + 42) * k], radius=14 * k, fill=NAVY + (255,))
    dots = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    dd = ImageDraw.Draw(dots)
    for i, (x, y) in enumerate(DOTS):
        r, c = (30, AMBER + (255,)) if i == LIT else (19, NAVY + (191,))
        dd.ellipse([(x - r) * k, (y - r) * k, (x + r) * k, (y + r) * k], fill=c)
    img = Image.alpha_composite(img, dots)
    return img.resize((size, size), Image.LANCZOS)


def status_png(px=36, ss=8):
    """Menu-bar template: black on transparent, tower and beam only (reads at 18 pt)."""
    S = px * ss
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # map the 1024 artwork's tower region (x 240..900, y 270..860) into the square
    ox, oy, span = 230, 250, 650
    f = lambda x, y: ((x - ox) / span * S, (y - oy) / span * S)
    black = (0, 0, 0, 255)
    d.polygon([f(*p) for p in [LAMP, (900, 250), (900, 480)]], fill=(0, 0, 0, 170))
    d.polygon([f(*p) for p in TOWER], fill=black)
    for a, b in STRIPES:
        (a0, a1), (b0, b1) = tower_x(a), tower_x(b)
        d.polygon([f(a0, a), f(a1, a), f(b1, b), f(b0, b)], fill=(0, 0, 0, 0))
    x0, y0, x1, y1 = GALLERY
    d.rectangle([*f(x0, y0), *f(x1, y1)], fill=black)
    x0, y0, x1, y1 = LAMP_ROOM
    d.rectangle([*f(x0, y0), *f(x1, y1)], fill=black)
    d.polygon([f(*p) for p in ROOF], fill=black)
    x0, y0, x1, y1 = BASE
    d.rounded_rectangle([*f(x0, y0), *f(x1, y1)], radius=20 / span * S, fill=black)
    x0, y0, x1, y1 = CAL
    w = 40 / span * S
    d.rounded_rectangle([*f(x0, y0), *f(x1, y1)], radius=50 / span * S, outline=black, width=int(w))
    d.rectangle([*f(x0, y0 + 10), *f(x1, CAL_HEAD)], fill=black)
    return img.resize((px, px), Image.LANCZOS)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    ICONS.mkdir(parents=True, exist_ok=True)
    (OUT / "vigia-icon.svg").write_text(svg())
    (ICONS / "vigia-logo.svg").write_text(svg(mark=True))
    status_png().save(ICONS / "vigia-status.png")
    big = draw(1024)
    big.save(OUT / "vigia-icon-1024.png")
    big.resize((512, 512), Image.LANCZOS).save(OUT / "vigia-icon-512.png")
    big.save(OUT / "AppIcon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    if shutil.which("iconutil"):
        with tempfile.TemporaryDirectory() as t:
            iconset = pathlib.Path(t) / "AppIcon.iconset"
            iconset.mkdir()
            for s in [16, 32, 128, 256, 512]:
                big.resize((s, s), Image.LANCZOS).save(iconset / f"icon_{s}x{s}.png")
                big.resize((s * 2, s * 2), Image.LANCZOS).save(iconset / f"icon_{s}x{s}@2x.png")
            subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(OUT / "AppIcon.icns")], check=True)
    print("icons written to", OUT)


if __name__ == "__main__":
    main()
