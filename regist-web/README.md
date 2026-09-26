# SuiCash 登録サイト (regist-web)

利用者のスマホ向け登録フロントエンド。カード番号(IDi)の登録 →
Sui ウォレット発行 → 顔認証の利用登録(ダミー)→ チャージ、までを行う。
決済時の顔照合は店舗の認証端末(Hi-CARA、`face-auth-ui/`)の中だけで行われる。

```sh
npm install
npm run dev      # 開発サーバ
npm run build    # 型チェック + 本番ビルド
```

## デプロイ

### 本番: GitHub Pages(自動デプロイ)

`face-regist` の `regist-web/**` に push すると、Actions
(`.github/workflows/pages.yml`。`site/` の紹介ページと一緒に `/app/` として)がビルドして GitHub Pages に公開する。
**HTTPS が自動で付く**ので、スマホの券面カメラ OCR(`getUserMedia` は
secure context 必須)もそのまま動く。サーバー・Docker・SSH は不要。

- 公開 URL: **https://yuki-js.github.io/suicash/app/**(ルートは `site/` の紹介ページ。旧 URL に `?treasury=` 等を付けたリンクは `/app/` へ転送される)
- 初回のみ GitHub 側で有効化: Settings → Pages → Build and deployment →
  **Source: GitHub Actions**
- `face-regist`(デフォルトブランチ以外)から公開するため、環境
  `github-pages` の保護ルールでこのブランチのデプロイが弾かれる場合は、
  Settings → Environments → github-pages → Deployment branches に
  `face-regist` を許可する(または main にマージ)
- トレジャリー鍵 / RPC / faucet を埋め込むなら Settings → Secrets and
  variables → Actions に `VITE_TREASURY_SECRET`(Secret)、
  `VITE_SUI_RPC` / `VITE_SUI_FAUCET`(Variables)を設定
- 相対パス出力(`base: "./"`)なのでサブパス `/suicash/` 配下で動く。
  SPA フォールバック用に `404.html` と `.nojekyll` はワークフローが生成する

### ローカル確認

```sh
npm run dev        # 開発サーバ(HMR、:5173)
npm run preview    # 本番ビルドの確認(:4173)
```

> **カメラ読み取りの注意**: 券面 OCR は `getUserMedia` を使うため
> **secure context(HTTPS または localhost)必須**。GitHub Pages は HTTPS
> なので問題ない。ローカルは `http://localhost` なら可(LAN の IP 平文は不可)。

## 登録フロー

```mermaid
flowchart TD
    A["1. カード番号(IDi)登録<br/>KA + 英数字15文字(計17文字)<br/>手入力 or カメラ読み取り(券面OCR)"]
    B["2. Sui ウォレット発行<br/>鍵は端末内で生成(IDi から導出しない)"]
    C["3. 顔認証の利用登録(ダミー)<br/>顔データは収集しない"]
    D["ダッシュボード<br/>残高表示・チャージ(testnet faucet)"]
    A --> B --> C --> D
```

## Sui エンドポイント(429 レート制限対策)

チャージや残高取得に使う公開 testnet エンドポイント(fullnode RPC / faucet)は
共有 IP からのアクセスが集中すると **429(レート制限)** を返す。デモで安定させたい
場合は、自前 or 別の RPC / faucet に差し替えられる(優先順:URL クエリ >
localStorage > ビルド時 env > 既定):

| 対象 | URL クエリ | localStorage | ビルド時 env |
| --- | --- | --- | --- |
| fullnode RPC | `?rpc=<url>` | `suicash.rpc` | `VITE_SUI_RPC` |
| faucet | `?faucet=<url>` | `suicash.faucet` | `VITE_SUI_FAUCET` |

例: `https://<host>/?rpc=https://your-node/....&faucet=https://your-faucet/...`
(一度クエリで渡すと localStorage に保存され、次回以降は付けなくてよい)

429 が出たときは UI 側でクールダウン(再試行まで N 秒)を表示する。faucet は
同一アドレス・同一 IP への連続要求を制限するので、少し間隔をあけて試すこと。

