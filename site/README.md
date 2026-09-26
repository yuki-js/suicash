# site — SuiCash 紹介サイト

SuiCash のランディングページ。依存なしの静的 HTML 1 枚(`index.html`)で、
ロゴ・カード券面・マスコットの SVG(`../assets/` 由来)をインラインで埋め込んでいる。

- フォント: REM + Noto Sans JP(Google Fonts)
- ライト/ダークテーマ両対応(`prefers-color-scheme` + `data-theme` 上書き)
- ビルド不要。ブラウザで `index.html` を開くだけで表示できる
- 日英 2 言語対応。日本語は HTML の原文、英語は末尾スクリプトの `EN` 辞書
  (`data-i18n` / `data-i18n-aria` / `data-i18n-content` / `data-i18n-alt` のキー)。
  言語は `?lang=ja|en` → 前回の選択(localStorage)→ ブラウザ言語の順で決まり、ナビの JA/EN で切り替え。
  文言を足すときは要素にキーを付け、`EN` に同じキーを追加する
- `img/` の実機スクリーンショットは WebP(元 PNG から quality 82 で変換)
- ローカル確認: `npx live-server --port=5500 site`(保存でライブリロード)

## 公開について

main の `site/**` か `regist-web/**` が更新されると、`.github/workflows/pages.yml` が
両方を 1 つの成果物にまとめて GitHub Pages に公開する。

- `/`      … このランディングページ(`site/` をそのままコピー。README は除外)
- `/app/`  … 登録サイト(`regist-web` のビルド成果物)

登録サイトがルートにあった頃のリンクで `?treasury=` / `?rpc=` / `?faucet=` が
付いているものは、このページの先頭スクリプトが `/app/` へクエリごと転送する。

## 注意

`assets/` の SVG を更新したら、このページ内のインラインコピーも合わせて更新すること。
