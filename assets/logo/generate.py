# -*- coding: utf-8 -*-
"""SuiCash ロゴ生成スクリプト。

  python3 assets/logo/generate.py

書き出すのは 2 ファイル。

  suicash-logo.svg   ワードマーク
  suicash-icon.svg   アプリ／ファビコン用アイコン（iC のみ）

グリフのパスは書かない。Google Fonts の可変フォント REM を <text> で呼び出し、
参照ロゴの字形に最も重なるウェイトを指定する。字間だけは、実測した比率どおりに
1 文字ずつ絶対座標で置く。数値の出どころは README を参照。

FONT_SOURCE で Web フォントの参照方法を選べる。

  "googlefonts" … Google Fonts から @import で読み込む（既定）
  "selfhost"    … font/ に書き出したサブセット WOFF2 を相対パスで読み込む

どちらの場合も、字送りの計算のためにローカルでフォントを解決する。
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
FAMILY = "SuiCash Wordmark"            # selfhost のときのファミリー名

# 参照ロゴ（JR東日本 Suica）の実測値。キャップハイト = 1.0 で正規化したもの。
# 4000px でラスタライズして画素単位で計測した。参照ロゴのパスは使っていない。
MEASURED = {
    "x_height": 0.711,      # x-ハイト
    "stem":     0.224,      # 小文字ステムの太さ
    "outline":  0.062,      # 白抜き文字の輪郭線の太さ
    "gap_flat": 0.444,      # 直線の側面どうしが隣り合うときのインク間のあき
    "gap_step": 0.039,      # 側面が曲線のとき、1 辺につき詰める量
}

# 参照ロゴの S / u / a のシルエットに最も重なるウェイト（73 書体から探索）。
AXES = {"wght": 605.0}

# 各文字の側面が曲線かどうか（左, 右）。字間の光学補正に使う。
ROUND_SIDES = {"S": (1, 1), "u": (0, 0), "i": (0, 0),
               "C": (1, 1), "a": (1, 0), "s": (1, 1), "h": (0, 0)}

INK = "#0A1A2F"                        # 濃紺
BLUE = "#4DA2FF"                       # Sui ブルー
WHITE = "#FFFFFF"

WORD = "SuiCash"
REVERSED = (2, 4)                      # "iC"（WORD[2:4]）を白抜きにする
CAP_PX = 100.0                         # キャップハイトを 100px として組む


# --------------------------------------------------------------------------
# 1. フォントの取得 → インスタンス化 → サブセット
# --------------------------------------------------------------------------
def download_source():
    """Google Fonts から latin サブセットの可変フォントを落としてキャッシュする。"""
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
    """軸を固定して静的フォントにし、使う文字だけに絞った WOFF2 を書き出す。"""
    download_source()
    font = TTFont(SRC_WOFF2)
    # 指定しなかった軸も既定値で固定して、可変フォントではなくしてしまう
    location = {a.axisTag: AXES.get(a.axisTag, a.defaultValue)
                for a in font["fvar"].axes}
    font = instancer.instantiateVariableFont(font, location, inplace=True)

    opts = Options()
    opts.layout_features = []          # カーニングは使わない（座標で置くため）
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


FONT = build_font(WORD + "Hx")         # H, x は寸法の基準に使う
CMAP = FONT.getBestCmap()
GLYPHS = FONT.getGlyphSet()
UPEM = FONT["head"].unitsPerEm


def bbox(ch):
    bp = BoundsPen(GLYPHS)
    GLYPHS[CMAP[ord(ch)]].draw(bp)
    return bp.bounds


CAP = bbox("H")[3]
SCALE = CAP_PX / CAP                   # フォント単位 → px
FONT_SIZE = CAP_PX * UPEM / CAP
OUTLINE_PX = MEASURED["outline"] * CAP_PX


# --------------------------------------------------------------------------
# 2. 字間 — 実測した比率どおりに 1 文字ずつ絶対座標で置く
# --------------------------------------------------------------------------
def gap_px(left_ch, right_ch):
    """隣り合う 2 文字のインク間のあき。曲線の側面 1 辺につき gap_step 詰める。"""
    n = ROUND_SIDES[left_ch][1] + ROUND_SIDES[right_ch][0]
    return (MEASURED["gap_flat"] - MEASURED["gap_step"] * n) * CAP_PX


def layout(word, rev_range=REVERSED):
    """各文字の origin の x 座標（px）と、全体のインク幅を返す。"""
    origins, right = [], 0.0
    for i, ch in enumerate(word):
        x0, _, x1, _ = bbox(ch)
        pad = OUTLINE_PX / 2 if rev_range[0] <= i < rev_range[1] else 0.0
        ink_left = 0.0 if i == 0 else right + gap_px(word[i - 1], ch)
        origins.append(ink_left + pad - x0 * SCALE)
        right = ink_left + pad + (x1 - x0) * SCALE + pad
    return origins, right


def extents(word):
    """ベースラインを 0 としたときの、上端と下端（px）。"""
    top = max(bbox(c)[3] for c in word) * SCALE + OUTLINE_PX / 2
    bot = min(bbox(c)[1] for c in word) * SCALE - OUTLINE_PX / 2
    return top, bot


# --------------------------------------------------------------------------
# 3. SVG の組み立て
# --------------------------------------------------------------------------
def stylesheet():
    """SVG に入れる <style> の中身。Web フォントは URL で参照する。"""
    if FONT_SOURCE == "googlefonts":
        return f"@import url('{GF_CSS}');"
    rel = os.path.relpath(OUT_WOFF2, HERE).replace(os.sep, "/")
    return (f"@font-face{{font-family:'{FAMILY}';font-style:normal;"
            f"font-weight:400;src:url('{rel}') format('woff2');}}")


def font_attrs():
    """<text> に載せるフォント指定。

    font-variation-settings は可変フォントを解釈するブラウザでしか効かない。
    画像ビューアや SVG エディタは既定ウェイト（多くは 400）で描いてしまうので、
    font-weight も併記して、そういう環境でも近い太さになるようにする。
    ブラウザでは font-variation-settings が優先されるので指定どおりになる。
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
    """白抜きの範囲だけ fill / stroke を入れ替えた <text> を返す。"""
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
    """ワードマーク。透過背景、文字は濃紺、iC は白フィル＋濃紺の輪郭線。"""
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
    """アイコン。Sui ブルーの角丸スクエアに、白抜きの iC を中央に置く。

    ワードマークと同じ書体・同じ字間規則なので、並べても字形が揃う。
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