### 429 を根本回避:トレジャリー送金(推奨)

公開 faucet の 429 を完全に避けるには、**事前入金済みの testnet アカウント
(トレジャリー)から送金**する。faucet を一切叩かないので 429 は出ない。

1. トレジャリー鍵を作って testnet SUI を入れる:
   ```sh
   sui client new-address ed25519           # suiprivkey1... を控える
   sui client switch --address <その address>
   sui client faucet                         # 何度か。デモ人数ぶん貯める
   sui keytool export --key-identity <address>   # suiprivkey1... を取得
   ```
2. その `suiprivkey1...` を実行時に注入(**リポジトリには入れない**):

   | 方法 | 指定 |
   | --- | --- |
   | URL クエリ | `?treasury=suiprivkey1...`(一度で localStorage に保存) |
   | ビルド時 env | `VITE_TREASURY_SECRET=suiprivkey1...` |

3. 設定されていれば「チャージ」は自動でトレジャリー送金(1 回 0.2 SUI)に切り替わる。
   未設定なら従来どおり faucet にフォールバック。

> testnet 専用・デモ用途。鍵はブラウザに載る(静的サイトのため)ので、
> 価値のある鍵は使わないこと。残高が尽きたら `sui client faucet` で補充。

## プライバシー設計

- **IDi(カード番号)は外部へ送らない**。外部(チェーン・IDi 検証サーバー)に
  渡すのは `hash(IDi, salt)` のコミットメントのみ。生の IDi と salt は
  この端末の localStorage にだけ保存する
  (コミットメント方式は暫定 SHA-256。検証サーバー側の ZKP 回路が
  決まり次第 Poseidon 等へ差し替える — `src/lib/idi.ts`)
- **顔データは収集しない**。ステップ 3 はプライバシー保護のための
  意図的なダミー登録(カメラも起動しない)。実際の顔照合は決済時に
  認証端末の内部だけで行われ、そこでも顔データは端末外へ出ない
- **鍵は IDi から導出しない**。IDi はリーダーで誰でも読める値のため、
  ウォレット鍵はランダム生成し、IDi(コミットメント)→ウォレットの
  対応付けはルータ/検証サーバー(別担当)がチェーン上で解決する設計
- 券面 OCR の撮影画像もブラウザ内でのみ処理し、外部送信しない

## 券面 OCR(カメラ読み取り)

認証端末(Hi-CARA)で実績のある読み方を Web(canvas + Tesseract.js)に移植:

1. カードガイド枠(「券面を枠の中に合わせてください」)に合わせて撮影
2. 枠に対する固定位置(`ID_ROI`)で ID 行だけを切り出し
3. 適応二値化 + 白抜き印字の自動反転で「白地×黒文字」に正規化
4. 英数字ホワイトリストで 1 行 OCR → 英字→数字の見間違い補正 → 17 文字整形
5. **結果は入力欄にプリフィルし、必ず利用者が確認・修正して確定**

ROI・ガイド枠の位置は `src/lib/ocr.ts`(計算)と `src/styles.css` の
`.scan__guide` / `.scan__idbox`(表示)で一致させてある。ズレたら両方直すこと。

## ディレクトリ

```
src/
  App.tsx                 ステップ進行(IDi → ウォレット → 顔 → ダッシュボード)
  lib/
    idi.ts                IDi の検証・整形・コミットメント計算
    ocr.ts                券面 OCR(前処理 + Tesseract.js + 整形)
    wallet.ts             Sui ウォレット(testnet)・残高・faucet チャージ
    storage.ts            登録状態の localStorage 永続化
  components/
    IdiStep.tsx           IDi 登録(手入力 + カメラ読み取り)
    WalletStep.tsx        ウォレット発行
    FaceStep.tsx          顔認証の利用登録(ダミー・非収集)
    Dashboard.tsx         残高・チャージ・登録情報
    Logo.tsx              SuiCash ワードマーク
Dockerfile / nginx.conf   ポート 1919 で配信
```
