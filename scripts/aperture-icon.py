"""Generates the Fluss mark: the aperture as a round, full-bleed icon (transparent corners).
Same geometry as the voice visual (docs/DESIGN.md, "aperture").
Writes ui/src-tauri/icons/icon.svg and site/favicon.svg; prints the 24px variant for Logo.svelte."""
import math
from pathlib import Path

GOLD, DIM, PLATE = "#E3B04B", "#8C6B2A", "#0B0A08"


def mark(size, open_=0.36, rot=0.3, rim=None, ray=None, hexw=None):
    c, R = size / 2, size / 2 - (rim or size * 0.035) / 2
    rim = rim or size * 0.035
    ray = ray or size * 0.022
    hexw = hexw or size * 0.03
    B, half = 6, math.pi / 6
    twist = rot + open_ * 1.3
    vr = R * open_ / math.cos(half)
    hexp = [(c + math.cos(twist + i * 2 * math.pi / B + half) * vr, c + math.sin(twist + i * 2 * math.pi / B + half) * vr) for i in range(B)]
    f = lambda v: f"{v:.2f}"
    rays = []
    for i in range(B):
        th = twist + i * 2 * math.pi / B
        vx, vy = hexp[(i - 1) % B]
        dx, dy = -math.sin(th), math.cos(th)
        ox, oy = vx - c, vy - c
        b = ox * dx + oy * dy
        u = -b + math.sqrt(max(0, b * b - (ox * ox + oy * oy - (R - rim / 2) ** 2)))
        rays.append(f"M{f(vx)} {f(vy)}L{f(vx + dx * u)} {f(vy + dy * u)}")
    hexd = "M" + " ".join(f"{f(x)} {f(y)}" for x, y in hexp) + "Z"
    d = R * open_ * 0.5
    dia = f"M{f(c)} {f(c - d)} {f(c + d)} {f(c)} {f(c)} {f(c + d)} {f(c - d)} {f(c)}Z"
    return (
        f'<circle cx="{f(c)}" cy="{f(c)}" r="{f(R)}" fill="{PLATE}" stroke="{GOLD}" stroke-width="{f(rim)}"/>'
        f'<circle cx="{f(c)}" cy="{f(c)}" r="{f(R - rim / 2)}" fill="{GOLD}" fill-opacity="0.13"/>'
        f'<path d="{"".join(rays)}" stroke="{DIM}" stroke-width="{f(ray)}" stroke-linecap="round"/>'
        f'<path d="{hexd}" fill="{PLATE}" stroke="{GOLD}" stroke-width="{f(hexw)}" stroke-linejoin="round"/>'
        f'<path d="{dia}" fill="{GOLD}"/>'
    )


root = Path(__file__).resolve().parent.parent
(root / "ui/src-tauri/icons/icon.svg").write_text(
    f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 64 64">{mark(64, rim=2.6, ray=1.5, hexw=2.2)}</svg>\n')
(root / "site/favicon.svg").write_text(
    f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">{mark(32, rim=1.8, ray=1.0, hexw=1.4)}</svg>\n')
print(mark(24, rim=1.3, ray=0.8, hexw=1.1))
