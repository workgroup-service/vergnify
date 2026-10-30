# Vergnify

クライアント（GUI、CLI、エージェント）と、自分たちで動かす Admin サーバーです。

@author Cursor

## クライアント

GUI、CLI、エージェントは同じ Cargo ワークスペースです。開発ビルドと本番ビルドの違いは最適化の有無で、別のソースや feature フラグはありません。アイコンはバイナリに埋め込まれます。

Rust は [rust-toolchain.toml](rust-toolchain.toml) の 1.90.0 を使います。リポジトリのルートで `cargo` を実行します。

### 開発環境

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

Mac 上で GUI をビルドするときは Metal を使います。ビルド時に `xcrun -sdk macosx metal` が必要です。Metal Toolchain が無いときは、Xcode から Metal Toolchain を入れてからやり直します。Windows と Linux 向けの GUI は、下の Actions がビルドします。

### 配布

`main` への push、または Actions の Build を手動実行すると、Windows、Mac、Linux のリリースビルドが Artifact になります。各 Artifact は 90 日残ります。中身は CLI `vergnify`、GUI `vergnify-gui`、エージェント `vergnify-agent` です。Windows では拡張子が `.exe` です。

GitHub の Actions タブで、成功した Build の実行を開き、下の Artifacts から OS に合うものを落とします。

| Artifact | 実行環境 |
| --- | --- |
| `vergnify-macos-aarch64` | Apple Silicon の macOS。GUI は Metal で描画します |
| `vergnify-windows-x86_64` | 64-bit Windows |
| `vergnify-linux-x86_64` | 64-bit Linux。GUI の実行には Vulkan と、Wayland または X11 が要ります |

署名、公証、インストーラ作成はこのリポジトリにはありません。macOS では、未署名のアプリを開く許可が必要になることがあります。

手元で同じバイナリを作るときは、リポジトリのルートで次を実行します。

```bash
cargo build --release -p vergnify-cli -p vergnify-gui -p vergnify-agent
```

成果物は `target/release/` に出ます。

エージェントの接続先はビルドには入っていません。配布先では、自分たちで動かす Admin の URL で `--enroll` し、保存された端末トークンで送ります。`VERGNIFY_AGENT_TOKEN` を渡したときは、そのトークンをファイルより優先します。

```bash
VERGNIFY_ADMIN_URL=https://vergnify.internal ./vergnify-agent --enroll
VERGNIFY_ADMIN_URL=https://vergnify.internal ./vergnify-agent
```

## Admin

Admin は Go 1.25 です。ソースは [admin](admin) にあり、画面テンプレートは実行時に `templates/` から読みます。開発環境は Docker Compose、本番の成果物は [admin/Dockerfile](admin/Dockerfile) のイメージです。

### 開発環境

リポジトリのルートで、PostgreSQL 16 と Admin を起動します。Admin は `127.0.0.1:8080` だけに公開され、PostgreSQL はホストへ公開されません。

```bash
docker compose up --build
```

この Compose が渡す値は開発用です。

- `DATABASE_URL=postgres://vergnify:vergnify@postgres:5432/vergnify?sslmode=disable`
- 招待メールは Mailpit（`http://127.0.0.1:8025`）に届きます

最初の Owner は、起動したあと次で作ります。Owner が既にいるときは拒否します。

```bash
docker compose run --rm admin create --email owner@example.com --password 'owner-pass'
```

データベースへ届いているかは `http://127.0.0.1:8080/healthz` が `ok` を返すことで確認できます。端末一覧は `http://127.0.0.1:8080/list` です。止めるときは `docker compose down` を使います。ボリューム `pgdata` は残ります。

Go だけをホストで起動する場合も、`DATABASE_URL` が必要です。作業ディレクトリは `admin` にします。テンプレートの相対パスがそこを基準にするためです。

```bash
cd admin
DATABASE_URL=postgres://vergnify:vergnify@127.0.0.1:5432/vergnify?sslmode=disable \
go run .
```

Compose の PostgreSQL はホストの 5432 へ出ていないので、この起動方法では自分で到達できる PostgreSQL を用意します。

### 本番環境

イメージはマルチステージで、静的リンクのバイナリとテンプレートだけを Alpine 3.20 に入れます。

```bash
docker build -t vergnify-admin:0.1.0 ./admin
```

コンテナ内の待ち受けは `:8080` です。TLS はこのイメージの外、前段のプロキシで終端します。起動時に次を渡します。

- `DATABASE_URL`。本番の PostgreSQL を指し、開発用の `vergnify:vergnify` と `sslmode=disable` は使いません。
- `PUBLIC_URL`。招待メールに載せる、自分たちの Admin の URL です。
- `SMTP_HOST` と `SMTP_PORT`。招待メールの送信先です。

在庫を送る資格は、Accept した人が端末を登録したときに発行します。共通のトークンはありません。

```bash
docker run --rm -p 8080:8080 \
  -e DATABASE_URL='postgres://USER:PASSWORD@postgres:5432/vergnify?sslmode=require' \
  -e PUBLIC_URL='https://vergnify.internal' \
  -e SMTP_HOST=smtp.internal \
  -e SMTP_PORT=587 \
  vergnify-admin:0.1.0
```

最初の Owner は、同じイメージで `create` を足して作ります。

```bash
docker run --rm \
  -e DATABASE_URL='postgres://USER:PASSWORD@postgres:5432/vergnify?sslmode=require' \
  vergnify-admin:0.1.0 create --email owner@example.com --password "$OWNER_PASSWORD"
```

このイメージは、自分たちで動かす Admin です。公開クラウド用の配備や料金計算はありません。
