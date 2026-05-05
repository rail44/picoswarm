# `pswarm run` での env 注入フラグ (`-e KEY=VAL`)

- **Priority:** 中

### Description

- **Summary:** `RunRequest` には `env: Vec<(String, String)>` フィールドがあり、daemon 側は `src/daemon/session.rs:56-58` で受理しているが、CLI に exposing がない (`src/client/run.rs:29` で常に `Vec::new()` を渡している)。docker 風に `-e KEY=VAL` / `--env KEY=VAL` を受け付けたい。
- **Impact:** agent ごとに API key を分けたい / `CLAUDE_*` 系の挙動切り替えをしたい / デバッグ用の `RUST_LOG` を入れたい時に、毎回 daemon 全体を再起動するか、shell の export に依存する必要がある。
- **Proposed Solutions:**
  1. **`-e KEY=VAL` 繰り返し可** (小, 1 時間): `clap` の `#[arg(short, long, value_parser = parse_kv)]` で複数受理。トレードオフ: 値内の `=` を含むケースの扱い (`split_once` で十分)。
  2. **`-e KEY=VAL` + `--env-file PATH`** (小〜中, 半日): dotenv 風に file 読み込みも対応。トレードオフ: パーサ仕様 (コメント許容?) を決める必要。
  3. **`--env-passthrough KEY`** (小, 1 時間): 値は渡さず key 名だけ指定して daemon の env から拾う (export 済み env を選択的に渡す)。トレードオフ: 上の機能と直交、組み合わせ可。
- **Knowledgement:**
  - `src/protocol.rs:25-32` `RunRequest`
  - `src/client/run.rs:25-31` 現在は env 空で送出
  - `src/daemon/session.rs:51-58` daemon 側の env 注入順 (daemon env → `PSWARM_DAEMON=1` → request env)
  - 関連 issue: #02 (`PSWARM_AGENT_ID` の自動注入と組み合わせ)
