# `pswarm send` サブコマンド

- **Priority:** 高

### Description

- **Summary:** 任意のエージェントに対して、attach せずに stdin (テキスト / キー / シグナル) を送る subcommand。`docs/protocol.md` の "Out of scope (for this protocol version)" にも `send` が将来項目として並んでおり、`docs/plan.md` の Next 候補でも「a clear story for keys vs. text vs. signals」と保留扱い。
- **Impact:** 「他のエージェントから別エージェントを操作する」「スクリプトから一斉指示を出す」「観察 + 介入のうち介入だけ」など、ヘッドレス運用の主要パターンが現状すべて attach 経由になっておりスクリプト化できない。
- **Proposed Solutions:**
  1. **テキスト + 改行のみの最小版** (小, 半日): `pswarm send <name> <text>` で `text + "\n"` を PTY に書く。protocol に `Send { name, payload: Vec<u8> }` を 1 variant 追加。Daemon 側は `entry.writer.lock()` に書くだけで済む。トレードオフ: Ctrl-C などの制御キーは未対応。
  2. **テキスト / キー / シグナルの 3 モード** (中, 1〜2 日): `--text`, `--keys "C-c"` (named keys), `--signal SIGINT` のフラグで分岐。`--keys` はパーサ実装が要る。トレードオフ: パーサのスコープが広がる (issue #07 の VT parser とは別問題)。
  3. **stdin pipe モード追加** (中, 上記 +半日): `echo foo | pswarm send <name> --stdin`。スクリプトから流し込み用。トレードオフ: 改行の扱い ("send-then-newline" vs "send-as-is") を決める必要。
- **Knowledgement:**
  - `docs/protocol.md` "Out of scope" の send 項
  - `docs/plan.md` Next 候補の send 項
  - `src/daemon/registry.rs:31` `AgentEntry.writer` は既に PTY への書き込み口を露出済み
  - 関連 issue: #02 (self-invocation 経由の send-to-self ガード)
