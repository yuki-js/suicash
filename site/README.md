# site — SuiCash landing page

The SuiCash landing page. A single dependency-free static HTML file (`index.html`) that inlines
the logo, card face, and mascot SVGs (from `../assets/`).

- Fonts: REM + Noto Sans JP (Google Fonts)
- Supports light/dark themes (`prefers-color-scheme` + `data-theme` override)
- No build step. Just open `index.html` in a browser
- Bilingual (Japanese/English). Japanese is the HTML source text; English lives in the `EN`
  dictionary in the trailing script (keys via `data-i18n` / `data-i18n-aria` /
  `data-i18n-content` / `data-i18n-alt`).
  The language is picked from `?lang=ja|en` → the previous choice (localStorage) → the browser
  language, in that order, and can be switched with JA/EN in the nav.
  When adding text, put a key on the element and add the same key to `EN`
- Device screenshots in `img/` are WebP (converted from the original PNGs at quality 82)
- Local preview: `npx live-server --port=5500 site` (live reload on save)

## Publishing

When `site/**` or `regist-web/**` on main is updated, `.github/workflows/pages.yml` bundles
both into one artifact and publishes it to GitHub Pages.

- `/`      … this landing page (`site/` copied as-is, excluding the README)
- `/app/`  … the registration site (build output of `regist-web`)

Links from when the registration site lived at the root that carry `?treasury=` / `?rpc=` /
`?faucet=` are forwarded, query included, to `/app/` by the script at the top of this page.

## Notes

When you update an SVG in `assets/`, update its inline copy in this page as well.
