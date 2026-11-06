<!-- translated from en/workflows.md sha256:ced46a4d6c27e0a9 -->

# ワークフロー

よくある作業の手順をまとめたレシピ集です。[ユーザーガイド](user-guide.md)はアプリの各部分を説明し、
[技術ノート](technical.md)は Apix のビルドと計測を扱います。

- [最初のリクエストを送信して保存する](#最初のリクエストを送信して保存する)
- [コレクションを git で管理する](#コレクションを-git-で管理する)
- [dev と staging と production を使い分ける](#dev-と-staging-と-production-を使い分ける)
- [シークレットを含むコレクションを共有する](#シークレットを含むコレクションを共有する)
- [Postman から移行する](#postman-から移行する)
- [コレクションを CI で実行する](#コレクションを-ci-で実行する)
- [モニターで API を監視する](#モニターで-api-を監視する)
- [OAuth 2.0 でサインインする](#oauth-20-でサインインする)
- [AWS の API を呼び出す](#aws-の-api-を呼び出す)
- [ファイルのアップロードと GraphQL の送信](#ファイルのアップロードと-graphql-の送信)
- [gRPC サービスを呼び出す](#grpc-サービスを呼び出す)
- [ブラウザーのリクエストを再現する](#ブラウザーのリクエストを再現する)
- [リクエストが遅い理由や失敗する理由を調べる](#リクエストが遅い理由や失敗する理由を調べる)
- [AI エージェントにコレクションを使わせる（MCP）](#ai-エージェントにコレクションを使わせるmcp)

## 最初のリクエストを送信して保存する

1. **ファイル › 新しいコレクション…** を選び、空のフォルダーを選んで名前を付けます。
2. Ctrl+N で新しい GET リクエストが開きます。`https://jsonplaceholder.typicode.com/users` と入力します。
3. Ctrl+Enter で送信すると、右側にレスポンスが表示されます。
4. Ctrl+S でコレクションフォルダーに新しいファイルとして保存します。サイドバーでそれを右クリックし、
   **名前を変更** で「List users」のような名前を付けます。

ファイルは読み書きできるプレーンな TOML です。

```toml
name = "List users"
url = "https://jsonplaceholder.typicode.com/users"
```

## コレクションを git で管理する

コレクションはフォルダーです。

```
my-api/
├── apix.toml                コレクション名、変数、共有の認証
├── environments/
│   ├── staging.toml         共有する値
│   └── staging.local.toml   シークレットの値（gitignore 対象）
├── users/
│   ├── folder.toml          任意：このフォルダー内のリクエストの認証
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

`*.local.toml` 以外はすべてコミットします。Apix は初めてシークレットを書き込むときに、
そのパターンを自分で `.gitignore` に追加します。リクエストはそれぞれ小さな独立したファイルなので、
レビューやマージが読みやすいままです。

## dev と staging と production を使い分ける

1. **環境 › 変数と環境…**（Ctrl+E）を開きます。
2. `dev`、`staging`、`prod` という名前の環境を追加し、それぞれに `base_url` を設定します。
3. リクエストでは `{{base_url}}/users` を使います。
4. サイドバーのコレクション名の下で環境を選びます。

コミットしてはいけない値（トークン、パスワード）には **シークレット** のチェックを付けます。
チームのメンバーはそれぞれ自分の `*.local.toml` を持ちます。コレクションのレベルで定義した変数は
既定値になり、環境で上書きできます。

## シークレットを含むコレクションを共有する

コレクション全体が機密である場合（社内の URL、顧客のサンプルデータ、長期間有効なキー）は、
暗号化します。

1. **ファイル › このコレクションを暗号化…** を選び、パスワードを決めます。
2. 表示された**回復キー**をパスワードマネージャーに保存します。
3. フォルダーをコミットします。ディスク上では暗号化されたオブジェクトの集まりになり、
   ファイル名さえ隠されます。

チームのメンバーは Apix でフォルダーを開き、パスワードを入力します。ターミナルや CI では次のようにします。

```sh
APIX_PASSWORD='…' apix run my-api --env staging
```

`apix decrypt my-api --out my-api-plain` は平文のコピーを書き出します。パスワードの代わりに
回復キーを使うには `--recovery` を付けます。

## Postman から移行する

1. Postman で、コレクション（Collection v2.1）と各環境をエクスポートします。
2. Apix で **ファイル › インポート…** を選ぶか、ファイルをウィンドウにドロップします。
   ターミナルなら次のようにします。

   ```sh
   apix import "My API.postman_collection.json"            # ./my-api を作る
   apix import "Staging.postman_environment.json" --into my-api
   ```

3. インポートのレポートを読みます。引き継げなかったものはすべてそこに一覧されます
   （たとえば NTLM 認証や保存済みのレスポンス例）。
4. プリリクエストスクリプトとテストスクリプトは、リクエストの隣に `.js` ファイルとして残ります。
   Apix はまだそれらを実行しません。
5. 認可コードグラントを使う OAuth 2.0 では、`http://127.0.0.1:8765/callback` のような
   ループバックのリダイレクト URL を ID プロバイダーに登録し、**認証** タブで設定します。

ワークスペースごと移行しますか？ Postman の **Settings › Data › Export data** を使い、
そのフォルダー（または zip）を Apix で開くかドロップします。すべてのコレクション、環境、
グローバル変数が 1 つのプロジェクトとして取り込まれます（`apix import postman-export.zip` も
同じことをします）。Postman を使い続けるメンバーには、**Postman にエクスポート…** で
コレクションを渡せます。

## コレクションを CI で実行する

`apix run` はコレクションのすべてのリクエストをサイドバーの順に送信し、1 つでも失敗すると
0 以外のコードで終了します。そのため CI のステップとして使えます。

```sh
apix run my-api --env staging --var token="$API_TOKEN"
```

テスト（スクリプトまたは**チェック**）のあるリクエストはテストによって成功・失敗が決まり、
それ以外はステータスコードによって決まります。出力では各テストがリクエストの下に並びます。

```text
create.apix                              201 Created  466.7 ms · 42 B
  │ new id 101
  ✓ created
get.apix                                 200 OK  174.7 ms · 292 B
  ✓ status 200
  ✗ has the right id expected 1 to equal 2

1 passed, 1 failed (1 failed tests)
```

スクリプトが設定した値（`pm.environment.set`、`pm.variables.set`）は実行中の後続のリクエストに
引き継がれ、`pm.execution.setNextRequest("name")` は別のリクエストにジャンプします
（`null` でその反復を終えます）。

オプション：

| オプション | 動作 |
|---|---|
| `apix run my-api/users` | 1 つのフォルダー内のリクエストだけ（または 1 つのリクエストファイル） |
| `-d, --data users.csv` | データ駆動の実行：CSV または JSON の各行が 1 回の反復になり、列が変数になります（`{{email}}`、`pm.iterationData.get("email")`） |
| `-n, --iterations 5` | リクエストを 5 回繰り返します（既定：データ行ごとに 1 回、なければ 1 回） |
| `--delay 200` | リクエストの間に 200 ms 待ちます |
| `--bail` | 最初に失敗したリクエストで止めます |
| `-p, --parallel 4` | 4 人のユーザーのように 4 つの反復を同時に実行します。それぞれが順にリクエストを送り、スクリプトが設定した値は自分の中だけに保ちます |
| `--junit report.xml` | JUnit XML レポート。CI サービスはこれをテストの一覧として表示します |
| `--json report.json` / `--html report.html` | すべてを JSON で、またはどのブラウザーでも開けるページで |
| `--markdown report.md` | プルリクエストのコメントや CI ジョブのサマリー用の要約（下記参照） |
| `--changed` / `--changed=origin/main` | 変更が影響するリクエストだけ（下記参照） |
| `--timeout 10`、`-k`、`--proxy URL` | リクエストの通信設定を上書きします |
| `--env-file .env` | dotenv ファイルから変数を読みます（下記参照） |

データファイルは次のような形です。コレクションの外から来る値（`--var` と下記のもの）は
列の値より優先されます。

```csv
id,name
1,Leanne Graham
2,Ervin Howell
```

### CI でのシークレット

シークレットは git に入りません（`.local.toml` ファイルは無視され、キーチェーンは自分の
コンピューターにあります）。そのためパイプラインは次の 3 つのいずれかの方法でシークレットを渡します。
後のものほど優先されます。

1. `--env-file .env`：`NAME=value` の行からなる dotenv ファイル（`#` のコメント、
   引用符、`export` が使えます）。
2. `APIX_VAR_<NAME>` 環境変数：`APIX_VAR_TOKEN` は `{{token}}` を埋めます。
   CI のシークレットはふつう大文字で書かれるので、名前は大文字小文字に関係なく、
   コレクションが定義する変数やリクエストが使う変数と照合されます。
3. `--var name=value`。

3 つとも環境の値より（そしてデータファイルの列より）優先されます。`apix` はどの名前をどこから
取ったかを stderr に出力しますが、値は決して出力しません。

```sh
APIX_VAR_TOKEN="$API_TOKEN" apix run api --env staging
```

### GitHub Actions

このリポジトリは GitHub Action でもあります。コレクションを実行し、JUnit と HTML の
レポートを書き出します。リリースタグで使うと、そのリリースの `apix` をダウンロードします（数秒）。
ブランチやコミットで使うと、初回に `apix` をビルドしてキャッシュします。`apix-version: latest`
（または `0.1.1`）を指定すると、代わりにリリースをダウンロードします。

```yaml
- uses: 11R5/Apix@v0.1.1        # リリース：apix はビルドせずダウンロード
  with:
    path: api
    environment: staging        # ほかに：data, iterations, delay, bail, env-file, junit, html, json, markdown, changed, diff, comment, args, apix-version
  env:
    APIX_VAR_TOKEN: ${{ secrets.API_TOKEN }}
```

[examples/ci/github-actions.yml](../../examples/ci/github-actions.yml) は完全なワークフローです。
環境ごとに 1 回ずつコレクションを実行し（`test` と `staging` のマトリックス）、JUnit の結果を
実行のページに表示し、HTML レポートを保存します。アクションはジョブのページに Markdown の
サマリーも追加します。

## 変更分だけ：git を考慮した実行

コレクションは git の中のフォルダーなので、`apix` は変更が何に影響したかを git に問い合わせられます。
`--changed` はそのリクエストだけを送信します。

```sh
apix run api --changed                 # まだコミットしていないもの
apix run api --changed=origin/main     # このブランチが main から分かれてから変えたもの
```

リクエストが再び実行されるのは、そのファイル自身やそのスクリプトが変わったとき、上位の
`folder.toml` やフォルダーのスクリプトが変わったとき（そのフォルダー内のすべてのリクエスト）、
`apix.toml`、コレクションのスクリプト、環境、プロジェクトファイルが変わったとき（すべてのリクエスト）、
または変更された別のファイル（ボディのファイル、`.proto`）を参照しているときです。新しいファイルは
コミット前から数えます。削除されたリクエストは一覧に出ます。値を指定しない場合、`--changed` は
GitHub Actions ではプルリクエストのベースブランチ（`origin/$GITHUB_BASE_REF`）と、それ以外では
`HEAD` と比較します。値は `=` を付けて書いてください。そうすれば実行するフォルダーと
取り違えられません。

```text
2 of 3 requests changed since origin/main
  users/create-user.apix  ← users/create-user.test.js
  users/list-users.apix
```

プルリクエストと同じく、比較はブランチがベースから分かれた地点から始まるので、その間に main に
入ったものは数えません。そのためには履歴が必要です。シャロークローン（`actions/checkout` の既定）では
`apix` は警告を出し、ベースそのものと比較します。`fetch-depth: 0` でチェックアウトしてください。
暗号化コレクションは比較できません。git には暗号化されたファイルしか見えないためです。

### 変更内容：`apix diff`

`apix diff` は、レビュアーが読むのと同じ形で変更を表示します。TOML の行ではなくリクエストの
フィールドとして、ヘッダーとクエリパラメーターは名前ごとに、ボディとスクリプトは行ごとに、
環境は変数ごとに示します。ファイルの書式を整えただけでは変更になりません。ほかの編集なしに
移動や名前変更をしたリクエストは、1 つの移動として扱われます。

```sh
apix diff                       # HEAD と現在のファイルを比較
apix diff main                  # main と現在を比較
apix diff v1.2 v1.3 -C api      # 2 つのタグの間、api/ にあるコレクションについて
```

```text
Between main and the working tree: 1 script, 1 request

A  script  users/create-user.test.js
     + pm.test("ok", () => pm.response.to.have.status(200));
M  GET List users  users/list-users.apix
     query._limit: 3 → 5
     headers.Accept: (none) → application/json
```

`--markdown` はプルリクエスト用に、`--json` はツール用に出力し、`--exit-code` は何かが
変わっていれば 1 で終了します。

### プルリクエストへのサマリー

`--markdown report.md` は実行結果を GitHub の Markdown として書き出します。合計、リクエストごとに
1 行、そしてその下に展開された形で、何がなぜ失敗したかが続きます。最初の行はコレクション名を含む
隠しマーカーなので、CI のステップは前回書いたコメントを見つけ、新しく追加する代わりにそれを
編集できます。

アクションはこれをすべて行います。プルリクエストでは、そのプルリクエストが影響するものだけを送信し、
サマリーの下に `apix diff` を加え、プルリクエスト上のコメント 1 つを最新の状態に保ちます。

```yaml
on: pull_request
permissions:
  contents: read
  pull-requests: write          # コメント用
jobs:
  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0        # ブランチの起点を見つけるための履歴
      - uses: 11R5/Apix@main
        with:
          path: api
          changed: "true"       # またはコミット、ブランチ、タグ
          diff: "true"
          comment: "true"
```

push では、`changed: "true"` は push 前のコミットと比較します。サマリーは
`apix-report.md`（入力 `markdown`）にも書き出され、ジョブのページに表示されます。

デスクトップアプリでは、ステータスバーにブランチと最後のコミット以降に変わったファイルの数が表示され、
サイドバーでは変更されたリクエストに **M**（変更）または **A**（新規）の印が付きます。
[ユーザーガイド](user-guide.md#git)を参照してください。

### Docker

ほかの CI システム向けに、`apix` コマンドだけを含むコンテナイメージがあります
（バージョンタグが付くと `ghcr.io/11r5/apix` に公開されます。自分でビルドするには
`docker build -t apix .`）。

```sh
docker run --rm -v "$PWD:/work" -e APIX_VAR_TOKEN ghcr.io/11r5/apix run api --env staging --junit reports/apix.xml
```

[examples/ci/gitlab-ci.yml](../../examples/ci/gitlab-ci.yml) と
[examples/ci/azure-pipelines.yml](../../examples/ci/azure-pipelines.yml) は、同じことを
GitLab CI（イメージを使用）と Azure Pipelines（`apix` を一度ビルドしてキャッシュ）で行います。
どちらも JUnit の結果を公開します。

デスクトップアプリの **Runner**（Ctrl+Shift+R）は、同じ実行をウィンドウで行います。
[ユーザーガイド](user-guide.md#runner)を参照してください。

`apix send <file>` は 1 つのリクエストを送信してレスポンスを出力します。ヘッダーを見るには `-i` を、
接続の詳細を見るには `--network` を付けます。

## モニターで API を監視する

モニターは、コレクション（またはその中のフォルダーやリクエスト）をスケジュールに従って実行し、
失敗し始めたときと再び成功したときに Webhook に知らせます。モニターはコレクションの
`apix.toml` に書きます。手で書くか、アプリの **表示 › モニター…** から書きます
（[ユーザーガイド](user-guide.md#モニター)を参照）。

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "health.apix"
environment = "production"
webhook = "{{alerts_url}}"
```

`apix monitor` は動かしている間ずっと、それぞれのスケジュールでモニターを実行し、
途中で `apix.toml` を読み直すので、編集が反映されます。

```sh
APIX_VAR_ALERTS_URL=https://hooks.slack.com/… apix monitor api
```

```text
Watching 2 monitor(s) of Shop API; logs in ~/.config/Apix/monitors (Ctrl+C stops)
09:30:05Z ✓ Health                 1 passed · 166 ms
                         next at 09:35:05Z
09:30:05Z ✗ Orders                 1 of 3 failed · 412 ms
    orders/list.apix: got 500 Internal Server Error
    webhook told: failing
```

| オプション | 動作 |
|---|---|
| `--once` | すべてのモニターを 1 回ずつ実行して終了します。1 つでも失敗すれば 1 で終了：cron、タスク スケジューラ、CI のスケジュール用 |
| `--list` | モニターと直近の結果 |
| `--only NAME` | このモニターだけ（繰り返し指定可）。`--once` と一緒なら、オフにしてあっても実行します |
| `--var`、`--env-file` | `apix run` と同じく、モニターの環境より優先される変数（`APIX_VAR_*` も読みます） |
| `--log-dir DIR` | ログを別の場所に保存します（CI ではキャッシュするフォルダー） |
| `--timeout`、`-k`、`--proxy`、`--cacert` | `apix run` と同じ通信設定 |

ログは各モニターの直近 500 件の結果を `<config>/Apix/monitors` に保存します。
デスクトップアプリも同じファイルを読むので、同じコンピューターで `apix monitor` が見つけたものを
表示します。Webhook に知らせるのは、モニターの状態が変わったときだけです（成功の後に失敗したとき、
または初回の実行で失敗したとき。失敗の後に成功したとき）。ボディは Slack、Discord、Mattermost、
Google Chat がメッセージとして表示する JSON です。

サーバーでは、`apix monitor api` をサービスとして起動するか、`apix monitor api --once` を
cron（`*/5 * * * *`）に登録します。GitHub ではスケジュール実行のワークフローがこれを行います。
[examples/ci/monitors.yml](../../examples/ci/monitors.yml) はアクション（`monitor: "true"`）を通して
15 分ごとにモニターを実行し、回復に気づけるようログをキャッシュします。

## OAuth 2.0 でサインインする

**クライアントクレデンシャル**（サービス間）：

1. **認証** タブ › **OAuth 2.0**、グラントタイプは **クライアントクレデンシャル**。
2. **トークン URL**、**クライアント ID**、**クライアントシークレット**、**スコープ** を入力します。
   シークレットをそのまま書く代わりに変数（`{{client_secret}}`、シークレットに指定）を使います。
3. 送信します。Apix はトークンを取得し、その後は再利用して更新します。

**PKCE 付きの認可コード**（ユーザーがサインインする場合）：

1. `http://127.0.0.1:<port>/callback` をクライアントのリダイレクト URL として登録します
   （プロバイダーが任意のループバックポートを受け付けるなら、**リダイレクト URL** は空のままにします）。
2. グラントタイプは **認可コード**。**認可 URL**、**トークン URL**、**クライアント ID**、
   **スコープ** を入力し、**PKCE を使用（推奨）** はチェックしたままにします。
3. **新しいトークンを取得** を押すか、送信します。ブラウザーでサインインページが開き、
   サインインするとトークンが Apix に表示されます。

OAuth 2.0 をコレクションに一度設定すれば（右クリック › **認証…**）、認証を継承するすべての
リクエストが同じトークンを使います。トークンはメモリ上にだけ保持され、Apix を再起動すると
もう一度サインインします。

## AWS の API を呼び出す

1. **認証** タブ › **AWS Signature**。
2. **アクセスキー** と **シークレットキー** は変数で（`{{aws_access_key}}`、
   `{{aws_secret_key}}`、シークレットに指定）、一時的な資格情報には **セッショントークン**、
   **リージョン**（たとえば `eu-central-1`）、**サービス**（たとえば `execute-api`、`s3`、
   `lambda`）を指定します。
3. 送信します。すべてのリクエストは、送信される直前に Signature Version 4 で署名されます。

**形式を指定してコピー › cURL** は `curl --aws-sigv4` を書き出すので、実行したときに curl が
改めて署名します。PowerShell のコピーにはコピーした時点で作った署名が含まれ、数分間有効です。

## ファイルのアップロードと GraphQL の送信

**Multipart：** **ボディ** › **Multipart** でフィールドを追加し、**ファイル** に切り替えてファイルを
選びます。テキストのフィールドは **テキスト** のままにします。テスト用のファイルはコレクション内に
置いてください。パスはリクエストからの相対パスで保存されるので、どのマシンでも動きます。

**バイナリ：** **ボディ** › **バイナリ** は 1 つのファイルをボディ全体として送ります。

**GraphQL：** **ボディ** › **GraphQL** でクエリを書き、その下に変数を JSON で書きます。
Apix は `Content-Type: application/json` で `{"query": …, "variables": …}` を送ります。
先に認証を設定してから **スキーマを取得** を押すと、以後クエリエディターがフィールド、引数、
列挙値を候補に出し、ホバーするとそのドキュメントが表示されます。スキーマは URL ごとに保存されるので、
次にリクエストを開いたときにはすでにそこにあります。プルリクエストでスキーマの変更を確認するには、
`apix schema api/graphql.apix > schema.graphql` をリポジトリに保存し、差分で何が変わったかを
見られるようにします。

## gRPC サービスを呼び出す

1. **リクエスト › 新しいリクエスト** を選び、`grpc://`（TLS の背後なら `grpcs://`）付きで
   アドレスを入力します。たとえば `grpc://localhost:50051`。
2. **メッセージ** タブで **メソッドを読み込む** を押します。サーバーリフレクションが
   オフの場合は、先に `.proto` ファイルを追加します（**ファイルを追加**）。パスがどのマシンでも
   動くよう、ファイルはコレクション内に置いてください。
3. メソッドを選び、**サンプルメッセージ** を押して値を入力します。
4. **認証**（Bearer）でトークンを設定するか、**ヘッダー** にメタデータを追加してから、
   **送信** します。

パイプラインでは、同じファイルがコマンドラインで使えます。

```sh
apix send api/say-hello.apix --env staging   # ステータスが OK でなければ終了コード 1
apix schema api/say-hello.apix > greeter.txt   # サービスが提供するもの。レビューで差分を見るため
```

## ブラウザーのリクエストを再現する

1. ブラウザーの開発者ツールで、リクエストを右クリック › **Copy as cURL**
   （bash でも cmd でも構いません）。
2. Apix で URL 欄をクリックして貼り付けます。メソッド、URL、クエリ、ヘッダー、Cookie、
   認証、ボディが入力されます。
3. 送信し、変更し、保存します。

逆方向には、**送信** の隣にあるコピーボタンでリクエストを `curl`、`curl.exe`、PowerShell として
取得でき、ターミナルやバグ報告に貼り付けられます。

## リクエストが遅い理由や失敗する理由を調べる

1. リクエストを送信し、レスポンスの **ネットワーク** タブを開きます。
2. タイミングバーを見ると、時間が DNS、TCP 接続の確立、TLS ハンドシェイク、サーバーの待ち、
   ダウンロードのどこにかかっているかが分かります。TCP 接続が遅ければネットワークが原因です。
   ハンドシェイクが遅ければ（1 往復で済む TCP と比べて）、サーバーか TLS 1.2 の余分な往復が原因です。
3. リモートアドレスとローカルのインターフェースを確認します。VPN やプロキシはそこに現れます。
4. リダイレクトは順に一覧されます。**暗号化** の下には TLS のバージョン、暗号スイート、
   HTTP/2 が合意されたかどうかが表示されます。証明書チェーンには各証明書の発行者と残り日数が
   表示されるので、中間証明書の欠落や期限切れ間近の証明書が目立ちます。圧縮されたボディでは、
   どれだけ節約できたかが表示されます。

同じレポートはターミナルでも見られます。

```sh
apix send my-api/health.apix --network
```

## AI エージェントにコレクションを使わせる（MCP）

`apix mcp` は Model Context Protocol でコレクションを提供します。これにより AI エージェント
（Claude、エディターのアシスタント、任意の MCP クライアント）が、あなたが Apix で行うのと
同じように API を扱えます。標準入出力で MCP を話し、エージェントがそれを起動します。
Claude Code の場合：

```bash
claude mcp add my-api -- apix mcp /path/to/my-api
```

ほかのクライアントでは、同じコマンドを設定に書きます。たとえば：

```json
{ "mcpServers": { "my-api": { "command": "apix", "args": ["mcp", "/path/to/my-api"] } } }
```

エージェントは 4 つのツールを使えます。

| ツール | 動作 |
|---|---|
| `list_requests` | コレクション全体またはフォルダーのリクエスト（パス、メソッド、名前、`{{variables}}` 付きの URL） |
| `list_environments` | 環境とその変数の名前。値は決して返しません |
| `send_request` | 1 つのリクエストを、スクリプトとテスト付きで、ある環境で、エージェントが設定した変数を使って送信します。ステータス、時間、ヘッダー、ボディ（最大 64 KB）、テスト結果を返します |
| `run_requests` | フォルダーまたはコレクション全体をテスト付きで実行し、何が成功したかを報告します |

届くのはそのコレクション内のリクエストだけです。`--list-only` を付けると一覧の 2 つのツールだけを残し、
送信するツールを外します。シークレットはその場所から動きません。エージェントはトークン付きで
リクエストを送るよう頼めますが、トークンの値を見せられることはありません（ただしそれをそのまま
返すレスポンスは別問題なので、エージェントに何を送らせるかには注意してください）。
暗号化コレクションでは、パスワードを `APIX_PASSWORD` で渡します。
