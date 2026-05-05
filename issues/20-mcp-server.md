# pswarm を MCP server としてエージェントに露出

- **Priority:** 低

### Description

- **Summary:** `docs/plan.md` "Later" の最後の項目。pswarm の機能 (run / ls / attach / send) を MCP tool としてエージェントに直接渡す。CLI 自己呼び出し (#02) とは別の経路 (構造化された tool call) でエージェントが pswarm を使えるようになる。
- **Impact:** Claude Code 等が tool として直接呼べるようになると、shell 経由より型安全で argument hint も効くようになる。一方、CLI の自己呼び出しが揃っていれば多くのケースは賄える。
- **Proposed Solutions:**
  1. **公式 mcp-rust SDK で stdio MCP server を実装** (中〜大, 3〜5 日): `pswarm mcp` で stdio MCP server を立て、各 subcommand を tool として公開。トレードオフ: MCP spec 追従コスト、依存追加、認可モデル設計。
  2. **既存 CLI を `mcp-shell` 系で wrap** (小, 半日): 第三者の汎用 wrapper で CLI を MCP 化。トレードオフ: 引数 schema が貧弱、自前で書くより制約が多い。
  3. **やらない / 後送り** (なし): CLI 自己呼び出し (#02) で先に体験を作って、足りなさが見えてから着手。トレードオフ: 将来手戻り (CLI 設計と MCP schema 設計が両立しないと辛い)。
- **Knowledgement:**
  - `docs/plan.md` "Later" の該当項
  - 関連 issue: #02 (self-invocation)
  - MCP spec: https://modelcontextprotocol.io/
