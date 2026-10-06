-- 最適化した FSRS パラメータ。行は id = 1 の 1 行だけ。params は 21 個の数値の JSON
CREATE TABLE fsrs_params (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  params TEXT NOT NULL,
  review_count INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
