# regist-web 自動デプロイ(GitHub Actions self-hosted runner / Kali)

`face-regist` の `regist-web/**` に push されると、Kali サーバー上の
self-hosted runner が **checkout → npm ci → npm run build → 配信サービス再起動**
を自動実行する。ワークフローは `.github/workflows/deploy-regist-web.yml`。

```
push (face-regist, regist-web/**)
   → GitHub Actions
   → Kali の self-hosted runner: npm ci && npm run build
   → sudo systemctl restart suicash-regist-web
   → vite preview :1919(前段の Let's Encrypt リバースプロキシで公開)
```

## 前提

- Node.js(runner ユーザーで `npm` が使えること)
- リポジトリ管理者権限(runner 登録トークンの取得に必要)
- ポート 1919 を前段のリバースプロキシ(Azure 側)が中継している既存構成

## セットアップ(Kali サーバーで一度だけ)

### 1. runner 用ユーザー(任意だが推奨)

```sh
sudo useradd -m -s /bin/bash github-runner
sudo -iu github-runner    # 以降このユーザーで作業
```

### 2. self-hosted runner を登録

GitHub リポジトリ → Settings → Actions → Runners → **New self-hosted runner**
(Linux x64)に表示されるコマンドを runner ユーザーで実行する。ラベルに
`suicash` を追加すること(ワークフローが `runs-on: [self-hosted, suicash]`)。

```sh
mkdir -p ~/actions-runner && cd ~/actions-runner
curl -o actions-runner.tar.gz -L <ページに表示される URL>
tar xzf actions-runner.tar.gz
./config.sh --url https://github.com/yuki-js/suicash \
  --token <ページに表示される TOKEN> \
  --labels suicash --name kali-suicash --unattended
```

### 3. runner を常駐サービス化

```sh
sudo ./svc.sh install github-runner
sudo ./svc.sh start
```

### 4. 配信サービスを設置

`suicash-regist-web.service` の `User` と `WorkingDirectory` を環境に合わせて
書き換えてから設置する。`WorkingDirectory` は runner のワークスペース:
`/home/github-runner/actions-runner/_work/suicash/suicash/regist-web`
(初回ワークフロー実行後に作られる。先に一度ワークフローを走らせてもよい)。

```sh
sudo cp suicash-regist-web.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now suicash-regist-web
```

### 5. runner に再起動権限を与える(sudoers)

ワークフロー最後の `sudo systemctl restart suicash-regist-web` を
パスワード無しで許可する。

```sh
echo 'github-runner ALL=(root) NOPASSWD: /usr/bin/systemctl restart suicash-regist-web' \
  | sudo tee /etc/sudoers.d/suicash-runner
sudo chmod 440 /etc/sudoers.d/suicash-runner
```

## トレジャリー鍵 / エンドポイントの注入

チャージのトレジャリー鍵や RPC/faucet は 2 通りで渡せる:

- **ビルド時に埋め込む(推奨・runner 経由)**: GitHub リポジトリの
  Settings → Secrets and variables → Actions で
  `VITE_TREASURY_SECRET`(Secret)、`VITE_SUI_RPC` / `VITE_SUI_FAUCET`(Variables)
  を設定。ワークフローがビルド時に渡す。
- **実行時に渡す**: URL クエリ `?treasury=...&rpc=...&faucet=...`(localStorage に保存)。

## 動作確認

```sh
# runner 状態
sudo ~github-runner/actions-runner/svc.sh status
# 配信サービスのログ
sudo journalctl -u suicash-regist-web -f
```

push 後、GitHub の Actions タブでジョブ成功 → 数十秒でサイトに反映される。

## 手動デプロイ(runner を使わず)

```sh
cd <checkout>/regist-web && git pull && npm ci && npm run build \
  && sudo systemctl restart suicash-regist-web
```
