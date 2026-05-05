# `pswarm ls` の人間向け出力に cwd / pid / uptime 等を追加

- **Priority:** 中

### Description

- **Summary:** 現状の人間向け `ls` は `<name>\t<status>\t<id>` の 3 列のみ (`src/client/ls.rs:30`)。JSON では cwd / created_at が出ているが、人間向けでは出ない。日常使いで欲しい情報 (どのディレクトリで動いているか、何時間動いているか、PID は何か) が抜けている。
- **Impact:** triage 時 (どの agent が暴走中? いつ起動した? どの worktree?) に毎回 `ps`, `pswarm cwd`, JSON parse を別々に叩く必要がある。
- **Proposed Solutions:**
  1. **デフォルト列を増やす** (小, 1〜2 時間): `<name>\t<status>\t<uptime>\t<cwd>\t<id>` に。長さは tab + 切り詰めで対処。トレードオフ: 端末幅で破綻する。
  2. **`-l` (long) フラグ追加** (小, 半日): デフォルトはそのまま、`pswarm ls -l` で詳細列を出す。pid/uptime/cwd は protocol に追加 (`AgentSummary` を拡張) or daemon-side で都度取得。トレードオフ: protocol 拡張なら version bump が要る。
  3. **テンプレート指定** (中, 1 日): `pswarm ls --format '{{.Name}} {{.Cwd}}'` (docker 風)。トレードオフ: 過剰スコープ、テンプレ言語の選定。
- **Knowledgement:**
  - `src/client/ls.rs:9-34` 現在の表示ロジック
  - `src/protocol.rs:42-49` `AgentSummary` (cwd / created_at は既に持っている)
  - `src/daemon/registry.rs:173` `Registry::pid_of` で PID は取れる
