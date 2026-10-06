# 本番セットアップ手順

本番(`https://neuron-pokopoko.digletts.dev`)で毎日使えるようにするための、**人が行う**初期設定。上から順に 1 回だけ行う。

前提:

- このリポジトリで `direnv allow` 済み(wrangler・just が使える)。`wrangler login` 済み(`wrangler whoami` で確認)
- `digletts.dev` が同じ Cloudflare アカウントのゾーンにある。`neuron-pokopoko.digletts.dev` の DNS レコードは手で作らない(デプロイがカスタムドメインとして作り、証明書も発行する)

入口はこのカスタムドメインだけ。workers.dev とプレビュー URL は `wrangler.toml` の `workers_dev = false` と `preview_urls = false` で閉じてある(Access の抜け道にならない)。

## 1. 本番 D1 を作る

```bash
cd crates/worker
wrangler d1 create neuron-pokopoko
```

- 出力の `database_id` を `crates/worker/wrangler.toml` の `database_id` に書き、PR にしてマージする。ID は秘密情報ではない(Claude に渡せば PR を作る)
- テーブルの作成(マイグレーション)は手で流さない。デプロイのジョブが `--remote` で適用する

## 2. Access で保護する

**最初のデプロイ(3)より前に行う。** 先にデプロイすると、Access を作るまでドメインを誰でも開ける。

1. Zero Trust → Access → Applications → Add an application → Self-hosted。ドメインは `neuron-pokopoko.digletts.dev`(パスは空 = 全体)
2. ポリシーを追加: Action「Allow」、Include「Emails: 自分のメールアドレス」
3. Zero Trust → Access → Service credentials → Service Tokens → Create Service Token(名前: `poko-cli`)。Client ID と Client Secret を控える(Secret は一度しか表示されない)
4. 1 のアプリにポリシーを追加: Action「Service Auth」、Include「Service Token: poko-cli」

セッションの長さはアプリの設定で変えられる。毎日スマホで開くなら長めにするとログインの手間が減る。

## 3. API トークンと GitHub の secrets

1. Cloudflare ダッシュボード → My Profile → API Tokens → Create Token → テンプレート「Edit Cloudflare Workers」を選び、権限に「Account / D1 / Edit」を足して作る。Zone Resources には `digletts.dev` を含める
2. GitHub → このリポジトリ → Settings → Secrets and variables → Actions に 2 つ登録する
   - `CLOUDFLARE_API_TOKEN`: 1 のトークン
   - `CLOUDFLARE_ACCOUNT_ID`: `wrangler whoami` に出る Account ID
3. 1 の PR(または次の PR)を main にマージすると、CI が通ったあと deploy ジョブが動き、`https://neuron-pokopoko.digletts.dev` に公開される

secrets か D1 の ID が未設定の間、deploy ジョブは警告を出してスキップする(CI は緑のまま)。

## 4. CLI の環境変数

手元の環境変数に設定する(**リポジトリにはコミットしない**)。fish の場合:

```fish
set -Ux POKO_ENDPOINT https://neuron-pokopoko.digletts.dev
set -Ux POKO_ACCESS_CLIENT_ID <2 で控えた Client ID>
set -Ux POKO_ACCESS_CLIENT_SECRET <2 で控えた Client Secret>
```

消すときは `set -eU <名前>`。

## 5. 確認する

```bash
just verify-prod
```

次の 3 点を確かめる: トークン無しの `/api/health` は拒否される / トークン有りで 200 / 同期の入口(`/api/sync/manifest`)に届く。

## 6. スマホのホーム画面に置く

スマホのブラウザで `https://neuron-pokopoko.digletts.dev` を開いて Access にログインし、共有メニューから「ホーム画面に追加」。Access のログインが切れたときは、画面が自動で再読み込みしてログイン画面に移る。

## 日々の使い方

- ノートを清書したら `poko sync <vault>`(`POKO_ENDPOINT` が本番を指していれば本番へ送られる)
- コードの変更は PR → マージ。CI が通ったときだけ自動でデプロイされ、マイグレーションも適用される

## 復習ログの退避と最適化

- **退避**(週 1 回など): `poko export <ファイル>`。履歴が残る場所に置く。例: `mkdir -p <vault>/.poko` してから `poko export <vault>/.poko/reviews.jsonl` し、vault と一緒にコミットする(poko が読むのは `learning/` の下だけなので、同期には影響しない)
- **復元**(D1 を作り直したとき): 新しい D1 の ID を `wrangler.toml` に入れてデプロイ → `poko sync <vault>` → `poko import <ファイル>`。「カードが無い」と出た行は vault から消えたノートの分で、戻せない
- **最適化**(月 1 回など): `poko optimize --dry-run` で結果を見てから `poko optimize`。学習に使える復習が 400 件に満たないうちは何も変えない。保存した値は、その後の回答と統計から使われる
- 形式と条件の詳細は [docs/spec/review-log.md](spec/review-log.md)
