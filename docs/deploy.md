# 本番セットアップ手順

本番(Cloudflare の workers.dev)で毎日使えるようにするための、**人が行う**初期設定。上から順に 1 回だけ行う。

前提: このリポジトリで `direnv allow` 済み(wrangler・just が使える)。`wrangler login` 済み(`wrangler whoami` で確認)。

## 1. 本番 D1 を作る

```bash
cd crates/worker
wrangler d1 create neuron-pokopoko
```

- 出力の `database_id` を `crates/worker/wrangler.toml` の `database_id`(今は全ゼロ)に書き、PR にしてマージする。ID は秘密情報ではない(Claude に渡せば PR を作る)
- テーブルの作成(マイグレーション)は手で流さない。デプロイのジョブが `--remote` で適用する

## 2. API トークンと GitHub の secrets

1. Cloudflare ダッシュボード → My Profile → API Tokens → Create Token → テンプレート「Edit Cloudflare Workers」を選び、権限に「Account / D1 / Edit」を足して作る
2. GitHub → このリポジトリ → Settings → Secrets and variables → Actions に 2 つ登録する
   - `CLOUDFLARE_API_TOKEN`: 1 のトークン
   - `CLOUDFLARE_ACCOUNT_ID`: `wrangler whoami` に出る Account ID
3. 1 の PR(または次の PR)を main にマージすると、CI が通ったあと deploy ジョブが動く。Actions のログに出る URL(`https://neuron-pokopoko.<サブドメイン>.workers.dev`)を控える

secrets か D1 の ID が未設定の間、deploy ジョブは警告を出してスキップする(CI は緑のまま)。

## 3. Access で保護する

1. Workers & Pages → neuron-pokopoko → Settings → Domains & Routes → workers.dev の「Enable Cloudflare Access」
2. 作られた Access アプリケーションのポリシーを「自分のメールアドレスだけ Allow」にする
3. プレビュー URL は `wrangler.toml` の `preview_urls = false` で無効化済み(抜け道にならない)

## 4. CLI 用のサービストークン

1. Zero Trust → Access → Service credentials → Service Tokens → Create Service Token(名前: `poko-cli`)。Client ID と Client Secret を控える(Secret は一度しか表示されない)
2. 3 の Access アプリケーションにポリシーを追加: Action「Service Auth」、Include「Service Token: poko-cli」
3. 手元の環境変数に設定する(**リポジトリにはコミットしない**)。fish の場合:

```fish
set -Ux POKO_ENDPOINT https://neuron-pokopoko.<サブドメイン>.workers.dev
set -Ux POKO_ACCESS_CLIENT_ID <Client ID>
set -Ux POKO_ACCESS_CLIENT_SECRET <Client Secret>
```

消すときは `set -eU <名前>`。

## 5. 確認する

```bash
just verify-prod
```

次の 3 点を確かめる: トークン無しの `/api/health` は拒否される / トークン有りで 200 / 同期の入口(`/api/sync/manifest`)に届く。

## 6. スマホのホーム画面に置く

スマホのブラウザで本番 URL を開いて Access にログインし、共有メニューから「ホーム画面に追加」。Access のログインが切れたときは、画面が自動で再読み込みしてログイン画面に移る。

## 日々の使い方

- ノートを清書したら `poko sync <vault>`(`POKO_ENDPOINT` が本番を指していれば本番へ送られる)
- コードの変更は PR → マージ。CI が通ったときだけ自動でデプロイされ、マイグレーションも適用される
