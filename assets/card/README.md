# SuiCash カード券面

`suicash-card.svg` — SuiCash プリペイドカードの券面(交通系 IC カード風の
オリジナルデザイン)。地はシルバー、左にテーマカラー(アクア #4DA2FF 系)の
台形パネル、その左下にワードマーク、右にマスコットを配置。viewBox は 172×108。

- `face-auth-ui/src/components/SuiCashCard.tsx` のインライン SVG を単独ファイル化したもの。
- ワードマークは [`../logo/suicash-logo.svg`](../logo/suicash-logo.svg) と同じ REM 605 の組み方。
  フォントは Google Fonts から `@import` で読み込む(オフラインでは sans-serif にフォールバック)。
- マスコットは [`../mascot/suicash-mascot.svg`](../mascot/suicash-mascot.svg) を縮小配置。
- `suicash-card-in.gif` — カードをリーダーへ挿入するループアニメーション(透過背景、465×487)。
