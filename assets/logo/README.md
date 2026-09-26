# SuiCash ロゴ

`.opencode/skills/suicash/skill.md` のロゴ要件にもとづく。

| ファイル | 用途 |
| --- | --- |
| `suicash-logo.svg` | ワードマーク（737 B） |
| `suicash-icon.svg` | アプリ／ファビコン用 512×512、`iC` のみ（682 B） |
| `generate.py` | 上記 2 点の生成スクリプト |
| `font/suicash-wordmark.woff2` | 自前ホスト用のサブセットフォント（1.2 KB） |

どちらの SVG もグリフのパスを持たない。`<text>` で組み、Web フォントは
`@import` で Google Fonts から読み込む。

```xml
<style><![CDATA[@import url('https://fonts.googleapis.com/css2?family=REM:wght@100..900');]]></style>
<text font-family="REM" font-weight="605" style="font-variation-settings:'wght' 605">
```

`font-variation-settings` は可変フォントを解釈するブラウザでしか効かないので、
`font-weight` も併記している。これが無いと、画像ビューアや SVG エディタでは
既定ウェイト（400）で細く描画される。

## 読み込み方の注意

外部フォントを参照する SVG は、**HTML にインライン展開するか、SVG を直接開いた
ときにしかフォントが適用されない**。`<img src="...">`・CSS の `background-image`・
favicon として使うと、ブラウザが外部リソースの読み込みを遮断するため
フォールバック書体で描画される。その場合は次のいずれか。

- `generate.py` の `FONT_SOURCE = "selfhost"` にして
  `font/suicash-wordmark.woff2` を相対パスで参照する
  （外部ドメインへの依存は無くなるが、`<img>` の制約は同じ）
- 用途に応じて PNG を書き出す
- 配布用にアウトライン化する

## なぜこのフォント？

`REM` は、Google Fonts で公開されているフォントのなかで一番雰囲気がいいなって思ったからです。

## 組み方

`SuiCash` は `Su + iC + ash` と分解でき、**なにか** のロゴが `Su + iC + a` の
`iC` を白抜きにしているのと同じ位置関係になる。この `iC` だけを
「白フィル + 本文色の輪郭線」で反転させ、IC カードであることを示す。


### 4. 字間

隣り合う文字の**インクの間隔**が上の規則どおりになるよう、
`<tspan x="...">` で 1 文字ずつ絶対座標に置いている。
フォント側のサイドベアリングやカーニングには依存しない。

4000px でレンダリングして実測した結果、全ペアで目標値との差は
レンダリング解像度の量子化誤差（±1px）の範囲に収まっている。

## 配色

| | |
| --- | --- |
| 濃紺（本文） | `#0A1A2F` |
| Sui ブルー | `#4DA2FF` |

**なにか** のグリーンではなく Sui ブルーを主色にしている。

## 既知の差異・未対応

- 参照ロゴの `i` のドットは横長の矩形だが、REM のドットは丸い。
  白抜きにすると輪郭線がつくぶん目立つ。数値上の一致度は最良だが、
  この一点だけは逆方向に外れている。
- 参照ロゴが持つステムの削ぎ落とし（前述）は再現できていない。

## 再生成

```bash
python3 -m venv venv
./venv/bin/pip install fonttools brotli
./venv/bin/python assets/logo/generate.py
```

字送りの計算のために、Google Fonts から REM の可変フォントを取得する
（`TMPDIR` にキャッシュ）。

## PNG の書き出し

SVG が正本。PNG は生成物で git 管理しない（`.gitignore` 済み）。

```bash
npm --prefix assets install
npm --prefix assets run generate-png       # 2x を SVG と同名で書き出す
node assets/generate-png.mjs --scale=3     # 倍率を変える
```

ロゴの `<text>` は REM（wght 605）で描くため、スクリプトが
`REM[wght].ttf` を取得して resvg に渡す。初回のみネットワークが必要
（`TMPDIR` にキャッシュ）。手元のフォントを使う場合は
`SUICASH_REM_TTF=/path/to/REM.ttf` を指定する。
