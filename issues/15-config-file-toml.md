# 設定ファイル (`config.toml`) の導入

- **Priority:** 中
- **Status:** 延期 — config を実際に要求する後続 issue が立ち上がるまで保留。

### 着手トリガー

下記いずれかが立ち上がったタイミングで再検討:

- #01 (PaneHost / kitty adapter) で adapter 選択や per-agent デフォルトを config で持ちたくなった時
- #20 (MCP server) で MCP 接続情報を持つ必要が出た時
- 任意の機能で「CLI フラグでも env でも収まらない設定」が出た時

「空のスキーマを先に作る」より「最初に具体的な需要と一緒に設計する」方がスキーマを誤りにくいと判断。config 層自体は起動時 1 回読みで侵襲的ではないので、defer のコストは低い。

### Description

- **Summary:** `docs/plan.md` Open decisions の 2 番目に `$XDG_CONFIG_HOME/picoswarm/config.toml` の予定だけ書かれており、実装は未着手。adapter 選択 (issue #01)、preferred PTY size (#14)、env passthrough whitelist (#11) など、CLI フラグに乗せ切れない設定が出てきた段階で必要になる。
- **Impact:** 単体では即時の困りごとは小さいが、他の issue が config を要求し始めると blocker になる。先送りすればするほど後で `pswarm` 全体に config を貫通させる作業が大きくなる。
- **Proposed Solutions:**
  1. **`toml` crate + `serde` で最小スキーマ** (小, 半日): 起動時に 1 回読んで `Arc<Config>` に流す。空ファイル / 不在を許容、デフォルト値で穴埋め。トレードオフ: なし、必要な後続を個別に追加。
  2. **`figment` で env / file / CLI のレイヤ統合** (中, 1〜2 日): 設定の出所を統一できるが、依存と複雑度が上がる。トレードオフ: スコープ過剰の恐れ。
  3. **やらない (CLI フラグだけで通す)** (なし): adapter 選択など複数値に拡張する時点で破綻する。トレードオフ: 後で痛い。
- **Knowledgement:**
  - `docs/plan.md` Open decisions の 2
  - `src/paths.rs` XDG path helper (config_path も追加できる)
  - 関連 issue: #01, #11, #14
