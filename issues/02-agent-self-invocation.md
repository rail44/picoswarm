# エージェント自己呼び出し (env 注入と send-to-self ガード)

- **Priority:** 高

### Description

- **Summary:** `CLAUDE.md` "Decisions that must not drift" には「Agents must be able to invoke the CLI on themselves (single binary, callable from a subshell)」と明記されている。これを成立させるには (a) 起動した PTY 内に `PSWARM_AGENT_ID` / `PSWARM_AGENT_NAME` を注入する、(b) `pswarm send self ...` のような自己宛操作で無限ループを防ぐガードを置く、の 2 ピースが要る。`docs/plan.md` の "PTY env policy" 決定事項にも同じ env 注入が予告されているが、`src/daemon/session.rs:55` では `PSWARM_DAEMON=1` だけしか入れていない。
- **Impact:** 「CLI ファースト」を picoswarm の差別化として確定した (`docs/decision-log.md` 4) 以上、エージェントが自分自身を pswarm から見つけられないと差別化が成立しない。`send` (#03) や tag/link (#16) の前段でもある。
- **Proposed Solutions:**
  1. **env 注入だけ先行** (小, 半日): `session.rs` で `PSWARM_AGENT_ID` / `PSWARM_AGENT_NAME` を `cmd.env` に追加。`pswarm` 内に `agent_id_from_env()` ヘルパを置き、各 client subcommand が必要に応じて拾えるようにする。ガードは `send` 実装時に追加。トレードオフ: 自己呼び出しの安全性は別 issue 任せ。
  2. **env + 専用 `self` キーワード** (中, 1 日): 上記に加え、`pswarm <verb> self` を「環境変数の id を解決」として書き、attach / send / cwd 等で対応。トレードオフ: subcommand 横断の規約が増える。
  3. **env + global self ガード middleware** (中, 1〜2 日): connection layer で「`PSWARM_AGENT_ID` と target が同じなら拒否」を中央で当てる。トレードオフ: いまの thin な client 実装に layer を一枚足す必要。
- **Knowledgement:**
  - `CLAUDE.md` "Decisions that must not drift" の self-invocation 項
  - `docs/plan.md` Resolved "PTY env policy" (env 注入は予告済み)
  - `src/daemon/session.rs:51-58` 現在の env 注入箇所
  - 関連 issue: #03 (send), #16 (tag/link)
