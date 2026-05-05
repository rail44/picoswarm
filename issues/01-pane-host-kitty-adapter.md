# PaneHost (kitty adapter) の実装

- **Priority:** 高

### Description

- **Summary:** `CLAUDE.md` で adapter (PaneHost) は first-class な抽象として明記されており、kitty が最初の対応 adapter として宣言されている。しかし現状コードベースに `adapter/` モジュールも `PaneHost` trait も存在しない。`pswarm run` / `pswarm attach` のたびにユーザーが手動で kitty タブを開く必要がある状態。
- **Impact:** 「並列に複数エージェントを動かす」という picoswarm の主目的において、ウィンドウ配置の手間が運用の最大の摩擦になる。`docs/plan.md` の Next 候補でも「手動タブ管理が面倒になったら」着手のトリガーとされており、すでに使い始めて摩擦を感じる段階に来ている。
- **Proposed Solutions:**
  1. **最小実装: kitty 専用 + 環境変数で gating** (小〜中, 1〜2 日): `src/adapter/mod.rs` に `PaneHost` trait と `Noop` 実装、`src/adapter/kitty.rs` に kitty 実装を置く。`KITTY_LISTEN_ON` が立っていれば kitty を選び、`pswarm run` で `kitty @ launch --type=tab pswarm attach <name>` を呼ぶ。trait は `open_pane(name) -> Result<()>` だけで十分にスタートできる。トレードオフ: trait が小さすぎて wezterm 等を後付けする時に signature を変える可能性。
  2. **trait を最初から複数 impl 想定で設計** (中, 2〜3 日): `open` / `focus` / `close` の 3 メソッドを最初から trait に乗せ、`Noop` / `Kitty` の 2 つを実装。コンフィグで明示選択 (`config.toml` の `[adapter] kind = "kitty"`)。トレードオフ: config 機能 (issue #15) に依存する。
  3. **CLI フラグで都度指定** (小, 半日): `pswarm run --pane kitty:tab` のように invocation ごとに渡す。adapter 抽象を作らず最低限。トレードオフ: 抽象が育たないので将来再設計コスト。
- **Knowledgement:**
  - `CLAUDE.md` "adapter (PaneHost)" / "Decisions that must not drift" の adapter 関連
  - `docs/plan.md` Next 候補の kitty adapter 項
  - kitty remote control: https://sw.kovidgoyal.net/kitty/remote-control/
  - `src/client/doctor.rs:13` で既に `KITTY_LISTEN_ON` を読んでいる (検出ロジック流用可)
  - 関連 issue: #15 (config), #21 (adapter 拡張時の他端末対応)
