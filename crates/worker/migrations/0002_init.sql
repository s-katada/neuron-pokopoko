-- スパイクの仮テーブルを片付ける
DROP TABLE IF EXISTS spike_entries;

-- ノート。deleted_at は論理削除(Unix 秒)。NULL は存続
CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  path TEXT NOT NULL,
  title TEXT NOT NULL,
  major TEXT NOT NULL,
  middle TEXT NOT NULL,
  minor TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  deleted_at INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

-- カード。due_at NULL は未学習で出題対象外。retired_at は論理削除
CREATE TABLE cards (
  stable_key TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id),
  level TEXT NOT NULL CHECK (level IN ('beginner','intermediate','advanced','integration')),
  question TEXT NOT NULL,
  answer TEXT NOT NULL,
  rubric TEXT,
  refs TEXT NOT NULL DEFAULT '[]',
  fsrs_state TEXT NOT NULL DEFAULT 'new' CHECK (fsrs_state IN ('new','review')),
  stability REAL,
  difficulty REAL,
  due_at INTEGER,
  last_reviewed_at INTEGER,
  reps INTEGER NOT NULL DEFAULT 0,
  lapses INTEGER NOT NULL DEFAULT 0,
  retired_at INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
-- 出題対象(retired でない)の due 検索
CREATE INDEX cards_due ON cards(due_at) WHERE retired_at IS NULL;
CREATE INDEX cards_note ON cards(note_id);

-- 復習ログ。消さない
CREATE TABLE reviews (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  card_key TEXT NOT NULL REFERENCES cards(stable_key),
  rating TEXT NOT NULL CHECK (rating IN ('again','hard','good','easy')),
  reviewed_at INTEGER NOT NULL,
  interval_days REAL NOT NULL,
  stability REAL NOT NULL,
  difficulty REAL NOT NULL
);
CREATE INDEX reviews_card ON reviews(card_key);
