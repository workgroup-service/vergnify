# 開発計画

進捗はこのチェックリストで追います。完了した項目は `[x]` にします。期間は目安です。

現在地は Phase 4 です。Phase 3.5 まで完了しています。

@author Cursor

## Phase 1: Core と CLI

目安は 2〜3 週間です。完了しています。

- [x] `vergnify-core` と `vergnify` CLI
- [x] 主要ランタイムと CLI の検出（PATH とバージョンマネージャ）
- [x] `vergnify list`
- [x] `vergnify check`。系列ごとの最新パッチで `STATUS` を決め、`NEWER` を出す
- [x] 最新版の 24 時間キャッシュ

## Phase 2: GPUI インターフェース

目安は 3〜4 週間です。完了しています。

- [x] GPUI による一覧表示
- [x] バージョン状態のバッジ
- [x] 更新の実行基盤（`vergnify update` と、homebrew / winget / scoop / apt）

## Phase 3: 脆弱性検知と通知

目安は 2〜3 週間です。完了しています。

- [x] OSV.dev API との照合
- [x] CVE 情報の表示
- [x] CVSS 7.0 以上のネイティブ通知

## Phase 3.5: 表示言語

完了しています。OS の優先言語が日本語のときだけ日本語、それ以外は英語です。

- [x] 日英の文言カタログ
- [x] GUI、CLI、警告、OS 通知での切り替え

## Phase 4: Admin 同期

目安は 4 週間以上です。手元のオンプレミス Admin への送信まで入っています。

- [x] インベントリを送るバックグラウンドエージェント（`vergnify-agent`）
- [x] 手元の Docker Compose で PostgreSQL と Admin を起動する
