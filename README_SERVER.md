# Admin サーバーのビルド

Admin は Go 1.25 です。ソースは [admin](admin) にあり、画面テンプレートは実行時に `templates/` から読みます。開発環境は Docker Compose、本番の成果物は [admin/Dockerfile](admin/Dockerfile) のイメージです。

@author Cursor

## 開発環境

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

## 本番環境

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
