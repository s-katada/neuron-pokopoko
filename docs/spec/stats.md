# 統計の仕様 v2

ツリー・ノート詳細・統計画面に出す数値の定義。core の統計関数はこの文書だけを正とする。

## 定着率(retention)

- カード 1 枚の定着率 R は、fsrs の `current_retrievability(MemoryState { stability, difficulty }, t, decay)` で求める(式を自前で書かない)
  - `t` = (now − last_reviewed_at) / 86400(日。小数)
  - `decay` = 保存済み FSRS パラメータの 21 番目。未保存なら `FSRS6_DEFAULT_DECAY`([復習ログの仕様](review-log.md))
  - 性質: `t = stability` のとき R = 0.9、`t = 0` のとき R = 1
- 対象: `fsrs_state = 'review'` のカードだけ。未学習(new)・retired・削除済みノートのカードは含めない
- 集計: ノート・小分類・中分類・大分類ごとに、対象カードの R の**算術平均**
  - 上位の分類は、子の平均の平均ではなく、配下にある全対象カードの平均
  - 全体の定着率も、全対象カードの平均
- 対象カードが 0 枚なら null(画面では「—」)

## 日次レビュー数

- reviews の `reviewed_at` を `study_day_start`(JST 04:00 始まり)で区切った件数
- 直近 N 日(既定 30、1〜90)。今日を含み、古い日から新しい日の順。0 件の日も並べる
- 「今日」は `study_day_start(now)` の日

## 表示

- 日時は Asia/Tokyo で表示する(`Intl.DateTimeFormat`)。UTC 変換(`toISOString` 等)で日付を作らない
- 定着率は % の整数(四捨五入)

## バージョン

- この文書が v2(v1 から、decay を保存済みパラメータから取るように変えた)。定義を変えるときは、テストと同じ PR で更新する
