# 複数クライアントの同時 attach (read-only observers)

- **Priority:** 中
- **Status:** 延期 — streaming 用途は #02 (self-invocation) が動いた時点で同時に価値が立つ。それまで peek (#08) で 80% カバー。

### 着手トリガー

下記いずれかが立ち上がったタイミングで再検討:

- #02 (self-invocation) が動いて agent 間の参照関係が表現できた時 (= "agent が別 agent をライブで観察" の前提が揃う)
- peek (#08) を loop で叩いて代替している運用が辛くなった時
- #18 (TUI dashboard) を作る時、複数同時 subscribe を必要とする backing として

### Description

- **Summary:** 現状 attach はエージェントごとに 1 client 排他 (`AlreadyAttached` エラー)。`docs/plan.md` Out-of-MVP の "Multiple-client concurrent attach (read-only observers)" として明示的に保留されている。`pswarm send` (#03) や別エージェントからの観察ユースケースが効くようになると価値が出る。
- **Impact:** 「人間が attach 中に別の agent (or 別端末) から覗きたい」「CI からヘルスチェック」が現状不可能。`docs/decision-log.md` 4 で picoswarm の差別化として強調された「agent が他 agent を観察」が動かない。
- **Proposed Solutions:**
  1. **read-only secondary attach** (中, 2〜3 日): protocol に `AttachReadOnly` を追加し、複数 subscribe を許容。`SessionInbox::subscribe` は既に複数 subscriber を受け付けられるので、daemon 側は `attached` フラグを「writer の排他」だけに格下げ。stdin/Resize は writer 側のみ受理。トレードオフ: 「writer になれるのは誰」のロジックが要る (先着順で十分か、`--steal` か)。
  2. **read-write も含めた一斉許可** (中〜大, 3〜5 日): 全 attach が write 可能。同時 stdin 衝突は PTY が解決 (≒ 競合) する。トレードオフ: 安全に見えるが、debug が難しい。
  3. **`pswarm peek` だけ別ルートで実装** (小, 1 日): attach せずに ring buffer の現在内容を 1 回出力するだけ。これは issue #08 と同義になるので、複数 attach のフルセットは将来へ送る。トレードオフ: 「観察」は満たせるが「介入の橋渡し」 (read+write) は別問題のまま。
- **Knowledgement:**
  - `docs/plan.md` Out-of-MVP の該当項
  - `src/daemon/server.rs:295-309` 現在の排他ロジック (`compare_exchange` で 1 attach に制限)
  - `src/daemon/output_session.rs:75` `subscribers: Vec<...>` — 複数前提の構造はすでにある
  - 関連 issue: #03, #08, #14
