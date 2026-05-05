# Daemon 再起動時の attach 体験改善

- **Priority:** 中

### Description

- **Summary:** `pswarm daemon restart` 中に attach 中の client は EOF だけ食って黙って `[connection closed]` で終わる。新しい daemon に再接続したり、エラーメッセージで状況を伝えたりはしない。`PROTOCOL_VERSION` mismatch (binary upgrade した場合) でも対面のメッセージが分かりにくい。
- **Impact:** 「なんで突然落ちた?」のデバッグが要る。本人 (= 開発中の自分) は原因を知っているが、しばらく経つと忘れる。
- **Proposed Solutions:**
  1. **エラーメッセージを足すだけ** (極小, 30 分): `AttachExit::Closed` のときに「daemon との接続が切れました。`pswarm doctor` を確認」と促す。トレードオフ: 体験は最低限改善、自動再接続はしない。
  2. **自動再 attach 試行** (中, 1〜2 日): EOF を受けたら最大 N 秒 daemon socket を待ち、再 attach (同名)。再 attach 成功時は画面 redraw 要求 (issue #07 と関連)。トレードオフ: ロジック増、agent が dead だった場合の分岐。
  3. **server 側で graceful shutdown 通知を送る** (小〜中, 半日): `Shutdown` を受けたら active attach に `SessionEnded { exit_code: None }` ではなく `Error { code: DaemonShuttingDown }` を流す。client はそれを見て分かりやすく出す。トレードオフ: protocol に新 ErrorCode 追加 → version bump (#04 と一緒に)。
- **Knowledgement:**
  - `src/client/attach.rs:101-132` `stream_loop` の終了パス
  - `src/daemon/server.rs:155-160` Shutdown 受理パス (active attach への通知なし)
  - `src/client/connection.rs:59-62` version mismatch メッセージ
  - 関連 issue: #04 (protocol doc), #07 (画面復元)
