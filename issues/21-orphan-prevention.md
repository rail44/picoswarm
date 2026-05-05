# Daemon ハードクラッシュ時の orphan agent 対策 (PR_SET_PDEATHSIG)

- **Priority:** 中

### Description

- **Summary:** `pswarm daemon stop` のような正常終了では `Registry::shutdown_all` が全 child を kill してくれるが、daemon が SIGKILL されたり panic で死んだ場合、agent process は init (pid 1) の子として生き残る。orphan を後で拾い直す手段もない (registry は in-memory)。
- **Impact:** daemon がクラッシュすると「動いているが管理外」の agent が増殖し、リソース (主に Claude のような重い process) が食われる。手動で `ps` で探して kill するしかない。
- **Proposed Solutions:**
  1. ~~**`prctl(PR_SET_PDEATHSIG, SIGTERM)` を child に設定**~~: 調査の結果 **不可**。portable-pty 0.9.0 の `CommandBuilder` は `pre_exec` を露出しておらず、内部の `unix.rs:238` で setsid/TIOCSCTTY 用に専有している。`std::process::Command::pre_exec` は複数回呼ぶと後勝ち上書きなので、portable-pty 経由で追加で hook を差し込む方法はない。実現するなら portable-pty を vendoring するか upstream PR が必要。
  2. **PID file + 起動時の orphan reaper** (中, 1〜2 日): 起動時に `$XDG_STATE_HOME/picoswarm/agents/<id>.pid` を書く。新 daemon 起動時にディレクトリを舐めて `kill 0 pid` で孤児を確認、SIGTERM。トレードオフ: 起動 path にロジックが増える、stale pid file の判定を堅く書く必要。
  3. **portable-pty を vendoring + `pre_exec` に prctl 追加** (大, 1〜2 日): unix backend だけ持ち込んでパッチを当てる。upstream に追従するメンテコストと引き換えに最も綺麗な挙動。トレードオフ: 単一目的の依存を持ち込むのが重い。
  4. **upstream に PR して merge を待つ** (リードタイム不確定): wezterm リポジトリへの PR。マージされれば 0 メンテで済むがいつ動くかわからない。
  5. **何もしない** (なし): クラッシュは稀という前提で運用。トレードオフ: 起きた時に痛い。
- **Knowledgement:**
  - `src/daemon/server.rs:80-88` 正常終了時の `shutdown_all`
  - `src/daemon/session.rs:60-63` 現在の child spawn (pre_exec の挿入点候補)
  - `prctl(PR_SET_PDEATHSIG)`: https://man7.org/linux/man-pages/man2/prctl.2.html
