# `AgentStatus::Idle` / `Unknown` を実装するか削除する

- **Priority:** 中

### Description

- **Summary:** `AgentStatus` enum には `Running` / `Idle` / `Dead` / `Unknown` の 4 variant があるが、`src/daemon/registry.rs:42-46` では `Running` か `Dead` しか出力されない。`Idle` / `Unknown` は dead code。
- **Impact:** 「何を意味する状態か」が曖昧で、CLI / JSON 出力の消費側 (補完 / monitoring 等) が混乱する。protocol に出ている以上 wire の version 議論にも引っ張られる。
- **Proposed Solutions:**
  1. **削除する** (小, 30 分): wire format に出ている enum を削るので protocol version bump 必要。`docs/protocol.md` (issue #04) と一緒に。トレードオフ: 将来 Idle 概念が出てきたら再追加。
  2. **`Idle` を「attach がいない」に割り当てる** (小, 1〜2 時間): `entry.attached` を見て Running / Idle を出し分け。`Unknown` は引き続き未使用なら削除。トレードオフ: 「running だが attach なし」が "Idle" と表現できるかは要合意。
  3. **`Idle` を「PTY に直近出力がない」に割り当てる** (中, 1 日): `output_session` に最終出力時刻を持たせ、N 秒経過で Idle 扱い。トレードオフ: 閾値の選定、agent 側のチャタリングで誤判定する。
- **Knowledgement:**
  - `src/protocol.rs:34-40` enum 定義
  - `src/daemon/registry.rs:38-50` `summary()` で参照されているのは Running / Dead のみ
  - 関連 issue: #04 (protocol doc), #06 (read-only attach)
