# プロジェクト指針

複数の Claude Code および類似の CLI コーディングエージェントを並列で動かすための、軽量な registry + lifecycle 管理 CLI/TUI を Rust で作る。

このファイルは設計判断の前提と方針を凍結したもの。実装中にこの方針から外れる選択をする場合は、必ず人間に確認すること。

---

## 1. 解こうとしている問題

複数の Claude Code セッションを同時に動かしたい (worktree に分けて別ブランチで作業させる、あるいは同じリポジトリで違うタスクをやらせる) という需要があり、既存の解決策が以下の問題を抱えている。

- 多くは tmux 固定で、kitty / WezTerm / Zellij / Ghostty / VS Code 統合ターミナル / 素朴なターミナル単独使用 のユーザに「tmux を入れろ」と要求してしまう
- 永続化機構 (PTY を持続させる仕組み) が tmux 一択になっていて、shpool や abduco や OS の機能 (systemd-run --pty、launchd) を選べない
- 画面分割を内蔵することで責務が膨らみ、ユーザー側の既存ワークフロー (各 multiplexer の流儀) と衝突する
- agent から自分自身を操作する CLI が用意されていないか、用意されていても tmux 前提

このプロジェクトは **「永続化と画面分割を外部に任せ、agent 概念のレイヤーだけを担当する純粋な層」** として、上記を解決する。

---

## 2. 採用する設計方針 (不変の決定)

以下は実装中に揺らがせない決定。ここから外れる選択を提案する場合は、必ず人間の確認を取ること。

### 2.1 コアの責務

コアは次の責務だけに集中する。

- **agent registry**: agent_id (UUID) を主キーに、name / worktree / parent_id / tags / status / backend session reference を保持する
- **agent lifecycle**: spawn / list / attach / send / kill / link の 6 操作を提供する
- **agent 間の関係性**: 親子 (parent_id) と tag による緩いグループ化のみ。複雑な workflow (DAG / 依存解決 / 状態機械の遷移ルール) は組み込まない

これ以外 (画面分割、永続化、ジョブ DAG、scheduling、CI 統合) は組み込まない。

### 2.2 PTY 永続化は backend pluggable で外部に任せる

PTY を自前で握る実装は最小限にする。代わりに backend trait を切って、永続化は外部ツールに任せる。

- **tmux backend**: 既存 tmux session または専用 socket (`tmux -L myorch`) に new-session か new-window で agent を起動する。観測は `capture-pane` と `pipe-pane`。入力は `send-keys`。
- **shpool backend**: `shpool attach -d --cmd "..." <name>` で起動。観測は shpool の log file。入力は shpool 経由。
- **none backend**: setsid で detached 起動 + log file への tee。再 attach 不可だが、log は tail できる。最小公倍数として常に動く。

将来候補: abduco backend、systemd-run --user --pty backend (Linux 専用)、ACP backend (将来 Claude Code が ACP に成熟したとき)。

backend は trait として実装し、core から `dyn Backend` で参照される。core に tmux / shpool / 任意の具体実装が直接登場するコードを書かない。

```rust
trait Backend {
    fn spawn(&self, spec: &SpawnSpec) -> Result;
    fn list(&self) -> Result<Vec>;
    fn attach(&self, session: &SessionRef) -> Result;
    fn send(&self, session: &SessionRef, text: &str) -> Result;
    fn kill(&self, session: &SessionRef) -> Result;
    fn log_path(&self, session: &SessionRef) -> Result;
}
```

### 2.3 表示・分割の統合は optional adapter で切り離す

画面分割の責務はツールに持たせない。代わりに、ユーザの環境で利用可能な multiplexer / terminal を検出して、`agent open <name>` でその環境のベストプラクティスに乗せる薄い adapter を提供する。

検出ロジックは環境変数で行う。

- `$KITTY_LISTEN_ON` → kitty adapter (`kitty @ launch --type=tab`)
- `$WEZTERM_PANE` → wezterm adapter (`wezterm cli spawn`)
- `$TMUX` → tmux adapter (`tmux new-window` / `tmux split-window`)
- `$ZELLIJ` → zellij adapter (`zellij action new-tab`)
- `$VSCODE_INJECTION` → vscode adapter (現在のターミナルで attach、最低限)
- いずれも無し → detached adapter (`$TERMINAL -e` を試す → 失敗したら **「実行するコマンドを print して終わる」** という最終 fallback)

