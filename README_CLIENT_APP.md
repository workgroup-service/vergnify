# クライアントアプリのビルド

GUI、CLI、エージェントは同じ Cargo ワークスペースです。開発ビルドと本番ビルドの違いは最適化の有無で、別のソースや feature フラグはありません。アイコンはバイナリに埋め込まれます。

Rust は [rust-toolchain.toml](rust-toolchain.toml) の 1.90.0 を使います。リポジトリのルートで `cargo` を実行します。

@author Cursor

## 開発環境

デバッグ情報付きで、変更のたびにそのまま起動します。

```bash
cargo run -p vergnify-cli -- check
cargo run -p vergnify-gui
```

GUI のメニューバーに出る名前は Vergnify です。Vergnify メニューの「登録」（英語では Enroll）から、この PC を Admin に登録します。承認が終わると、GUI が在庫を送り、起動しているあいだは 15 分ごとに送り続けます。接続先は `VERGNIFY_ADMIN_URL`、または以前保存した資格情報です。

エージェントは、招待を Accept した人が端末を登録したあと、その端末のトークンで在庫を送ります。

```bash
VERGNIFY_ADMIN_URL=http://127.0.0.1:8080 \
cargo run -p vergnify-agent -- --enroll
```

表示されたコードを、Accept 後の登録画面で承認します。資格情報は `~/Library/Application Support/vergnify/agent-credential.json` に保存されます。その後は次で在庫を送ります。

```bash
VERGNIFY_ADMIN_URL=http://127.0.0.1:8080 \
cargo run -p vergnify-agent -- --once
```

`--once` を外すと、起動時に1回送り、その後は15分ごとに送ります。間隔は `VERGNIFY_SYNC_INTERVAL_SECS`（60秒以上）で変えられます。

GUI は macOS の Metal で描画します。ビルド時に `xcrun -sdk macosx metal` が必要です。Metal Toolchain が無いときは、Xcode から Metal Toolchain を入れてからやり直します。

## 本番環境

リリース最適化で3つのバイナリを作ります。

```bash
cargo build --release -p vergnify-cli -p vergnify-gui -p vergnify-agent
```

成果物は次です。

- `target/release/vergnify`
- `target/release/vergnify-gui`
- `target/release/vergnify-agent`

エージェントの接続先はビルドには入っていません。配布先では、自分たちで動かす Admin の URL で `--enroll` し、保存された端末トークンで送ります。`VERGNIFY_AGENT_TOKEN` を渡したときは、そのトークンをファイルより優先します。

```bash
VERGNIFY_ADMIN_URL=https://vergnify.internal ./vergnify-agent --enroll
VERGNIFY_ADMIN_URL=https://vergnify.internal ./vergnify-agent
```

署名、公証、インストーラ作成はこのリポジトリにはありません。GUI の本番バイナリも、開発ビルドと同じく macOS と Metal が必要です。
