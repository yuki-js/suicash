# -*- coding: utf-8 -*-
"""SuiCash logo generator.

  python3 assets/logo/generate.py

Writes two files.

  suicash-logo.svg   wordmark
  suicash-icon.svg   app / favicon icon (iC only)

No glyph paths are written. The Google Fonts variable font REM is referenced from <text>
at the weight that best overlaps the reference logo's letterforms. Only the spacing is
fixed: each letter is placed at absolute coordinates following the measured ratios. See the
README for where the numbers come from.

FONT_SOURCE selects how the web font is referenced.

  "googlefonts" … loaded from Google Fonts via @import (default)
  "selfhost"    … subset WOFF2 written to font/, loaded by relative path

Either way, the font is resolved locally to compute advances.
"""
import os
import tempfile
import urllib.request

from fontTools.ttLib import TTFont
from fontTools.pens.boundsPen import BoundsPen
from fontTools.varLib import instancer
from fontTools.subset import Subsetter, Options

HERE = os.path.dirname(os.path.abspath(__file__))
FONT_DIR = os.path.join(HERE, "font")
SRC_WOFF2 = os.path.join(tempfile.gettempdir(), "REM-latin-variable.woff2")
OUT_WOFF2 = os.path.join(FONT_DIR, "suicash-wordmark.woff2")

FONT_SOURCE = "googlefonts"            # "googlefonts" / "selfhost"
GF_FAMILY = "REM"
GF_CSS = ("https://fonts.googleapis.com/css2?family=REM:"
          "wght@100..900&display=block")
FAMILY = "SuiCash Wordmark"            # family name when selfhosting

# Measured per pixel from a 4000px rasterization. The reference logo's paths are not used.
MEASURED = {
    "x_height": 0.711,      # x-height
    "stem":     0.224,      # lowercase stem thickness
    "outline":  0.062,      # outline thickness of the reversed letters
    "gap_flat": 0.444,      # ink gap between two adjacent straight sides
    "gap_step": 0.039,      # tightening per curved side
}

# Weight that best overlaps the reference logo's S / u / a silhouettes (searched across 73 typefaces).
AXES = {"wght": 605.0}

# Whether each letter's sides are curved (left, right). Used for optical spacing.
ROUND_SIDES = {"S": (1, 1), "u": (0, 0), "i": (0, 0),
               "C": (1, 1), "a": (1, 0), "s": (1, 1), "h": (0, 0)}

INK = "#0A1A2F"                        # navy
BLUE = "#4DA2FF"                       # Sui blue
WHITE = "#FFFFFF"

WORD = "SuiCash"
REVERSED = (2, 4)                      # "iC" (WORD[2:4]) is reversed out
CAP_PX = 100.0                         # set with a cap height of 100px


