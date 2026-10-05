# e2e-sync.sh と e2e-review.sh から source する。開発用の .wrangler/state は触らない。

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
TMP=""
STATE=""
VAULT=""
PID=""
POKO=""

e2e_init() {
  TMP=$(mktemp -d)
  STATE="$TMP/state"
  VAULT="$TMP/vault"
  trap e2e_cleanup EXIT
}

e2e_cleanup() {
  if [[ -n "$PID" ]]; then
    kill "$PID" 2>/dev/null || true
    pkill -P "$PID" 2>/dev/null || true
  fi
  if command -v lsof >/dev/null 2>&1; then
    local listeners
    listeners=$(lsof -nP -iTCP:8788 -sTCP:LISTEN -t 2>/dev/null || true)
    if [[ -n "$listeners" ]]; then
      # shellcheck disable=SC2086
      kill $listeners 2>/dev/null || true
    fi
  fi
  if [[ -n "$TMP" ]]; then
    rm -rf "$TMP"
  fi
}

d1_json() {
  (
    cd "$ROOT/crates/worker"
    wrangler d1 execute neuron-pokopoko --local --persist-to "$STATE" --json --command "$1"
  )
}

d1_n() {
  d1_json "$1" | jq -r '.[0].results[0].n'
}

d1_exec() {
  (
    cd "$ROOT/crates/worker"
    wrangler d1 execute neuron-pokopoko --local --persist-to "$STATE" --command "$1"
  )
}

expect_eq() {
  local label=$1 expected=$2 actual=$3
  if [[ "$actual" != "$expected" ]]; then
    echo "$label: 期待 $expected 実際 $actual" >&2
    exit 1
  fi
  echo "$label=$actual"
}

require_substr() {
  local label=$1 haystack=$2 needle=$3
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "$label: '$needle' が無い: $haystack" >&2
    exit 1
  fi
}

e2e_prepare() {
  command -v jq >/dev/null || {
    echo "jq が PATH に無い" >&2
    exit 1
  }

  mkdir -p "$VAULT"
  cp -R "$ROOT/crates/core/tests/fixtures/vault/." "$VAULT/"
  cargo build -q -p poko
  POKO="$ROOT/target/debug/poko"

  (
    cd "$ROOT/crates/worker"
    wrangler d1 migrations apply neuron-pokopoko --local --persist-to "$STATE"
  )

  (
    cd "$ROOT/crates/worker"
    wrangler dev --port 8788 --persist-to "$STATE"
  ) >"$TMP/wrangler.log" 2>&1 &
  PID=$!

  local ready=0
  for _ in $(seq 1 180); do
    if curl -sf "$ENDPOINT/api/health" >/dev/null; then
      ready=1
      break
    fi
    if ! kill -0 "$PID" 2>/dev/null; then
      echo "wrangler dev が終了した" >&2
      tail -n 40 "$TMP/wrangler.log" >&2
      exit 1
    fi
    sleep 1
  done
  if [[ "$ready" != 1 ]]; then
    echo "health が 180 秒以内に返らなかった" >&2
    tail -n 40 "$TMP/wrangler.log" >&2
    exit 1
  fi
}