最初の対応は kitty を一級。tmux も最初から入れる (これは自身の backend と兼ねる場合もあるため)。残りは Phase 3 以降。

adapter も trait として core から分離する。

```rust
trait PaneHost {
    fn open(&self, title: &str, cmd: &[&str]) -> Result;
    fn focus(&self, pane: &PaneHandle) -> Result;
    fn close(&self, pane: &PaneHandle) -> Result;
}
```

### 2.4 agent から自分自身の CLI を叩ける形を維持する

agent (Claude Code 本体) が自分自身の所属する orchestrator を操作できるよう、CLI は **single binary** で提供する。fish 関数や bash 関数で実装してはいけない (subshell から呼べないため)。

具体的には Claude Code の system prompt や AGENTS.md に「`myorch list` で他の agent を見られる」「`myorch send <name> "<msg>"` で別 agent に送れる」と書いて、agent が tool として使えるようにする。

将来的には MCP server を立てて、agent から構造化された tool として叩ける形も追加する (Phase 3 以降)。

### 2.5 採用しない方向

以下は「議論の結果として明示的に採用しない」方向。実装が進んで気が変わったら人間に確認すること。

- **ACP に倒す**: 現時点では Claude Code の ACP 対応が限定的で、PTY を介した TUI 体験 (`/rewind` などの最新機能) が劣化する。将来 ACP が成熟したら backend として追加する候補。
- **Anthropic Agent SDK 直叩き**: TUI 体験を捨てるリスクが大きい。SwarmSDK 路線。これも将来 backend として追加する候補だが、最初は採らない。
- **自前で multiplexer を実装する**: 責務が爆発する。tmux / shpool で十分。
- **画面分割を内蔵する**: 責務外。Conductor / cmux / Crystal の Electron 路線は採らない。
- **ジョブ DAG / workflow engine を内蔵する**: Tutti 路線は採らない。必要なら外部の Pueue / systemd / Make を組み合わせる。
- **agent 状態の git 永続化 (Beads 方式)**: 思想として面白いが、最初は SQLite 1 ファイルで十分。

---

## 3. 言語選択の根拠

Rust を採用する。

### 3.1 Rust が今回の用途で優位な点

- **PTY を自前で握ることになった場合**: `portable-pty` (wezterm 由来) が成熟している
- **TUI dashboard を将来追加するとき**: `ratatui` が `bubbletea` (Go) より柔軟
- **設定ファイル・registry の serialization**: `serde` が他言語の追随を許さない
- **起動時間**: `agent ls` を 10ms 以下で返したい場合、Go の GC + runtime 初期化では構造的に厳しい
- **embedded SQLite**: `rusqlite` (CGo 不要) または `sqlx` (compile-time クエリ検証)
- **CLI フレームワーク**: `clap` の補完生成が成熟、fish 補完も自動生成される
- **本人習熟済み**: 学習コストゼロ

### 3.2 Go の優位性が今回の用途で決定的でない理由

- goroutine の楽さ: tokio で代替可能、本人習熟があるので差は小さい
- 外部プロセス呼び出しの書き味: `tokio::process::Command` でも十分書ける
- MCP/ACP の Go SDK: 翻訳すれば済む、依存性が決定的でない
- クロスコンパイル: cargo + cross / zig cc で出せる

### 3.3 将来 Go への部分的書き直しを検討する転機

- 長時間動く daemon に育てて、watchdog や複数 worker の並行処理が中核になったとき
- 新規貢献者を多く集めたいとき (Go の方が取っつきやすい人口が多い)
- MCP server を本格実装して、Anthropic の Go SDK の流用を考えるとき

これらは現時点では発生していない。

---

## 4. 既存ツールの評価結果と立ち位置

### 4.1 機能的に最も近い: Agent of Empires (njbrake/agent-of-empires)

- Rust + tmux + git worktree、TUI/CLI/Web ダッシュボード、Claude Code / OpenCode / Codex / Gemini CLI / Cursor CLI / Copilot CLI / Mistral Vibe / Pi.dev / Factory Droid 対応
- v1.0 到達済、Homebrew/Nix/cargo 配布、メンテ活発
- `aoe send` で agent 間メッセージング、Docker sandbox、Tailscale モバイル access
- **tmux 固定**。kitty / shpool / 非 tmux ユーザは取り込めない

### 4.2 設計が綺麗: Bosun (yetidevworks/bosun)

- Rust + ratatui + tmux control mode (`tmux -C`) + actor pattern
- 専用 tmux socket (`tmux -L bosun`) でユーザの tmux を汚さない
- single-writer な AppState、tmux I/O を 1 actor が握る
- **tmux 固定**

