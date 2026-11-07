"""VulcanBench ComputerUse v1 announcement card, 16:9, sized for X.

A text card (no data: the suite is a charter, nothing is measured yet) in
the brand styling the results cards use: logo chip plus Chakra Petch
wordmark, Geist body, IBM Plex Mono labels, monochrome surface. Rendered
with Pillow directly since there is no chart.

    python scripts/rankings-chart/make_computer_use_announce.py

Output: ``vulcanbench_computer_use_v1_announce.png`` (2560x1440) next to this script.
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
OUT = HERE / "vulcanbench_computer_use_v1_announce.png"

W, H = 2560, 1440
SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#8a897f"
GRID = "#e7e6e1"
CARD = "#ffffff"

DATE = "2026-11-05"
HEADLINE = "A computer-use suite, built like the coding suites."
SUB = (
    "Design doc published " + DATE + ". Nothing is measured yet. "
    "Hermetic containers, frozen tasks, deterministic grading, "
    "run counts and stderr on every chart."
)
CARDS = [
    (
        "01  GRADE THE END STATE",
        "A checker inspects files, rows and settings after the agent stops. "
        "Never the trajectory, never an LLM judge. Every checker is probed "
        "with a scripted cheat before a task is admitted.",
    ),
    (
        "02  SCREENSHOT TAX",
        "Pass rate with pixels only, minus pass rate with pixels plus an "
        "accessibility tree. Five perception conditions per task, the way "
        "Voice v1 measures the voice tax.",
    ),
    (
        "03  SAFETY TRACK",
        "Injection compliance and unsafe-action rates, reported beside task "
        "success. Irreversible controls are always on screen. Refusing "
        "everything does not score.",
    ),
]
SPECS = [
    "3 task families",
    "12 to 15 tasks",
    "Harbor-native",
    "network off",
    "product vs. API",
    "no LLM judge",
]
FOOT = "vulcanbench.com  ·  design doc: docs/COMPUTER_USE_CHARTER.md"


def font(name: str, size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(str(HERE / name), size)


def wrap(draw: ImageDraw.ImageDraw, text: str, fnt, width: int) -> list[str]:
    lines, line = [], ""
    for word in text.split():
        cand = (line + " " + word).strip()
        if draw.textlength(cand, font=fnt) <= width:
            line = cand
        else:
            lines.append(line)
            line = word
    if line:
        lines.append(line)
    return lines


def text_block(draw, xy, text, fnt, fill, width, leading) -> int:
    x, y = xy
    for ln in wrap(draw, text, fnt, width):
        draw.text((x, y), ln, font=fnt, fill=fill)
        y += leading
    return y


def main() -> None:
    im = Image.new("RGB", (W, H), SURFACE)
    d = ImageDraw.Draw(im)
    margin = 150

    # Header: logo chip + wordmark + suite name
    logo = Image.open(HERE / "vb_logo_rounded.png").convert("RGBA")
    chip = 112
    logo = logo.resize((chip, chip), Image.LANCZOS)
    y0 = 120
    im.paste(logo, (margin, y0), logo)
    brand = font("chakra-600.ttf", 72)
    d.text((margin + chip + 34, y0 + chip / 2), "VulcanBench", font=brand, fill=INK, anchor="lm")
    wm_w = d.textlength("VulcanBench", font=brand)
    suite = font("chakra-500.ttf", 72)
    d.text(
        (margin + chip + 34 + wm_w + 40, y0 + chip / 2),
        "ComputerUse v1",
        font=suite,
        fill=MUTED,
        anchor="lm",
    )
    tag = font("ibm-plex-mono-500.ttf", 30)
    d.text(
        (W - margin, y0 + chip / 2),
        "DRAFT  ·  " + DATE,
        font=tag,
        fill=MUTED,
        anchor="rm",
    )

    # Headline + subtitle
    y = 330
    y = text_block(d, (margin, y), HEADLINE, font("chakra-500.ttf", 92), INK, W - 2 * margin, 104)
    y += 18
    y = text_block(d, (margin, y), SUB, font("geist-400.ttf", 38), INK2, W - 2 * margin - 200, 52)

    # Three cards
    top = y + 60
    gap = 40
    cw = (W - 2 * margin - 2 * gap) // 3
    ch = 520
    label_f = font("ibm-plex-mono-500.ttf", 30)
    body_f = font("geist-400.ttf", 36)
    for i, (label, body) in enumerate(CARDS):
        x = margin + i * (cw + gap)
        d.rounded_rectangle((x, top, x + cw, top + ch), radius=28, fill=CARD, outline=GRID, width=3)
        d.text((x + 44, top + 48), label, font=label_f, fill=MUTED)
        d.line((x + 44, top + 108, x + cw - 44, top + 108), fill=GRID, width=3)
        text_block(d, (x + 44, top + 140), body, body_f, INK, cw - 88, 50)

    # Spec strip
    sy = top + ch + 56
    spec_f = font("ibm-plex-mono-400.ttf", 32)
    x = margin
    for i, s in enumerate(SPECS):
        if i:
            d.text((x, sy), "·", font=spec_f, fill=MUTED)
            x += d.textlength("·", font=spec_f) + 28
        d.text((x, sy), s, font=spec_f, fill=INK2)
        x += d.textlength(s, font=spec_f) + 28

    # Footer
    d.text((margin, H - 100), FOOT, font=font("geist-400.ttf", 30), fill=MUTED, anchor="lm")

    im.save(OUT, optimize=True)
    print(OUT, im.size)


if __name__ == "__main__":
    main()
