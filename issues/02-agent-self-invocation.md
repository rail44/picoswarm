# エージェント自己呼び出し (env 注入と send-to-self ガード)

- **Priority:** 高
- **Status:** 延期 — `self` 概念が必要な機能 (#16 tag/link、#03 の send-to-self ガード等) が動き始めるタイミングまで保留。

### Description

- **Summary:** `CLAUDE.md` "Decisions that must not drift" には「Agents must be able to invoke the CLI on themselves (single binary, callable from a subshell)」と明記されている。これを成立させるには (a) 起動した PTY 内にエージェントの id を注入する、(b) `pswarm <verb> self` の `self` 解決と send-to-self ガードを置く、の 2 ピースが要る。`docs/plan.md` の "PTY env policy" 決定事項にも env 注入が予告されているが、`src/daemon/session.rs:55` では `PSWARM_DAEMON=1` だけしか入れていない。
- **Impact:** 「CLI ファースト」を picoswarm の差別化として確定した (`docs/decision-log.md` 4) 以上、エージェントが自分自身を pswarm から見つけられないと差別化が完成しない。ただし **#03 (send) は self 識別なしでも成立** (`pswarm send <他人の名前>` で十分) なので、`self` キーワードや send-to-self ガードを実装するタイミングまで延期可能。

### 識別方式の比較 (検討済み)

| 方式 | 仕組み | Pros | Cons |
|---|---|---|---|
| **A. env 注入** | spawn 時に `PSWARM_AGENT_ID` / `_NAME` を inject | 5 行で済む / Unix idiom (TMUX, KITTY_LISTEN_ON 等) / subshell・nohup・setsid 越えに耐える | 子プロセス全部に漏れる / agent 内で unset/spoof 可能 |
| **B. getpeercred + ppid 遡り** | daemon 側で接続元 pid を取得し `/proc/<pid>/status` の PPid を遡って registry の agent pid と一致させる | env 汚染ゼロ / spoof 不可 | 30〜50 行 / `nohup` で session 切れた場合のロバスト性微妙 / pid 再利用レース (実害ほぼ無) |
| C. cwd 一致 | 接続元の cwd と agent の cwd を突合 | 簡単 | 同一 worktree で複数 agent の場合 ambiguous → 却下 |
| D. controlling tty | 接続元の tty から agent の PTY を逆引き | 自然 | agent の child process も同じ tty を持つので結局 env と同じ間接性 / 実装は重い |
| E. cwd にマーカファイル | `.pswarm-agent-id` を撒く | env 不使用 | worktree が汚れる → 却下 |
| F. socket fd 渡し | spawn 時に open socket fd を継承 | spoof 不可 | subshell に継承されない (= env より弱い) → 却下 |

現実的に残るのは **A (env)** と **B (peercred + ppid 遡り)**。脅威モデルが「単一ユーザのローカルマシン」である以上、spoof 耐性は弱い要件。env で十分というのが現時点の見立て。

### Proposed Solutions

- **(着手時の第一候補) 方式 A: env 注入だけ先行** (小, 半日): `session.rs` で `PSWARM_AGENT_ID` / `PSWARM_AGENT_NAME` を `cmd.env` に追加。`pswarm` 内に `agent_id_from_env()` ヘルパを置き、各 client subcommand が必要に応じて拾えるようにする。ガードは send 実装時に追加。トレードオフ: 自己呼び出しの安全性は別 issue 任せ。
- env + 専用 `self` キーワード (中, 1 日): 上記に加え `pswarm <verb> self` を「env の id を解決」として書く。attach / send / cwd 等で対応。トレードオフ: subcommand 横断の規約が増える。
- env + global self ガード middleware (中, 1〜2 日): connection layer で「`PSWARM_AGENT_ID` と target が同じなら拒否」を中央で当てる。トレードオフ: いまの thin な client 実装に layer を一枚足す必要。
- 方式 B (peercred + ppid 遡り) (中〜大, 2〜3 日): env 汚染を避けたい場合の代替。脅威モデル的に正当化しづらいので、env で困った時の検討対象として保留。

### 着手トリガー

下記いずれかが立ち上がったタイミングで再検討:

- #16 (tag/link) で `--parent self` が必要になった時
- #03 (send) で send-to-self ガードを入れたくなった時 (= 無限ループ事故が見えてきた時)
- ライフサイクルログ (#05) に「誰が誰を操作したか」を残したくなった時

### Knowledgement

- `CLAUDE.md` "Decisions that must not drift" の self-invocation 項
- `docs/plan.md` Resolved "PTY env policy" (env 注入は予告済み)
- `src/daemon/session.rs:51-58` 現在の env 注入箇所
- 関連 issue: #03 (send は単独で成立), #16 (tag/link), #05 (lifecycle log)