### 4.3 PTY 自前で握る数少ない例: ai-session crate (ccswarm 由来)

- Rust crate、PTY 自前管理、tmux 不要、MCP 対応
- ccswarm という個別プロダクトの一部、汎用 OSS としての成熟度は要検証
- **library として使えるなら理想だが、API 公開度は要確認**

### 4.4 その他の Rust 製候補

- **Agent Hand (weykon/agent-hand)**: agent-deck の Rust 実装、tmux 固定、UX のキー操作に特化
- **Batty (battyterm/batty)**: 階層 (Architect → Manager → Engineer)、Maildir messaging、kanban、tmux 固定
- **Tutti (nutthouse/tutti)**: workflow 志向、Web ダッシュボード、tmux 前提、Aider/Codex/Claude 対応

### 4.5 Rust 以外で構造参考

- **overstory (jayminwest/overstory)**: TS、11 runtime adapter pluggable、SQLite mail system、watchdog 三段、tmux opt-in / 主は headless subprocess + Web UI。**「PTY を escape hatch にする」**設計が今回の方向性に近い
- **muxtree**: 単一 bash スクリプト。極端なミニマル路線のベンチマーク
- **sesh (joshmedeski/sesh)**: Go、tmux session の registry。registry CLI の最小実装として参考になる
- **ittybitty**: bash、agent から自分の CLI を叩く設計
- **amux (mixpeek)**: Python 1 ファイル、REST API + Web UI、tmux 経由

### 4.6 ニッチ

「pluggable backend (tmux / shpool / none を切り替え可能) + multiplexer adapter (kitty を一級対応) + Rust」という組み合わせの OSS は、調べた範囲で見当たらない。これがこのプロジェクトの差別化要因。

---

## 5. 実装フェーズ

順序を厳守する。Phase 0 が終わるまで Phase 1 に入らない。

### Phase 0: 既存ツールで要件が満たせるか検証

実装に入る前に、Agent of Empires を実機で動かして判定する。

#### 0.1 インストール

```bash
brew install njbrake/aoe/aoe
# または
curl -fsSL https://raw.githubusercontent.com/njbrake/agent-of-empires/main/scripts/install.sh | bash
# または
cargo install --git https://github.com/njbrake/agent-of-empires
```

#### 0.2 チェックリスト

`docs/aoe-evaluation.md` を作成し、以下を 1 つずつ検証して結果を書く。

- [ ] Claude Code セッションを `aoe add --cmd claude` で並列 spawn できる
- [ ] git worktree と `aoe add --worktree feat/x --new-branch` で連動する
- [ ] tmux 経由で attach/detach できる、TUI から戻れる
- [ ] `aoe send <name> "..."` で別 agent に送信できる、Claude が受け取って動く
- [ ] kitty で使った時、kitty の tab/split 機能が aoe と矛盾しないか
- [ ] tmux を入れたくないユーザに勧められるか (= 入れるしかない、を許容できるか)
- [ ] shpool ユーザに勧められるか
- [ ] backend を pluggable にしたいというニーズが既存 issue や discussion に出ていないか

#### 0.3 判定

- aoe で十分 → 自作中止、aoe に PR / issue を出して機能追加で済ませる
- aoe で不十分 → Phase 1 に進む。不足点を新プロジェクトの差別化として明確に書き出す

判定は人間が下す。Claude Code が「aoe で十分」と結論しても鵜呑みにしない。

### Phase 1: 最小コア (auto 検出は Phase 2、kitty は Phase 2)

#### 1.1 リポジトリ構造
myorch/
├── Cargo.toml
├── CLAUDE.md (これ)
├── README.md
├── docs/
│   ├── design-discussion.md (議論の保存)
│   ├── aoe-evaluation.md (Phase 0 の結果)
│   └── architecture.md (実装が固まってから)
├── src/
│   ├── main.rs
│   ├── cli.rs (clap 定義)
│   ├── registry.rs (SQLite)
│   ├── backend/
│   │   ├── mod.rs (Backend trait)
│   │   ├── tmux.rs
│   │   ├── shpool.rs
│   │   └── none.rs
│   ├── adapter/
│   │   ├── mod.rs (PaneHost trait, auto-detect)
│   │   ├── kitty.rs (Phase 2)
│   │   ├── tmux.rs (Phase 2)
│   │   └── detached.rs
│   ├── config.rs (TOML)
│   └── error.rs
└── tests/
└── integration/

