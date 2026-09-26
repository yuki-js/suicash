# site — SuiCash 紹介サイト

SuiCash のランディングページ。依存なしの静的 HTML 1 枚(`index.html`)で、
ロゴ・カード券面・マスコットの SVG(`../assets/` 由来)をインラインで埋め込んでいる。

- フォント: REM + Noto Sans JP(Google Fonts)
- ライト/ダークテーマ両対応(`prefers-color-scheme` + `data-theme` 上書き)
- ビルド不要。ブラウザで `index.html` を開くだけで表示できる

## 公開について

GitHub Pages は現在 `regist-web` がルートを使っている
(`.github/workflows/pages-regist-web.yml`)。このサイトも Pages に載せる場合は、
デプロイステップで両方を 1 つの成果物にまとめる(例: ルートに site、`/app` に
regist-web など)必要がある。

## 注意

`assets/` の SVG を更新したら、このページ内のインラインコピーも合わせて更新すること。
