# SuiCash Logo

Based on the logo requirements in `.opencode/skills/suicash/skill.md`.

| File | Purpose |
| --- | --- |
| `suicash-logo.svg` | Wordmark (737 B) |
| `suicash-icon.svg` | App / favicon version, 512×512, using only `iC` (682 B) |
| `generate.py` | Script that generates the two files above |
| `font/suicash-wordmark.woff2` | Self-hosted subset font for the wordmark (1.2 KB) |

Neither SVG contains glyph paths. They are composed using `<text>`, and the web font is loaded via `@import` from Google Fonts.

```xml
<style><![CDATA[@import url('https://fonts.googleapis.com/css2?family=REM:wght@100..900');]]></style>
<text font-family="REM" font-weight="605" style="font-variation-settings:'wght' 605">
```

`font-variation-settings` only works in browsers that support variable fonts, so `font-weight` is also included. Without it, image viewers and SVG editors will render the text at the default weight (400), which appears too thin.

## Notes on loading

SVGs that reference external fonts will only have the font applied when they are either:
- embedded inline in HTML, or
- opened directly as an SVG file

If they are used as `<img src="...">`, CSS `background-image`, or favicons, browsers typically block external resource loading, so the fallback font is used instead. In that case, use one of the following:

- Set `FONT_SOURCE = "selfhost"` in `generate.py` and reference `font/suicash-wordmark.woff2` via a relative path
  (this removes the external domain dependency, but the `<img>` limitation remains the same)
- Export the asset as PNG for the intended use case
- Convert the text to outlines for distribution

## Why this font?

I chose `REM` because among the fonts published on Google Fonts, it matched the intended mood the best.

## Composition

`SuiCash` can be decomposed as `Su + iC + ash`, and it follows the same positional relationship as the reference logo where `Su + iC + a` uses the `iC` as a white-cutout. This `iC` is inverted by using a white fill with a dark outline in the primary text color, indicating that it represents an IC card.

### 4. Character spacing

To make the **ink gap** between adjacent characters match the rule above, each glyph is positioned one by one using absolute coordinates with `<tspan x="...">`. This avoids dependence on the font’s side bearings or kerning.

After rendering at 4000px and measuring it, the difference from the target values for all pairs stayed within the quantization error range of the rendering resolution (±1 px).

## Color palette

| | |
| --- | --- |
| Deep navy (body text) | `#0A1A2F` |
| Sui blue | `#4DA2FF` |

The primary color is Sui blue instead of the green used in the reference logo.

## Known differences / not yet addressed

- The reference logo has a horizontally elongated dot on the `i`, while REM’s dot is round. When the dot is inverted to white, the outline becomes more visible and thus more prominent. The numeric match is still the best possible, but this is the one point where it diverges in the opposite direction.
- The stem cutaway detail in the reference logo has not been reproduced.

## Regenerate

```bash
python3 -m venv venv
./venv/bin/pip install fonttools brotli
./venv/bin/python assets/logo/generate.py
```

To calculate letter spacing, the REM variable font is fetched from Google Fonts and cached in `TMPDIR`.

## Exporting PNGs

The SVG is the source of truth. PNGs are generated files and are not tracked in git (`.gitignore` is already configured).

```bash
npm --prefix assets install
npm --prefix assets run generate-png       # export 2x PNGs alongside the SVGs
node assets/generate-png.mjs --scale=3     # change the scale factor
```

Because the logo text is drawn with `<text>` using REM (wght 605), the script retrieves `REM[wght].ttf` and passes it to resvg. Network access is required only on the first run (cached in `TMPDIR`). If you want to use a local font, set `SUICASH_REM_TTF=/path/to/REM.ttf`.
