# デフォルト以外のツール定義

組み込みの12ツールに無いランタイムや CLI を、Vergnify に追加する仕様です。未実装です。実装するときは、このファイルを正とします。

定義ファイルは、利用者が自分のマシンに置きます。

@author Cursor

## 方針

追加に必要な差分はデータです。

- バイナリ名と、バージョンを表示する引数
- 出力からバージョンを取り出す規則
- 系列の切り方
- 最新版の出典

定義は TOML で書きます。ネイティブライブラリの動的読み込みや WASM は使いません。組み込みの取得処理と外部コマンドは、どちらも同じ `ReleaseLine` の配列を返すリゾルバとして扱います。

## ファイルの場所

次の順で読み、同じ `id` は後の定義で置き換えます。

1. 組み込みカタログ
2. ユーザー定義
3. `--catalog` で渡したファイル

ユーザー定義のパスです。

- macOS: `~/Library/Application Support/vergnify/tools.toml`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/vergnify/tools.toml`
- 同じディレクトリの `tools.d/*.toml` も読みます

`--catalog` はファイルかディレクトリです。CI でリポジトリ内の定義を渡すときに使います。

## ツール定義

```toml
[[tool]]
id = "bun"
name = "Bun"
binaries = ["bun"]
args = ["--version"]
line = "single"

[tool.latest]
kind = "github"
owner = "oven-sh"
repo = "bun"
```

| フィールド | 必須 | 内容 |
| --- | --- | --- |
| `id` | はい | `vergnify` 内の識別子。英小文字、数字、ハイフン |
| `name` | はい | 表示名 |
| `binaries` | はい | PATH とバージョンマネージャから探す実行ファイル名。先にあるものを採用する |
| `args` | はい | バージョン表示の引数 |
| `line` | はい | `major`、`major_minor`、`single` のいずれか |
| `pattern` | いいえ | 出力から版を取る正規表現。無名グループの1つ目、または `version` という名前のグループを使う。省略時は最初の `major.minor` または `major.minor.patch` |
| `latest` | はい | 下の出典のいずれか |

`line` の意味は現在の系列判定と同じです。

- `major`: Node.js の 22、Java の 17
- `major_minor`: Python の 3.13、Go の 1.27
- `single`: 保守系列が1本。`NEWER` は空になる

ローカル版は、そのバイナリを `args` 付きで実行し、標準出力と標準エラーから取ります。判定は既存の `STATUS` と `NEWER` をそのまま使います。

## 最新版の出典

### `github`

Release の `tag_name` を使い、Release が無いか prerelease ならタグの最新安定版を使います。結果は系列1本です。

```toml
[tool.latest]
kind = "github"
owner = "oven-sh"
repo = "bun"
```

### `endoflife`

`https://endoflife.date/api/<product>.json` の cycle を系列にします。EOL と LTS の扱いは組み込みの Python や Java と同じです。

```toml
[tool.latest]
kind = "endoflife"
product = "bun"
```

### `http`

JSON なら `pointer` の位置の文字列を版にします。JSON 以外なら `pattern` を本文へ適用します。版が1つなら系列は1本です。`lines_pointer` がある場合、その位置は下の「系列の JSON」の配列です。

```toml
[tool.latest]
kind = "http"
url = "https://example.com/helm/releases.json"
pointer = "/stable"
```

### `file`

閉域や社内ミラーに置いたファイルです。中身が版の1行なら系列は1本です。JSON 配列なら系列の JSON として読みます。

```toml
[tool.latest]
kind = "file"
path = "/opt/vergnify/versions/bun.txt"
```

### `command`

外部コマンドです。引数の末尾にツールの `id` を渡します。終了コード 0 の標準出力だけを見ます。

- 1行の版なら、系列は1本です
- JSON オブジェクトなら、系列の JSON として読みます

```toml
[tool.latest]
kind = "command"
command = "company-vergnify-resolve"
args = ["--offline"]
```

タイムアウトは 20 秒です。標準エラーは警告に回し、版としては使いません。

## 系列の JSON

`command` の標準出力、および `http` / `file` の系列配列は、次の形です。フィールドは `ReleaseLine` と同じです。

```json
{
  "lines": [
    {
      "id": "3.13",
      "label": "3.13",
      "latest": "3.13.2",
      "support": "maintenance"
    }
  ]
}
```

`support` は `active_lts`、`maintenance`、`current`、`eol`、`stable` のいずれかです。

- `eol` の系列に今の版が属していれば `STATUS` は `unsupported`
- `eol` 以外で、今の版より新しい系列が `NEWER` に出ます
- `latest` は `major.minor` または `major.minor.patch`

## 組み込みツールとの関係

組み込みの Node.js、Go、Rust、endoflife.date、GitHub の取得も、同じ `ReleaseLine` を返す内部リゾルバです。ユーザー定義が同じ `id` を持つときだけ、出典と検出コマンドを置き換えます。定義が無いツールは、今までどおり組み込みカタログのままです。

## まだやらないこと

- プロセス内へのネイティブプラグインの読み込み
- WASM
- 信頼できないカタログ向けのサンドボックス
- このファイルに書いた TOML の読み込み実装
