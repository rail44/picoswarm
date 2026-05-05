# クロスホスト対応 (別マシンの agent を見る)

- **Priority:** 低

### Description

- **Summary:** `docs/plan.md` "Later, only if justified"。現状 daemon は Unix socket、ローカル単一ユーザー前提。別マシンの daemon に attach / send したいユースケースは概念的にあり得るが、優先度は低。
- **Impact:** リモート開発機 + 手元マシンを併用するワークフローでは欲しくなる。なくても困らない人が大多数。
- **Proposed Solutions:**
  1. **`ssh -L` で socket forward を README に書く** (極小, 1 時間): 機能追加なし、運用パターンの提示のみ。トレードオフ: pswarm CLI からは reach できないが大半のケースで足りる。
  2. **`pswarm --remote user@host` で SSH multiplex** (大, 1〜2 週間): pswarm 自体が SSH 経由で remote daemon を呼ぶ。認証、connection multiplexing、wire format バイナリ互換性 (両端で version 一致) が要る。トレードオフ: 大スコープ、threat model 検討要。
  3. **TCP listener + TLS / mTLS** (大, 2〜3 週間): 完全分散。トレードオフ: スコープ大幅超過、CLAUDE.md "Decisions that must not drift" の境界を再議論する必要。
- **Knowledgement:**
  - `docs/plan.md` "Later" の cross-host 項
  - `docs/protocol.md` "Out of scope" の Authenticated multi-user access 項
