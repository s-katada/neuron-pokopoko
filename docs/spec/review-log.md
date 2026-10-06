# 復習ログの退避・復元・最適化 v1

`poko export` / `poko import` / `poko optimize` と、保存した FSRS パラメータの使い方。core の該当関数はこの文書だけを正とする。

## JSONL(export / import)

- 1 行 1 レビュー。reviews の id の昇順
- 項目: `card_key`・`rating`(again / hard / good / easy)・`reviewed_at`(Unix 秒)・`interval_days`・`stability`・`difficulty`・`response`(null 可)
- id は書かない。行の同一性は `(card_key, reviewed_at)` で見る

## 復元(import)

- 送る前に全行を検証する。不正な行が 1 つでもあれば何も送らない
- 入れない行
  - カードが無い(retired・ノート削除済みを含む)
  - 同じ `(card_key, reviewed_at)` が、DB かファイルの前の行にある
- 何度流しても結果は同じ
- 入れたカードの FSRS 列は、そのカードの全レビューから作り直す(回答時と同じ値になる)
  - `stability`・`difficulty`・`last_reviewed_at`: 最後のレビュー(`reviewed_at` の順、同時刻は id の順)
  - `due_at` = 最後の `reviewed_at` + round(`interval_days` × 86400)
  - `reps` = レビュー数、`lapses` = 2 回目以降の again の数
- vault から消えたカードのレビューは戻せない。card_key の一覧を出して終了コード 1

## 最適化(optimize)

- 学習データ: カードごとに時刻順に並べ、2 回目以降の各レビューについて、そこまでの履歴を 1 件にする
  - 評価: again=1・hard=2・good=3・easy=4
  - 経過日数: 1 回目は 0、以降は前回からの秒差 ÷ 86400 の切り捨て(出題の計算と同じ)
  - 最後の経過日数が 0(同じ日の復習)の件は使わない
- 保存するのは、次の両方を満たすときだけ
  - 学習データが `MIN_OPTIMIZE_ITEMS` 件以上
  - fsrs の結果が既定値と違う(fsrs はデータが足りないと既定値を返す)
- `--dry-run` は計算して表示するだけ

## パラメータの保存と使い方

- D1 の `fsrs_params` に 1 行だけ持つ。FSRS-6 の 21 個で、fsrs の検証を通る値だけを保存する
- 出題: 回答時の次の間隔は、保存値があればそれで、無ければ既定値で計算する
- 統計: 定着率の decay は保存値の 21 番目。無ければ既定値([統計の仕様](stats.md))
- 保存値が壊れていたら既定値を使う

## 定数(core の 1 か所で定義する)

| 名前 | 値 |
|---|---|
| `EXPORT_PAGE_DEFAULT` | 200 |
| `EXPORT_PAGE_MAX` | 500 |
| `IMPORT_BATCH_MAX` | 200 |
| `MIN_OPTIMIZE_ITEMS` | 400 |

## バージョン

- この文書が v1。規則を変えるときは、テストと同じ PR で更新する
