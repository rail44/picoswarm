# attach せずに直近出力を見る (`pswarm logs` / `peek`)

- **Priority:** 中

### Description

- **Summary:** ring buffer に 64 KB の直近出力があるのに、attach しないと中身が見えない。`pswarm logs <name>` か `pswarm peek <name>` で 1 回 dump する subcommand があれば、health check / 状況確認が劇的に楽になる。`docs/protocol.md` の Out of scope に既に "Streaming logs without attaching (`peek` / `logs` subcommands)" として挙がっている。
- **Impact:** トラブルシュート時、いちいち attach → ノイズ → detach のサイクルを踏む必要がある。スクリプトからも grep できない。Issue #06 (multi-client read-only) の subset として最も需要があるユースケース。
- **Proposed Solutions:**
  1. **1-shot dump** (小, 半日): `Peek { name }` → `Stdout(backlog)` 1 frame → close。`output_session.rs` に `Snapshot` 要求を 1 個足すだけ。トレードオフ: tail-follow できない。
  2. **1-shot + `-f` follow** (小〜中, 1 日): `--follow` で attach 同等の subscribe (read-only) に切り替わる。これは issue #06 の subset。トレードオフ: 排他の扱いを #06 と整合させる必要。
  3. **時間/バイト範囲指定** (中, 1〜2 日): `--last 30s` / `--bytes 4096` 等。ring buffer は時刻情報を持たないので時間絞り込みは VT/タイムスタンプ拡張が要る。トレードオフ: 過剰スコープ。
- **Knowledgement:**
  - `docs/protocol.md` Out of scope
  - `src/daemon/output_session.rs:97-104` Subscribe 時に backlog を返すロジックは流用可能
  - 関連 issue: #06 (read-only attach)
