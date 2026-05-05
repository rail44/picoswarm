# TUI ビュー (`pswarm tui`) — 補完ビューとして

- **Priority:** 低
- **Status:** 延期 — CLI が十分に効いている間は着手しない。

### 着手トリガー

下記いずれかが立ち上がったタイミングで再検討:

- agent 数が常時 10+ になり `pswarm ls` の出力が辛くなった時 → option 1 (ratatui ダッシュボード)
- 「ls を 1 Hz で再表示したい」のような軽い watch 需要が出た時 → option 2 (`pswarm watch` だけ先行)

`CLAUDE.md` の "Decisions that must not drift" に「TUI を primary にしない」がある以上、TUI は常に CLI の補完。現状 CLI で困っていない以上、着手の判断は実需要次第。

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
