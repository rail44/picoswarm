# `docs/protocol.md` の最新化 (バージョン / 新規 variant 追加)

- **Priority:** 高

### Description

- **Summary:** `docs/protocol.md` の冒頭が `Protocol version: **1**` のままで、実際は `src/protocol.rs:13` で `PROTOCOL_VERSION = 3`。`Status` / `Shutdown` / `Clean` / `GetCwd` (request 側) と `Status` / `Cleaned` / `AgentCwd` (response 側) の 7 variant が未記載。"Failure modes" 表の「daemon 再起動時に `Running` を `Dead` に reconcile する」記述も、registry が in-memory only に変更された時点で不正確 (起動時に reconcile するものが存在しない)。
- **Impact:** 唯一の wire spec ドキュメントが信用できない状態。新規参加者 (将来のコントリビュータも、自分自身も) が誤った前提でクライアントを書く / プロトコル変更時の差分判断を誤るリスク。doc を信用できないと結局コードを読むことになり、protocol.md の存在価値が下がる。
- **Proposed Solutions:**
  1. **コードに合わせて手で書き直す** (小, 1〜2 時間): version, message variants, "Per-command flows" の `clean` / `cwd` / `daemon stop` セクション追加, "Failure modes" の旧 reconciliation 行を削除。トレードオフ: 次の wire 変更でまた drift する。
  2. **`protocol.rs` から自動生成** (中, 半日〜1 日): `schemars` / `ts-rs` 系で型を JSON schema として吐き、CI で再生成して diff チェック。トレードオフ: フロー図など表現力が落ちる、依存追加。
  3. **手書き + CI で版番号一致を assert** (極小, 1 時間): `just check` で `grep -q "Protocol version: \*\*${PROTOCOL_VERSION}\*\*"` を当てるだけ。drift の早期検知のみ。トレードオフ: variant 追加忘れは検知できない。
- **Knowledgement:**
  - `docs/protocol.md` 全体
  - `src/protocol.rs:13` 現在のバージョン
  - `src/protocol.rs:62-92` 現在の `ClientToDaemon` 全 variant
  - `src/protocol.rs:94-134` 現在の `DaemonToClient` 全 variant