cargo workspace にはしない (single binary で始める)。

#### 1.2 依存 crate (初期セット)

- `clap` (derive feature)
- `rusqlite` (bundled feature で SQLite を embed)
- `serde` / `serde_json` / `toml`
- `directories` (XDG パス解決)
- `anyhow` / `thiserror`
- `tracing` / `tracing-subscriber`
- `uuid` (v4)

非同期は最初は要らない。tokio を入れるのは Phase 2 以降の watchdog や log tail を実装するとき。

#### 1.3 SQLite スキーマ

```sql
CREATE TABLE agents (
    id TEXT PRIMARY KEY,           -- UUID v4
    name TEXT NOT NULL UNIQUE,     -- 人間用の名前
    worktree TEXT,                 -- 絶対パス、null 可
    parent_id TEXT,                -- 別 agent の id、null 可
    backend TEXT NOT NULL,         -- "tmux" / "shpool" / "none"
    backend_ref TEXT NOT NULL,     -- backend 内での識別子 (tmux pane_id, shpool name, etc.)
    cmd TEXT NOT NULL,             -- 起動コマンド
    status TEXT NOT NULL,          -- "running" / "idle" / "dead" / "unknown"
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (parent_id) REFERENCES agents(id)
);

CREATE TABLE tags (
    agent_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (agent_id, tag),
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
);

CREATE INDEX idx_agents_name ON agents(name);
CREATE INDEX idx_agents_parent ON agents(parent_id);
CREATE INDEX idx_tags_tag ON tags(tag);
```

DB の場所: `$XDG_DATA_HOME/myorch/registry.db` (デフォルトは `~/.local/share/myorch/registry.db`)。

#### 1.4 subcommand (最小)
myorch new <name> [--worktree <dir>] [--cmd <cmd>] [--parent <name>] [--tag <tag>]...
myorch ls [--tag <tag>] [--parent <name>] [--json]
myorch attach <name>
myorch send <name> <text>...
myorch kill <name> [--remove-worktree]
myorch link <child> <parent>
myorch tag <name> <tag>...
myorch untag <name> <tag>...
myorch doctor   # backend の検出と PATH チェック

`open` `focus` `peek` は Phase 2。

#### 1.5 設定ファイル

`$XDG_CONFIG_HOME/myorch/config.toml` (デフォルト `~/.config/myorch/config.toml`):

```toml
[backend]
type = "auto"  # auto | tmux | shpool | none

[backend.tmux]
socket_name = "myorch"  # tmux -L myorch を使う

[backend.shpool]
# shpool は daemon を別途起動しておく前提

[adapter]
type = "auto"  # auto | kitty | tmux | wezterm | zellij | vscode | detached
```

backend = "auto" は以下の順で検出:

1. `$TMUX` がセットされている → tmux
2. `shpool` が PATH にある → shpool
3. それ以外 → none (warning を stderr に出す)

#### 1.6 Phase 1 完了条件

- `myorch new feat-x --cmd "claude --dangerously-skip-permissions" --worktree ../wt-feat-x` で agent が起動して registry に登録される
- `myorch ls` で agent が出る、`--json` で構造化出力できる
- `myorch attach feat-x` で attach できる、Ctrl+? で detach して myorch に戻れる
- `myorch send feat-x "テスト"` で Claude に届く
- `myorch kill feat-x` で停止し、registry から消える (--remove-worktree なら worktree も消える)
- backend を tmux と shpool の両方で動作確認する
- backend = none でも spawn / send / log は動く (attach は不可で warning)
- doctor が backend と adapter の状態を表示する

### Phase 2: kitty adapter と表示統合

- adapter trait を切る、auto 検出を実装する
- kitty adapter を一級で実装する (`kitty @ launch --type=tab`、`kitty @ focus-tab`、`kitty @ ls` で状態取得)
- tmux adapter (backend と兼用、`tmux split-window` / `tmux new-window`)
- detached adapter (`$TERMINAL -e` 試行 → 失敗時はコマンドを print)
- `myorch open <name>` (adapter に応じて画面に出す)
- `myorch focus <name>` (既存の表示を前面に)
- `myorch peek <name>` (現在のターミナルで log を tail)

### Phase 3: TUI ダッシュボードと観測

- ratatui で `myorch tui` を実装
- agent 一覧、status、log preview、key bind で attach/send/kill
- backend の status をリアルタイム更新 (tokio 導入)
- log tail (PTY pipe-pane / shpool log) を集約

