# エージェントライフサイクルログ (履歴の永続化)

- **Priority:** 中

### Description

- **Summary:** Registry を in-memory only にする決定 (`docs/plan.md` Resolved) は agent の "live state" については正しいが、「過去にどんな agent をどこで起動したか」「いつ exit したか」の履歴が一切残らない副作用がある。daemon を 1 回再起動するとすべて失われる。append-only な lifecycle log (JSONL) を別経路で持てば、live registry の決定を変えずに観測性を上げられる。
- **Impact:** 「昨日動かしてた worktree どこだっけ」「あの agent はいつ落ちた」を追えない。`pswarm doctor` も daemon 起動以降の情報しか出せない。複数の agent を回す日が続くと露骨に困る。
- **Proposed Solutions:**
  1. **JSONL の append-only ログ** (小〜中, 1 日): `$XDG_STATE_HOME/picoswarm/lifecycle.jsonl` にイベント (`spawned`, `attached`, `detached`, `exited`, `removed`) を 1 行ずつ書く。`pswarm history` で読み出し。トレードオフ: ローテーションは別途必要 (issue #17 の枠で兼ねる)。
  2. **既存 daemon.log にイベント行を混ぜる** (極小, 数時間): `tracing` で構造化ログを既存ログに統合。トレードオフ: 検索/解析が grep ベースになる、人間向け log と機械向け event が混ざる。
  3. **SQLite に倒す** (中〜大, 2〜3 日): `rusqlite` 追加。関係クエリが書ける反面、依存追加と「persistence は in-memory のみ」決定との緊張がある。トレードオフ: 過剰スコープの恐れ。
- **Knowledgement:**
  - `docs/plan.md` Resolved "Registry persistence: none for MVP" — 決定は live registry に対するもので、log 用途は別軸
  - `src/daemon/registry.rs` `insert` / `remove` / `prune_dead` がイベント発火点候補
  - `src/daemon/output_session.rs` `SessionEvent::Ended` の生成箇所も発火点
  - 関連 issue: #17 (log rotation)
