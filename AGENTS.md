# Vergnify

開発者マシン上の言語ランタイムと CLI ツールのバージョンを可視化し、更新を支援し、既知の脆弱性（CVE）を警告するデスクトップアプリと CLI です。GUI（GPUI、JetBrains Toolbox 風）と CLI（clap）は同じコアを使います。

設計の出典は [code_artifact.md](code_artifact.md) です。デフォルト以外のツール定義は [custom-catalog.md](custom-catalog.md) です。未実装です。製品名は、このファイルが優先します。仕様書の仮称 VerCheck / StackBox、および CLI 名 `stackbox` は使いません。

## 質問があれば聞くこと

仕様、命名、フェーズの範囲が曖昧なとき、または現在のフェーズを超える実装が必要に見えるときは、実装の前に質問してください。

## 提供形態

- この製品はオンプレミスだけで動きます。Admin も、利用する人が自分の環境で動かします。
- クラウドでの販売、契約、料金計算はありません。

## 技術

- 言語は Rust、edition は 2021 です。
- 非同期は tokio、バージョン比較は semver、ローカル DB は rusqlite または sled です。
- 脆弱性情報は OSV.dev、OS 通知は notify-rust です。
- 更新の実行は homebrew、winget、scoop、apt に合わせます。
- 依存クレートは各フェーズで追加します。環境構築の雛形には外部クレートを入れません。
- GPUI は macOS のネイティブ描画（Metal など）を使います。GUI はホストで動かします。

## 構成

```text
crates/vergnify-core    検出・比較・CVE・設定
crates/vergnify-cli     CLI（バイナリ名 vergnify）
crates/vergnify-gui     GPUI（Phase 2）
crates/vergnify-agent   Admin 同期（Phase 4）
```

## フェーズ

進捗は [PLAN.md](PLAN.md) です。現在地は Phase 4 です。Phase 1 から Phase 3.5 は完了しています。

コーディングルールは `.cursor/rules/`、Skill は `.cursor/skills/` に、必要になった時点で追加します。

@author Cursor