### Phase 4: 拡張 (どれを先にやるかは需要次第)

- wezterm / zellij adapter
- abduco backend
- systemd-run --user --pty backend (Linux 限定)
- ACP backend (Claude Code が ACP に成熟したら)
- MCP server (agent から構造化された tool call で他 agent 操作)
- Web UI / モバイル access (Tailscale 経由)

---

## 6. 実装規約

### 6.1 コード構造

- backend を呼ぶコードを core ロジックに直接書かない、必ず `dyn Backend` 越しに呼ぶ
- adapter も同様、`dyn PaneHost` 越し
- backend と adapter は src/main.rs 起動時に config を読んで一度組み立てて、core にハンドルを渡す
- registry は `Registry` struct で隠蔽、SQL を core ロジックから直接書かない

### 6.2 エラー処理

- ライブラリ的な低レベルでは `thiserror` で named error 型
- main / コマンドハンドラでは `anyhow::Result` に集約
- ユーザ向けエラーは「何ができなかったか + 次に何をすればよいか」を含める。`doctor` への誘導を活用する

### 6.3 テスト

- backend trait に対する mock 実装をテストで使う
- 統合テストは `tests/integration/` で実 tmux / 実 shpool を呼ぶが、CI では skip 可能にする
- snapshot test (`insta`) は出力フォーマットが固まってから導入

### 6.4 配布

- Homebrew tap、`cargo install`、`curl ... | sh` の 3 経路
- GoReleaser 相当として `cargo-dist` を採用検討
- macOS arm64 / macOS x86_64 / Linux x86_64 / Linux arm64 / Linux musl の 5 ターゲット

### 6.5 ドキュメント

- README は「3 行で何ができるか」+「各環境別の Quick start」
- 環境別 recipe ("I use tmux" / "I use kitty" / "I use VS Code" / "I'm on Windows/WSL" / "I just want it to work") を docs/ に分けて書く
- 同じツールでも環境ごとに違う書き方で説明する。tmux 派には tmux 中心、kitty 派には kitty 中心

---

## 7. ぶれてはいけない決定 (再掲)

実装中に「シンプルさのため」「機能要求のため」を理由にこの決定から外れる選択を提案する場合は、必ず人間に確認すること。

- backend は trait で抽象化する (tmux 直結のコードを書かない)
- multiplexer adapter は core から完全に切り離す (core は環境非依存)
- registry の主キーは agent_id (UUID)、backend が持つ session 名はその attribute
- agent から自身の CLI を叩ける形を維持する (single binary、subshell から呼べる形)
- 画面分割を内蔵しない、永続化を内蔵しない、ジョブ DAG を内蔵しない
- 最初は kitty 1 つ + tmux backend 1 つ + shpool backend 1 つ + none backend 1 つの最小構成
- Phase 0 の判定が出るまで Phase 1 に進まない
- Phase 1 完了条件 (1.6) を満たすまで Phase 2 に進まない

---

## 8. 用語

- **agent**: 1 つの CLI コーディングエージェントのインスタンス。Claude Code、OpenCode、Codex CLI など
- **session**: backend (tmux / shpool / none) が管理する PTY 単位
- **registry**: agent と session の対応、属性、関係性を保持する SQLite DB
- **backend**: PTY 永続化を担当する外部ツール抽象
- **adapter (PaneHost)**: 表示・画面分割を担当する外部ツール (multiplexer / terminal) 抽象
- **worktree**: git worktree。agent ごとに別ディレクトリで作業させる場合の隔離単位

---

## 9. 参考資料

`docs/design-discussion.md` に至るまでの議論の経緯がある場合はそれを参照。無い場合はこの CLAUDE.md が唯一の正本。

外部参照:

- Agent of Empires: https://github.com/njbrake/agent-of-empires
- Bosun: https://github.com/yetidevworks/bosun
- ai-session crate: https://crates.io/crates/ai-session
- shpool: (Google が開発する Rust 製 PTY セッションマネージャ)
- portable-pty: https://crates.io/crates/portable-pty
- ratatui: https://crates.io/crates/ratatui
- clap: https://crates.io/crates/clap
- rusqlite: https://crates.io/crates/rusqlite
- kitty remote control: https://sw.kovidgoyal.net/kitty/remote-control/
- ACP (Agent Client Protocol): Zed が中心になって策定中の標準
- MCP (Model Context Protocol): Anthropic が策定する agent ↔ tool 標準
