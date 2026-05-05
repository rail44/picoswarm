# Bash / zsh の補完スクリプト

- **Priority:** 中

### Description

- **Summary:** `pswarm completions fish` は実装済み (commit d281e00) だが、bash / zsh は未対応で、`shell != "fish"` で `bail!` する。
- **Impact:** picoswarm を fish 以外で使うユーザー (将来の他人を含む) は補完なし。本人ユースケースが fish 100% であれば即時の困りごとはない。
- **Proposed Solutions:**
  1. **bash / zsh のスクリプトも手書き** (小〜中, 1 日): fish 版と同じ方針で `__pswarm_agent_names` 相当を bash/zsh 流に書く。トレードオフ: 3 つのスクリプトを手で同期する必要、bash の補完仕様が辛い。
  2. **`clap_complete` を導入** (小, 半日): 依存追加だけでほぼ自動生成。動的補完 (agent name) は generated script に追記する形で別途差し込み。トレードオフ: 依存追加 (~5 crates), 動的補完を後付けする手間。
  3. **fish のみ supported と割り切る** (なし, 即時): README に明記して終わり。トレードオフ: 将来他人に勧める時の体験が悪化。
- **Knowledgement:**
  - `src/client/completions.rs` 現在の実装 (fish 以外で `bail!`)
  - `src/client/completions/pswarm.fish` fish 版
  - clap_complete: https://docs.rs/clap_complete
