# 全 client detach 時の PTY サイズ方針 (Open decision の解決)

- **Priority:** 中

### Description

- **Summary:** `docs/plan.md` Open decisions の 1 番目: 「最後の client が detach した時、daemon は最後の size を保持するか、80×24 に reset するか」。working assumption は "keep last" だが、未確定で実装も特に何もしていない (= 結果的に keep last)。
- **Impact:** reattach 時に terminal size が変わっていると、TUI agent が画面崩れを起こす。決め切らずに放置されると issue #07 (screen restoration) との合わせ技で問題が出る。
- **Proposed Solutions:**
  1. **明示的に "keep last" として close** (極小, 1 時間): `docs/plan.md` と protocol doc に明記、attach 時に必ず client size で resize する現状ロジックで十分。トレードオフ: なし、決定の確定だけ。
  2. **detach 時に 80×24 へ reset** (小, 半日): detach handler で `apply_resize` を呼ぶ。トレードオフ: keep last より trace が増える、reattach 直前の何回か無駄な resize。
  3. **agent ごとに sticky size をコンフィグ可能に** (中, 1〜2 日): config.toml で `[agent.<name>] preferred_size = "120x40"` 等。トレードオフ: config 機構 (issue #15) 依存、過剰機能。
- **Knowledgement:**
  - `docs/plan.md` Open design decisions の 1
  - `src/daemon/server.rs:405-414` `apply_resize` (attach 時に呼んでいる)
  - 関連 issue: #07, #15
