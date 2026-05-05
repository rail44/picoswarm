# TUI ビュー (`pswarm tui`) — 補完ビューとして

- **Priority:** 低

### Description

- **Summary:** `CLAUDE.md` "Decisions that must not drift" で「primary UX は CLI のまま、TUI は complementary view として後で足すのは可」とされている。`docs/plan.md` Next 候補にも「CLI list view が不十分になったら」着手と書かれている。
- **Impact:** CLI で済んでいる現状はすぐの困りごとは小さい。10+ agent を同時に並べて状態監視する段階で価値が出る。
- **Proposed Solutions:**
  1. **`ratatui` の最小ダッシュボード** (中〜大, 3〜5 日): `pswarm ls --watch` 相当 + 各 agent の最後の数行 preview。read-only attach (#06) と組み合わせると tail 機能になる。トレードオフ: 依存大、UX デザイン工数。
  2. **`pswarm watch` だけ実装** (小〜中, 1〜2 日): TUI までいかず、`pswarm ls` を 1 Hz で reprint する watch モード。トレードオフ: 最終形ではないが、当面の痒み解消には十分。
  3. **やらない** (なし): CLI で十分という決定を維持。トレードオフ: 将来不要であれば最適。
- **Knowledgement:**
  - `CLAUDE.md` "Decisions that must not drift" — TUI を primary にしないこと
  - `docs/decision-log.md` 4 (ccmanager 評価) — TUI-first を明確に拒否した経緯
  - 関連 issue: #06, #08
