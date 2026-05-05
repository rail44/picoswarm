# Reattach 時の画面復元 (VT parser 導入)

- **Priority:** 中

### Description

- **Summary:** 現状の reattach は 64 KB の raw byte ring buffer を再送するだけで、画面 (cursor 位置, scroll region, alt screen, attribute) は復元されない。Claude Code のような alt-screen を使う TUI に reattach すると、ノイズだけが流れる/真っ黒/壊れた表示になりがち。
- **Impact:** 日常の reattach 体験の最大の摩擦。`docs/plan.md` "Later, only if justified" でも筆頭に挙がっているが、実運用で頻繁に当たる項目。
- **Proposed Solutions:**
  1. **`vte` (alacritty) crate を採用** (中〜大, 3〜5 日): 画面状態を持つだけのライト VT parser。`docs/decision-log.md` 5 で「VT 復元が要るとなったら自然な選択」と判定済み。daemon 側に VT screen を 1 個持ち、ring buffer の代わりに「現在の画面 snapshot」を新規 subscriber に送る。トレードオフ: 依存追加 + 画面サイズ変化時の reflow 設計。
  2. **`libghostty-vt` を採用** (大, 1 週間+): SIMD パース + Ghostty 品質。ただし Zig toolchain 依存と未 release 状態 (`docs/decision-log.md` 5)。トレードオフ: build 複雑化が disqualifying と判定済み。
  3. **Refresh signal を agent に送って描き直させる** (小, 半日): `kill -SIGWINCH` で再描画を促す hack。Claude Code がこれで素直に再描画するか要検証。トレードオフ: agent ごとに挙動が違う / 失敗時に画面壊れたまま。
  4. **何もしない (現状維持)** (なし): trade-off 受容。reattach 直後に Ctrl-L 等で逃げる運用を続ける。
- **Knowledgement:**
  - `docs/plan.md` "Later" の screen restoration 項
  - `docs/decision-log.md` 5 (VT parser library landscape)
  - `src/daemon/output_session.rs:74` 現在の `VecDeque<u8>` ring
  - alacritty/vte: https://github.com/alacritty/vte
