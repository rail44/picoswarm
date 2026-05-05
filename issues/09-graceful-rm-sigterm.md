# `pswarm rm` の段階的終了 (SIGTERM → SIGKILL)

- **Priority:** 中

### Description

- **Summary:** `docs/protocol.md` には "the daemon sends `SIGTERM` then `SIGKILL` after a short grace period" と書かれているが、実装は `src/daemon/registry.rs:113-121` で portable-pty の `child.kill()` を直接呼んでおり、Unix では SIGKILL 1 発。`--force` フラグも no-op であることはコメントで明言済み。
- **Impact:** Claude Code のような agent が graceful shutdown 中に書き出すべき状態 (会話履歴の最終 flush 等) を skip してしまう可能性。仕様 (`protocol.md`) と実装が乖離している点も保守上のノイズ。
- **Proposed Solutions:**
  1. **`nix` で SIGTERM → 1s 待機 → SIGKILL** (小, 半日): `child.process_id()` で pid を取得し `kill(pid, SIGTERM)`、`try_wait()` を 100 ms 間隔で polling、1s 経ったら SIGKILL。`--force` は SIGTERM をスキップ。`nix` 依存が増えるが小さい。トレードオフ: polling より SIGCHLD ベースの方が綺麗だが MVP 過剰。
  2. **portable-pty の API だけで済ませる** (小, 半日): `Child` trait に signal 系メソッドがないので、結局 (1) と同じ pid + nix の経路。トレードオフ: なし。
  3. **graceful 期間を CLI で可変に** (中, 1 日): `pswarm rm --grace 5s`。トレードオフ: フラグが増える、デフォルト値の選定。
- **Knowledgement:**
  - `docs/protocol.md` rm セクション
  - `src/daemon/registry.rs:107-121` 現在の実装とコメント
  - portable-pty `Child` trait
  - 関連 issue: #04 (protocol doc を再生成する時に挙動を合わせる)