# --------------------------------------------------------------------------
# 1. Fetch the font → instantiate → subset
# --------------------------------------------------------------------------
def download_source():
    """Download and cache the latin-subset variable font from Google Fonts."""
    os.makedirs(FONT_DIR, exist_ok=True)
    if os.path.exists(SRC_WOFF2):
        return
    req = urllib.request.Request(GF_CSS, headers={"User-Agent": (
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/130.0 Safari/537.36")})
    css = urllib.request.urlopen(req).read().decode()
    block = css.split("/* latin */")[1]
    url = block.split("url(")[1].split(")")[0]
    urllib.request.urlretrieve(url, SRC_WOFF2)


def build_font(text):
    """Pin the axes into a static font and write a WOFF2 subset to only the used characters."""
    download_source()
    font = TTFont(SRC_WOFF2)
    # Pin unspecified axes to their defaults too, so it is no longer a variable font
    location = {a.axisTag: AXES.get(a.axisTag, a.defaultValue)
                for a in font["fvar"].axes}
    font = instancer.instantiateVariableFont(font, location, inplace=True)

    opts = Options()
    opts.layout_features = []          # no kerning (letters are placed by coordinates)
    opts.name_IDs = [1, 2, 4, 6, 16]
    opts.notdef_outline = False
    sub = Subsetter(options=opts)
    sub.populate(text=text)
    sub.subset(font)

    name = font["name"]
    for nid, val in ((1, FAMILY), (2, "Regular"), (4, FAMILY),
                     (6, FAMILY.replace(" ", "")), (16, FAMILY)):
        name.setName(val, nid, 3, 1, 0x409)
        name.setName(val, nid, 1, 0, 0)
    font.flavor = "woff2"
    font.save(OUT_WOFF2)
    return font


FONT = build_font(WORD + "Hx")         # H and x are used as metric references
CMAP = FONT.getBestCmap()
GLYPHS = FONT.getGlyphSet()
UPEM = FONT["head"].unitsPerEm


def bbox(ch):
    bp = BoundsPen(GLYPHS)
    GLYPHS[CMAP[ord(ch)]].draw(bp)
    return bp.bounds


CAP = bbox("H")[3]
SCALE = CAP_PX / CAP                   # font units → px
FONT_SIZE = CAP_PX * UPEM / CAP
OUTLINE_PX = MEASURED["outline"] * CAP_PX


# --------------------------------------------------------------------------
# 2. Spacing — place each letter at absolute coordinates following the measured ratios
# --------------------------------------------------------------------------
def gap_px(left_ch, right_ch):
    """Ink gap between two adjacent letters, tightened by gap_step per curved side."""
    n = ROUND_SIDES[left_ch][1] + ROUND_SIDES[right_ch][0]
    return (MEASURED["gap_flat"] - MEASURED["gap_step"] * n) * CAP_PX


def layout(word, rev_range=REVERSED):
    """Return each letter's origin x (px) and the total ink width."""
    origins, right = [], 0.0
    for i, ch in enumerate(word):
        x0, _, x1, _ = bbox(ch)
        pad = OUTLINE_PX / 2 if rev_range[0] <= i < rev_range[1] else 0.0
        ink_left = 0.0 if i == 0 else right + gap_px(word[i - 1], ch)
        origins.append(ink_left + pad - x0 * SCALE)
        right = ink_left + pad + (x1 - x0) * SCALE + pad
    return origins, right


def extents(word):
    """Top and bottom extents (px) relative to a baseline at 0."""
    top = max(bbox(c)[3] for c in word) * SCALE + OUTLINE_PX / 2
    bot = min(bbox(c)[1] for c in word) * SCALE - OUTLINE_PX / 2
    return top, bot


# --------------------------------------------------------------------------
# 3. SVG assembly
# --------------------------------------------------------------------------
def stylesheet():
    """Contents of the SVG <style>. The web font is referenced by URL."""
    if FONT_SOURCE == "googlefonts":
        return f"@import url('{GF_CSS}');"
    rel = os.path.relpath(OUT_WOFF2, HERE).replace(os.sep, "/")
    return (f"@font-face{{font-family:'{FAMILY}';font-style:normal;"
            f"font-weight:400;src:url('{rel}') format('woff2');}}")


def font_attrs():
    """Font attributes for <text>.

    font-variation-settings only works in browsers that handle variable fonts.
    Image viewers and SVG editors render at the default weight (usually 400), so
    font-weight is set as well to get a similar weight there.
    Browsers give font-variation-settings priority, so they render as specified.
    """
    if FONT_SOURCE == "selfhost":
        return (f'font-family="{FAMILY}" font-size="{FONT_SIZE:.3f}" '
                f'style="font-variant-ligatures:none" font-kerning="none"')
    var = ",".join(f"'{k}' {v:g}" for k, v in AXES.items())
    w = AXES.get("wght")
    fw = f' font-weight="{w:g}"' if w else ""
    return (f'font-family="{GF_FAMILY}"{fw} font-size="{FONT_SIZE:.3f}" '
            f'style="font-variation-settings:{var};'
            f'font-variant-ligatures:none" font-kerning="none"')


def text_element(word, ink, fill, rev_range=REVERSED):
    """Return a <text> with fill / stroke swapped for the reversed range only."""
    origins, _ = layout(word, rev_range)
    fmt = lambda xs: " ".join(f"{x:.3f}" for x in xs)
    spans = []
    for a, b in ((0, rev_range[0]), rev_range, (rev_range[1], len(word))):
        if a >= b:
            continue
        if a == rev_range[0]:
            style = (f'fill="{fill}" stroke="{ink}" '
                     f'stroke-width="{OUTLINE_PX:.2f}" stroke-linejoin="miter"')
        else:
            style = f'fill="{ink}"'
        spans.append(f'<tspan x="{fmt(origins[a:b])}" y="0" {style}>{word[a:b]}</tspan>')
    return f'<text {font_attrs()}>' + "".join(spans) + "</text>"


def write_wordmark(path, pad=(40, 30, 34)):
    """Wordmark. Transparent background, navy letters, iC in white fill + navy outline."""
    _, ink_w = layout(WORD)
    top, bot = extents(WORD)
    padx, padt, padb = pad
    w = ink_w + padx * 2
    h = (top - bot) + padt + padb
    out = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{w:.2f}" height="{h:.2f}" viewBox="0 0 {w:.2f} {h:.2f}" role="img" aria-label="SuiCash">
  <title>SuiCash</title>
  <style><![CDATA[{stylesheet()}]]></style>
  <g transform="translate({padx} {top + padt:.3f})">
    {text_element(WORD, INK, WHITE)}
  </g>
</svg>
'''
    open(path, "w").write(out)


def write_icon(path, size=512, radius=112, margin=74, mark="iC"):
    """Icon. A reversed iC centered on a Sui-blue rounded square.

    Same typeface and spacing rules as the wordmark, so the letterforms match side by side.
    """
    rev = (0, len(mark))
    _, ink_w = layout(mark, rev)
    top, bot = extents(mark)
    ink_h = top - bot
    k = (size - margin * 2) / max(ink_w, ink_h)
    tx = (size - ink_w * k) / 2
    ty = (size + ink_h * k) / 2 + bot * k
    out = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}" role="img" aria-label="SuiCash">
  <title>SuiCash icon</title>
  <style><![CDATA[{stylesheet()}]]></style>
  <rect width="{size}" height="{size}" rx="{radius}" fill="{BLUE}"/>
  <g transform="translate({tx:.3f} {ty:.3f}) scale({k:.5f})">
    {text_element(mark, WHITE, BLUE, rev)}
  </g>
</svg>
'''
    open(path, "w").write(out)


write_wordmark(os.path.join(HERE, "suicash-logo.svg"))
write_icon(os.path.join(HERE, "suicash-icon.svg"))
print("generated -> suicash-logo.svg, suicash-icon.svg")
