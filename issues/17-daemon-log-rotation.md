# Daemon ログのローテーション

- **Priority:** 中

### Description

- **Summary:** `docs/plan.md` Resolved に「Append-only for MVP; rotation is out of scope」と書かれているが、実運用が長引くと無視できないサイズに育つ。`$XDG_STATE_HOME/picoswarm/daemon.log` がただ膨らむ。
- **Impact:** 数か月放置で GB 級になる可能性。disk / log viewer / inotify の負荷。日常的に困る前に手を打っておくのが安い。
- **Proposed Solutions:**
  1. **`tracing-appender` の rolling file** (小, 1〜2 時間): `RollingFileAppender::new(Rotation::DAILY, dir, "daemon.log")` 等で日次 rotate。残数も `with_max_log_files` で制御。トレードオフ: 依存追加 (`tracing-appender`)。
  2. **OS の `logrotate` 任せ** (極小, 設定例だけ用意): README に logrotate 例を書く。トレードオフ: 自前で完結しない、ユーザーの環境次第。
  3. **サイズ閾値で in-process rotate** (小〜中, 半日): 起動時 + 定期にサイズチェックして自前で `.1` / `.2` をリネーム。トレードオフ: 車輪の再発明。
- **Knowledgement:**
  - `docs/plan.md` Resolved の log path 項
  - `src/daemon/lifecycle.rs:33-38` 現在の log open 箇所
  - tracing-appender: https://docs.rs/tracing-appender
  - 関連 issue: #05 (lifecycle log のローテーションも同じ枠で扱える)
