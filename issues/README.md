# Issues

`docs/plan.md` (方向性 / 大枠) と併存する、より細かい実行単位での未実装項目・改善ポイント一覧。実施判断は別途。

> 注: 重複は許容している (plan.md と一部内容が被る)。issues は粒度を細かくして triage しやすくするのが目的。

## 一覧

### 高 (foundational / 差別化に直結)

- ~~01 PaneHost (kitty adapter) の実装~~ — out of scope に変更 (composition + `docs/integration.md`、decision-log #13)
- [02 エージェント自己呼び出し (env 注入と send-to-self ガード)](02-agent-self-invocation.md) — 延期 (`self` を要する後続機能の着手時に再検討)
- ~~03 `pswarm send` サブコマンド~~ — 解決済み (テキスト + stdin + 自動改行、protocol bump 4→5)
- ~~04 `docs/protocol.md` の最新化~~ — 解決済み (commit にて protocol.md を v4 に追従)

### 中 (機能ギャップ / 既存機能の穴)

- ~~05 エージェントライフサイクルログ~~ — 削除 (recovery が out of scope な以上の concrete consumer なし、decision-log #14 に再検討時のメモ)
- [06 複数クライアントの同時 attach (read-only observers)](06-multi-client-readonly-attach.md) — 延期 (#02 self-invocation 着手 or peek 運用が辛くなった時)
- ~~07 Reattach 時の画面復元 (VT parser 導入)~~ — 削除 (issue 本文の "最大の摩擦" 主張は未検証の推測。実害が顕在化した時に新規 issue として書き直す)
- ~~08 attach せずに直近出力を見る~~ — 解決済み (`pswarm view <name>` で 1-shot snapshot、protocol bump 5→6)
- ~~09 `pswarm rm` の段階的終了 (SIGTERM → SIGKILL)~~ — 解決済み (graceful 1s grace 実装、`--force` で SIGTERM スキップ)
- ~~10 `pswarm ls` の人間向け出力 enrichment~~ — 不要として却下
- ~~11 `pswarm run` での env 注入~~ — 解決済み (client env を自動継承する方式に再設計、`-e` フラグは不採用)
- ~~12 `AgentStatus::Idle` / `Unknown` を実装するか削除する~~ — 解決済み (削除、protocol bump 3→4)
- ~~13 Bash / zsh の補完スクリプト~~ — 解決済み (clap_complete `unstable-dynamic` 採用、bash/zsh/fish/elvish/powershell すべてで動的補完)
- ~~14 全 client detach 時の PTY サイズ方針~~ — 解決済み (keep last を採用、docs に明記)
- [15 設定ファイル (`config.toml`) の導入](15-config-file-toml.md) — 延期 (config を要求する後続 issue の着手時に再検討)
- [16 tag / link / parent-child リレーション](16-tag-link-relationships.md) — 延期 (agent 数 5+ や `--parent self` 需要が出た時に着手)
- ~~17 Daemon ログのローテーション~~ — 解決済み (tracing-appender で daily rotation + 7 日保持、crash log を分離)
- ~~21 Daemon ハードクラッシュ時の orphan agent 対策~~ — 解決済み (pty-process に乗換 + `pre_exec` で `PR_SET_PDEATHSIG`)
- [22 Daemon 再起動時の attach 体験改善](22-attach-on-daemon-restart-ux.md)

### 低 (要件が固まってから)

- [18 TUI ビュー (`pswarm tui`) — 補完ビューとして](18-tui-secondary-view.md) — 延期 (agent 数 10+ や watch 需要が出た時に着手)
- ~~19 クロスホスト対応~~ — 不要として却下
- ~~20 pswarm を MCP server としてエージェントに露出~~ — 不要として却下 (Bash tool 経由で十分、MCP 自体のトレンドも踊り場)

## テンプレート

新規 issue 追加時は以下のテンプレートに従う:

```markdown
# <タイトル>

- **Priority:** <高 | 中 | 低>

### Description

- **Summary:** 問題の概要を簡潔に
- **Impact:** どのような影響があるか / 何が解決されるか
- **Proposed Solutions:**
  1. **<アプローチ名>** (規模/難易度): 内容とトレードオフ
  2. **<アプローチ名>** ...
- **Knowledgement:**
  - 関連コード `path/to/file.rs:LL`
  - 関連ドキュメント `docs/...`
  - 外部リンク
  - 関連 issue: #NN
```
