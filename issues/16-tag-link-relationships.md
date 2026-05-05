# tag / link / parent-child リレーション

- **Priority:** 中

### Description

- **Summary:** `CLAUDE.md` "Confirmed direction / Scope / In scope" に「parent-child / tags」が含まれているが、protocol / registry / CLI のいずれにも実装がない。`docs/plan.md` Out-of-MVP の "link / tag / parent-child relationships" として保留扱い。
- **Impact:** agent 数が増えてきた時の整理 (例: `pswarm ls --tag worktree`、`pswarm rm --tag stale`) ができない。エージェントが自分の "親" を発見する経路もない。CLAUDE.md の direction に明記されている以上、長期的には実装が前提。
- **Proposed Solutions:**
  1. **tag だけ先行** (小〜中, 1〜2 日): `pswarm run --tag foo --tag bar`、`AgentSummary.tags: Vec<String>`。`pswarm ls --tag` で絞り込み、`pswarm rm --tag` で一括削除。トレードオフ: parent-child は別物として後送り。
  2. **parent-child + tag を同時** (中, 3〜5 日): `--parent <name|id|self>` で関係を張り、tree 表示 `pswarm ls --tree`。`PSWARM_AGENT_ID` (#02) と組み合わせて agent 自身が child を spawn する流れも自然になる。トレードオフ: モデル決定 (関係は 1:N か N:M か)、wire format 拡張。
  3. **link (任意のラベル付きリレーション)** (中, 上記 +1〜2 日): `pswarm link <a> <b> --as upstream` のような汎用エッジ。トレードオフ: 過剰汎用化。
- **Knowledgement:**
  - `CLAUDE.md` "In scope" 項
  - `docs/plan.md` Out-of-MVP, Next 候補
  - `src/daemon/registry.rs:23-35` `AgentEntry` に tags / parent_id を足す形
  - 関連 issue: #02 (self-invocation との組み合わせ)
