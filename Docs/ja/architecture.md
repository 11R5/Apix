<!-- translated from en/architecture.md sha256:75b4d247b3b61d0d -->

# Apix のアーキテクチャ

Apix は Rust で書かれたネイティブの API クライアントです。UI は
[GPUI](https://github.com/zed-industries/zed)（Zed の UI エンジン）と
[gpui-kit](https://github.com/longbridge/gpui-kit) コンポーネントライブラリによって GPU 上に
描画されます。WebView も Electron も使いません。

## 原則

1. **ローカルファースト、git と相性がよい。** コレクションは小さな TOML ファイルが入ったフォルダーです。
   アカウントもクラウドも必要ありません。コレクションはコードと同じようにコミットし、
   差分を取り、レビューします。
2. **UI は決してブロックしない。** ネットワーク、ディスク、解析、スクリプトは UI スレッドの外で
   実行されます。UI スレッドはレイアウトと描画だけを行います。
3. **1 つのエンジン、多くのフロントエンド。** デスクトップアプリ、`apix` CLI、将来のフロントエンド
   （TUI、MCP サーバー）は同じクレートを共有するので、リクエストは CI を含むどこでも同じように動きます。
4. **巨大なレスポンスにも対応する。** 100 MB の JSON レスポンスも滑らかにスクロールできなければ
   なりません。レイアウトするのは見えている行だけで（仮想リスト）、ボディは遅延して解析します。
5. **軽く、速く起動する。** 速度と機能はサイズより優先します。リリースバイナリは 100 MB 未満に
   収め（現在 54.4 MB。Electron アプリは 300 MB 以上）、150 ms 未満でウィンドウを開きます。

## クレート

```
crates/
├── apix-core     リクエストモデル、{{variables}} と {{$dynamic}} の値、ディスク上の形式、
│                 プロジェクト、環境とシークレット、ツリーとファイルの操作、
│                 GraphQL スキーマ（イントロスペクション、SDL）とクエリの補完、
│                 git：ブランチ、変更されたファイル、変更が影響するもの、リクエスト単位の差分
├── apix-http     HTTP エンジン：reqwest + rustls、タイミング、レスポンスの取得、ネットワークトレース、
│                 server-sent events、WebSocket（tokio-tungstenite）、Socket.IO、GraphQL スキーマの取得
│                 （DNS、接続の確立、リダイレクト、証明書、ローカルインターフェース）
├── apix-import   Postman、Insomnia、OpenAPI、HAR、WSDL → Apix のコレクション
├── apix-codegen  リクエスト ↔ シェルコマンド：curl（bash）、curl.exe（cmd）、PowerShell。貼り付けた curl を解析。10 言語のコード
├── apix-vault    暗号化コレクション：ファイルごとの XChaCha20-Poly1305、Argon2id、回復キー
├── apix-script   プリリクエストスクリプトとテストスクリプト：pm.* 互換 API を持つ QuickJS
├── apix-runner   コレクションの実行：順序、反復（並行も）、データファイル、遅延、bail、
│                 レポート（プルリクエスト用の Markdown も）、モニター：スケジュール、ログ、Webhook
├── apix-grpc     gRPC 呼び出し：tonic、JSON としてのメッセージ（prost-reflect）、.proto ファイル（protox）、コードまたは JSON からの定義
│                 またはサーバーリフレクション
├── apix-proto    その他のプロトコル：MQTT（rumqttc）
├── apix-mock     保存済みの例から応答するローカルモックサーバー（hyper）
├── apix-capture  記録用プロキシ：普通の HTTP、CONNECT トンネル、ローカル CA で開く HTTPS（rcgen、rustls）
├── apix-route    サーバーまでの経路（システムの traceroute）と各ホップの場所（オフラインの .mmdb データベース）
├── apix-plugins  WebAssembly プラグイン（wasmi）：認証の種類、インポーター、レスポンスのビュー。apix-plugin-sdk で Rust で書く
├── apix-cli      `apix` バイナリ：ターミナルと CI から send / run / diff / schema / import / shell
└── apix-desktop  GPUI デスクトップアプリ（1 つのクレートから Windows、macOS、Linux）
tools/
└── icongen       Logo/LogoFirst.jpg からすべてのアプリアイコン（ico、icns、hicolor PNG）を作る
```

`apix-desktop/src` の中身：`workspace.rs`（リクエストエディター、レスポンスパネル、
アクションとキーバインド）、`workspace/collections.rs`（サイドバー、プロジェクト、
ファイル操作、インポート、ファイル監視）、`workspace/env_editor.rs`（変数ダイアログ）、
`workspace/auth_editor.rs`（**認証** タブと、フォルダー・コレクション・プロジェクトの認証ダイアログ）、
`workspace/palette.rs`（コマンドパレットとそのあいまい一致）、`workspace/network.rs`
（レスポンスの **ネットワーク** タブ）、`workspace/runner_ui.rs`（Runner）、`workspace/vault_ui.rs`
（ロック解除、ロックと自動ロック、暗号化、復号、パスワード変更、回復キー）、`assets.rs`
（gpui-kit のアイコンと 2 つのロックアイコン）、`workspace/menu_bar.rs`（タイトルバーの
ファイル／リクエスト／環境／表示／ヘルプ。ボタンはタイトルバーのドラッグ領域を `occlude()` するので、
Windows はクリックをボタンに渡します）、`workspace/tabs.rs`（開いているリクエスト。前面のタブは
エディターのウィジェットに置かれ、ほかのタブは下書き、レスポンス、実行中のタスクを保持します）、
`workspace/drag.rs`（リクエストやフォルダーをドラッグして移動）、`workspace/shell_copy.rs`
（形式を指定してコピー／curl の貼り付け）、`workspace/var_chips.rs`（URL バーの `{{variable}}` チップと
そのホバーカード）、`workspace/graphql_ui.rs`（GraphQL スキーマの取得、キャッシュ、補完、ホバー）、
`workspace/grpc_editor.rs` と `workspace/grpc_ui.rs`（gRPC リクエストの **メッセージ** タブと、
レスポンス欄での呼び出し）、`large_view.rs`（巨大なボディ用の行ベースのビューアー）、`net.rs`
（tokio とのブリッジ）、`bidi.rs`（右から左へのレイアウトの補助。「言語」を参照）、`state.rs`
（ユーザーの設定フォルダーに置く最近の項目とフォルダーごとの環境）、`theme.rs`、`fonts.rs`。

### 3 つの OS に 1 つのデスクトップクレート

GPUI と gpui-kit は同じコードで Windows、macOS、Linux で動きます。Zed もそうやって配布しています。
そのため OS ごとの UI フォルダーはありません。OS によって本当に異なるコードは
`crates/apix-desktop/src/platform/{windows,macos,linux}.rs` の `cfg` の後ろに置きます：
タイトルバーとウィンドウのコントロール、ぼかし（blur/vibrancy）、macOS のメニューバー、
`.apix` のファイル関連付け、キーチェーンへのアクセス。インストーラーとアイコンは
`packaging/{windows,macos,linux}/` にあります。UI を別に分けるのは、ある OS が本当に異なる
インターフェースを必要としたときだけです。

依存の方向：`core` ← `http`/`script`/`proto` ← `cli`/`desktop`。
`desktop` に依存するものはありません。

## リクエストのデータの流れ

```
.apix file ──load──▶ Request ──resolve(Variables)──▶ Request (resolved)
                                                        │
                       pre-request script (later) ◀─────┤
                                                        ▼
                                         apix-http::HttpClient::send
                                                        │  (tokio runtime, background thread)
                                                        ▼
                    Response {status, headers, body: Bytes, timing}
                                                        │
                           test script (later) ◀────────┤
                                                        ▼
                         UI: status line, virtualized body viewer
```

`{{variables}}` は層ごとに解決されます。後の層が優先されます：
**グローバル → コレクション（`apix.toml`）→ 環境 → 上書き（CLI の `--env-file`、`APIX_VAR_*`、`--var`。大文字小文字に関係なくコレクションの名前と照合。スクリプトの `pm.variables.set`）**。
定義されていない名前はそのまま残して報告するので、UI では下線を引け、CLI では送信を拒否できます。

### ネットワークトレース

送信のたびに短命な reqwest クライアントを作るので、記録したものはすべてそのリクエストのものです
（`apix_http::trace`）。リゾルバーが DNS の時間を計り、reqwest のコネクターを包む Tower レイヤーが
各接続の時間を計り、リダイレクトポリシーが各ホップを記録します。reqwest は TLS 接続を外に渡さず、
独自のコネクターが TLS 接続を返すことも許さないので、TLS の詳細は rustls の内部から、Apix が
自分で組み立てるクライアント設定を通して取得します（`use_preconfigured_tls`、
`tls::traced_config`、`tls_trace.rs` のフック）。

- 包み込む形の証明書検証器が、サーバーが送ってきたチェーンと、どのバージョンの署名
  （TLS 1.2 または 1.3）を確認するかを記録し、その後で通常の検証器（チェックを切っている場合は
  「すべて受け入れる」）に判断を任せます。
- 何も保存しないセッションストアは、client hello を書いている間に鍵交換のヒントを求められます。
  これが TLS ハンドシェイクの開始の印になります。接続の時間のうち、それより前（DNS の後）が TCP、
  後が TLS です。また、サーバーが選んだグループ（TLS 1.3）を知らされます。
- 暗号スイートは rustls 自身のもので、その暗号化を薄いラッパーで包んで一度だけ作り、どのスイートが
  設定されたかを記録します。スイートは `'static` で共有されるので、ラッパーはリクエストの future が
  poll されている間に設定されるスレッドローカル（`Scoped`）を通してリクエストを見つけます。
  ハンドシェイクはその poll の中で実行されます。

ALPN はレスポンスのバージョンから分かります（TLS 上の HTTP/2 は `h2` が合意された後だけ）。
圧縮されたボディは reqwest ではなく Apix が展開します（`decode.rs`。gzip、zlib または raw deflate、
brotli を届いた順に）。reqwest に任せると `Content-Encoding` と転送時のサイズが失われるためです。

## ディスク上の形式

```
my-api/
├── apix.toml               名前 + コレクションレベルの [vars] と [auth]
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── folder.toml         任意：このフォルダー内のリクエストの [auth]
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

**認証の継承。** `auth` のないリクエストは、`folder.toml` で認証を設定している最も近いフォルダーから、
次にコレクション（`apix.toml`）から、次にプロジェクト（`apix-project.toml`）から継承します。
どこにもなければ何も送りません。リクエスト（またはフォルダー）の `auth = "none"` は継承を止めます。
`apix_core::project::inherited_auth` が、デスクトップアプリ、CLI、「形式を指定してコピー」の
すべてのために検索を行います。

**リクエストの設定。** `[settings]` テーブル（タイムアウト、リダイレクト、TLS の検証、プロキシ、
クライアント証明書、信頼する CA、Cookie）も同じ 4 つのレベルに置けます。各フィールドは `Option` です。
`None` はフィールドごとに同じ連鎖をたどって継承し（`apix_core::project::inherited_settings`）、
それでも未設定のものは `RequestSettings::effective` の既定値になります：タイムアウトなし、
リダイレクトは 10 回まで追従、TLS を検証、プロキシは環境から、Cookie はオン。エンジンは各送信の
`reqwest::Client` を作るときにこの結果を適用します（信頼する CA のファイルはその証明書をルートとして
追加します）。共有の Cookie ジャー（Cookie マネージャーが一覧・編集できる `cookie_store`）は、
リクエストが Cookie をオフにしていない限りそのクライアントに渡されます。

**署名する認証。** AWS Signature V4、OAuth 1.0a、Digest は `apix_core::sign` の純粋関数で、
AWS のテストスイート、RFC 7616、RFC 7636、古典的な OAuth 1.0 の例で検証されています。そのため
エンジンと「形式を指定してコピー」は同じ方法で署名します。エンジンはまず `reqwest::Request` を
組み立て、それから署名します（AWS のペイロードハッシュには最終的なボディが必要です。multipart の
ストリームは `UNSIGNED-PAYLOAD` として送ります）。Digest は一度送信し、401 のチャレンジを読み、
同じ接続で応答します。OAuth 2.0 は `apix_http::oauth2` にあります。トークンは送信前に取得され
（クライアントクレデンシャル、パスワード、または PKCE 付きの認可コード。認可コードではブラウザーが
127.0.0.1 上の一度きりのリスナーに戻ってきます）、サーバー、クライアント、グラント、スコープ、
ユーザーごとにメモリに保持され、リフレッシュトークンで更新され、ディスクには決して書き込まれません。

リクエストファイル：

```toml
name = "Create user"
method = "POST"
url = "{{base_url}}/orgs/:org/users"   # クエリなし：それは [[query]] にある

[[query]]
key = "notify"
value = "yes"

[[path]]                 # :org セグメントの値
key = "org"
value = "{{org}}"

[[example]]              # リクエストと一緒に保存したレスポンス
name = "Created"
status = 201
body = '{ "id": 7 }'
[example.request]        # 送信元のリクエスト（書いたとおり）
method = "POST"
url = "{{base_url}}/orgs/:org/users?notify=yes"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false          # ファイルには残すが送信しない

[body]
json = '''
{ "name": "Ada" }
'''

[auth.bearer]
token = "{{token}}"

[[assert]]               # コードなしのチェック。テストスクリプトの後に実行
target = "status"
op = "=="
value = "201"
```

**チェック。** `[[assert]]` の行は、対象（`status`、`time`、`size`、`body`、`body.<path>`、
`header.<name>`）を値と比較します。使える演算子は `==`、`!=`、`>`、`>=`、`<`、`<=`、`contains`、
`not contains`、`exists`、`not exists`、`matches`、`is`（JSON の型）です。
`apix_script::assertions_script` がこれらをもう 1 つのテストスクリプトに変換するので、どこでも
`pm.test` と同じように報告されます。

**ボディ。** `[body]` には `json`、`text`、`xml`、`form`（URL エンコードの行）、`multipart`
（ファイルのフィールドには `file = true`、任意で `content_type` を持つ行）、`binary = { file = "…" }`、
`graphql`（`query` と `variables`）のどれか 1 つが入ります。ファイルパスには変数を含められ、
リクエストファイルからの相対パスです。`Request::with_files_relative_to` が変数の解決後にそれらを
絶対パスにします。アプリでも CLI でも同じです。エンジンは送信するときにだけファイルを読むので、
ファイルがないことは読み込みのエラーではなく送信のエラーになります。GraphQL のボディは
`{"query", "variables"}` の JSON として送られ、変数が不正なら送信を止めます。multipart のボディでは、
エンジンの boundary を含まない `Content-Type: multipart/…` ヘッダー（Postman のエクスポートによくある）
は捨てられるので、ボディは読める状態に保たれます。

**gRPC。** URL が `grpc://host:port`（TLS なら `grpcs://`）のリクエストは、`[grpc]` でメソッドを
指定します。JSON のボディがメッセージで、メッセージのストリームを受け取るメソッドでは JSON のリストが
メッセージのストリームになります。`protos` がなければサーバーに問い合わせます（リフレクション）。
パスはリクエストファイルからの相対パスです。

```toml
name = "Say hello"
url = "grpc://localhost:50051"

[body]
json = '{"name": "Ada"}'

[grpc]
method = "helloworld.Greeter/SayHello"
protos = ["../protos/helloworld.proto"]   # 任意
import_paths = ["../protos"]              # 任意
```

ファイルの代わりに、`definition` は説明をリクエストの中に保持します：`.proto` のテキスト、
または JSON のディスクリプターセット。これは `protos` より優先されます。`guessed = true` は、
型や JSON から推測したものであることを示します：サーバーリフレクションがそれより優先され、
違いが知らされます。`protos` はディスクリプターセット（バイナリまたは `.json`）も受け付けます。

```toml
[grpc]
method = "shop.v1.Orders/Create"
guessed = true
definition = """
syntax = "proto3";
package shop.v1;
service Orders { rpc Create(CreateRequest) returns (CreateResponse); }
message CreateRequest { string customer = 1; }
message CreateResponse {}
"""
```

検討事項：
- **シークレット。** トークンのような環境の値はコミットすべきではありません。
  `environments/<name>.local.toml` が `<name>.toml` の上に重ねられます。これを書き込むと、
  `environments/` の隣の `.gitignore` に `*.local.toml` がなければ追加します。シークレットは
  共有ファイルにプレースホルダーを残せるので、チームのメンバーには名前が見えます。
  オプションで（**設定 › 環境のシークレット**）シークレットを代わりに OS のキーチェーンに置けます。
  そのとき `<name>.local.toml` には `keychain = "<id>"` だけが入り、値はサービス `Apix` の
  エントリ `env:<id>` に置かれます（`apix_vault::keychain::save_text`。Windows の 2560 バイトの
  上限のため複数のエントリに分割）。`apix_core::secrets::use_keychain` が新しいシークレットの
  書き込み先を決めます。読み込みはファイルに従うので、CLI には設定が要りません。id はファイルと
  一緒に移動するので、環境の名前を変えたりフォルダーを移動したりしてもつながりは保たれます。
  暗号化コレクションの中では、シークレットは vault の中にとどまります。
- **動的変数。** `{{$name}}` は、その名前の変数が定義されていないときに `apix_core::dynamic` が
  埋め、出現するたびに新しい値になります（`$guid`、`$timestamp`、`$isoTimestamp`、`$randomEmail`、…）。
  追加のクレートは使いません：小さな xorshift 生成器と手書きの UTC 日付フォーマッターです。
- **サイドバーの並び順。** 何も保存されていなければ、フォルダーが先、次にリクエストで、それぞれ
  名前順です。ドラッグすると、子の名前の `order` リストがフォルダーの `folder.toml`（または
  コレクションの `apix.toml`）に保存されます。`store::place` はある項目を別の項目の前または後ろに
  置き、名前の変更や移動はリストを追従させます。リストにない名前は既定の順で後に続くので、
  新しいファイルが消えることはありません。プロジェクトのコレクションは `apix-project.toml` の
  `order` を使います（`Project::place_collection`）。Postman のインポーターは、Postman の順序が
  既定と異なるときだけ `order` を書きます。

### プロジェクト

プロジェクトはコレクションをまとめ、そのすべてで共有する環境を持ちます。

```
shop/
├── apix-project.toml       名前、共有の [vars]、コレクションの順序、group_by、pins、colors
├── environments/           test.toml、prod.toml（+ *.local.toml のシークレット）
├── users/                  コレクション（apix.toml + .apix ファイル）
└── payments/               コレクション。一部の環境に固定できる
```

`[colors]` はコレクションのフォルダーを色の名前（`red`、`orange`、`yellow`、`green`、`teal`、
`blue`、`purple`、`pink`）に対応付けます。アプリはそれをサイドバーとタブに点として描きます。
`project::move_collection_into` はコレクションのフォルダーをプロジェクトの中に移動します
（ドライブをまたぐ場合はコピーして削除）。

変数の層（後が優先）：プロジェクトの変数 → コレクションの変数 → プロジェクトの環境
→ 同じ名前のコレクションの環境（それぞれ `.local.toml` のシークレット付き）。
CLI（`apix run --env`）とデスクトップアプリは同じ関数 `apix_core::project::variables` を使います。
`group_by = "environment"` はサイドバーをプロジェクト → 環境 → コレクションの形に並べ、
`[pins]` はコレクションを一部の環境に限定します。

### 暗号化コレクション（M4）

形式とエンジンは `crates/apix-vault` にあります。Apix のほかの部分は `apix_core::vfs` を通して
それらに到達します。これは `std::fs` と同じ呼び出しを持つ薄い層で、ロック解除された vault の
ルート以下のパスは vault からメモリ上で復号して提供し、それ以外のパスはディスクに行きます。
そのためストア、サイドバー、タブ、CLI は普通のパスを使い続けられ、復号したものがディスクに
書き込まれることは決してありません。ロック解除された vault はプロセス全体のレジストリに置かれます。
ロック（手動、15 分間操作がないとき、または終了時）は vault を破棄し、その鍵を消去します。
暗号化コレクション内での削除は完全な削除です：システムのゴミ箱には復号されたファイルが残ってしまう
ためです。

```
my-api/
├── apix-vault.toml   平文のヘッダー：形式、Argon2id のコストとソルト、
│                     パスワードと回復キーでラップしたデータ鍵
├── .gitignore        objects/local/
└── objects/
    ├── index.apixe   すべてのファイルの実際のパス（暗号化済み）
    ├── 3f9c…e1.apixe 暗号化したファイル 1 つずつ。ランダムな名前、フラット（フォルダー構造なし）
    └── local/        *.local.toml のシークレット。同じ形式で、決してコミットしない
```

- ランダムな 256 ビットの**データ鍵**が、すべてのファイルを XChaCha20-Poly1305 で、書き込みごとに
  新しい nonce を使って暗号化します。オブジェクト id が関連データなので、blob を気づかれずに
  入れ替えたり名前を変えたりはできません。
- データ鍵は 2 回保存されます：**パスワード**から導いた鍵（Argon2id、64 MiB、3 パス）でラップした
  ものと、**回復キー**（160 ランダムビット。Crockford base32 の `XXXX-XXXX-…` として一度だけ表示）から
  導いた鍵でラップしたものです。パスワードを変更すると鍵をラップし直します。ファイルは再暗号化
  しません。
- 名前の変更と移動は、暗号化されたインデックスだけを変更します。
- ロック解除した鍵はメモリ上にだけ置かれます（ロック時に `zeroize` が消去します）。
- git にはどのオブジェクトが変わったかは見えますが、その中身は決して見えません。安全性は暗号と
  パスワードによるもので、形式が秘密であることによるものではありません。Apix はオープンソースです。
- パスワードは OS の資格情報ストアに記憶させられます（`apix_vault::keychain`。`keyring` クレート
  経由：資格情報マネージャー、macOS キーチェーン、Secret Service）。サービス `Apix` の下に vault ごとに
  1 つのエントリがあり、名前は `vault:<absolute folder>`（Windows では小文字）です。ロック解除の
  プロンプトで選んだときだけ使われます。アプリと CLI は尋ねる前にそれを試し、もう vault を開けない
  パスワードは削除して改めて尋ねます。パスワードを変更すると更新され、コレクションを復号すると
  削除されます。ストアが使えないときは、Apix はこれまでどおり単に尋ねます。

## スクリプト（M5）

`apix_core::scripts` はリクエストのスクリプトを見つけ（`_collection.<event>.js`、各フォルダーの
`_folder.<event>.js`、そして `<request>.<event>.js`。イベントは `prerequest` と `test`）、
スクリプトがスコープごとに設定・削除したものである `ScriptVars` を保持します。`ScriptVars::apply` は
それらをリクエストの `Variables` の上に重ねるので、解決はこれまでどおりに動きます。スクリプトが
設定したものはディスクに書き込まれません。

`apix-script` は 1 つのイベントのスクリプトを新しい QuickJS ランタイムで実行します（`rquickjs`。
上限は 10 秒と 256 MB）。`pm` オブジェクトは JavaScript（`prelude.js`）です。Rust はコンテキストを
JSON で渡し（未解決のリクエスト、テスト用のレスポンス、`project::scope_values` からのスコープの値、
それまでの `ScriptVars`、`pm.info`）、スクリプトは関数で包まれて 1 つずつ実行され、
`__apix_output()` が変更されたリクエストと変数、テスト結果、コンソールの行、`setNextRequest`、
`skipRequest` を JSON で返します。API を JavaScript 側に置くことで、Rust 側は少数の呼び出しで済み、
Postman の動作にも合わせやすくなります。

`pm.sendRequest` はリクエストを JSON にしてネイティブの `__apix_send` を呼びます。
`apix_script::run` の呼び出し元は、それを送信してレスポンスが届くまでブロックする `Sender` を渡します
（CLI はスクリプトのスレッド上の新しいランタイムで、アプリはそのネットワークランタイムで）。
スクリプトは async 関数で包まれるので、コールバック形式と `await` の両方が使え、待っている時間は
上限に数えられません。`require` は `libs.js` から来ます：crypto-js（ダイジェストと HMAC は
`__apix_hash` / `__apix_hmac` を通して Rust で計算）、uuid、lodash、moment、ajv、tv4、chai の
サブセットで、ダウンロードしたものではなく Apix のために書いたものです。

スクリプト付きの送信：プリリクエストスクリプト、次に `ScriptVars::apply` と解決、HTTP リクエスト、
そして `ResponseView` を使ったテストスクリプト。`apix_runner::exchange` が `apix send`、`apix run`、
Runner のためにこれを行います。デスクトップアプリの単発の送信は `script_run.rs` でこれを行い、
スクリプトはバックグラウンドスレッドで、リクエスト自身のスクリプトは保存済みかどうかに関係なく
エディターから取ります。

`apix_runner::run` は `Plan`（順に並んだリクエストファイル、変数とスクリプトのスコープ、データ行、
反復回数、遅延、`bail`、通信設定の上書き、開始時のスクリプトの状態）を受け取って処理します：
反復ごとにデータ行（次に `--var` の値）を変数と `pm.iterationData` の上に重ね、リクエスト名で
`setNextRequest` に従い、リクエストの間で待ち、`Progress`（反復、送信中、項目完了）をコールバックに
報告します。`Control` が別のスレッドから一時停止や停止を行います。戻り値は `ItemResult` の
`RunReport` で、`report.rs` がそれを JSON、JUnit XML（リクエストと反復ごとに 1 つのスイート、
テストごとに 1 つのケース、テストのないリクエストにはステータスのケース）、単体で完結した HTML ページ
として書き出します。`data.rs` は CSV（RFC 4180、BOM、`;` のヘッダー）と JSON の配列を読みます。
CLI は進捗を届いた順に出力します。デスクトップの Runner（`workspace/runner_ui.rs`）はネットワーク
ランタイムで実行し、共有の `Live` 構造体から 1 秒に数回進捗をコピーします。ビューはワークスペースに
残るので、実行はダイアログより長く続けられます。

## ストリームと WebSocket（M6）

`HttpClient::send_with` は `Progress::Head`（ステータスとヘッダー）を届いた時点ですぐ報告し、その後は
ボディの `Progress::Chunk` を 1 つずつ報告します。`send` は誰も聞いていない `send_with` で、
レスポンスにはボディ全体が入ります。`apix_http::sse` は `text/event-stream` のボディを少しずつ
読みます（`SseParser`：どの改行でも可、断片はどこで分かれてもよい、`id` は引き継がれる、最後の
未完了のイベントは捨てる）。パーサーは再接続のために最後の id と最後の `retry:` も保持します：
`reconnected()` は両方を持った次の接続用のパーサーを作り、`reconnect_request` は `Last-Event-ID` を
追加します。待機と再送信はアプリと CLI が自分で行うので、Esc や Ctrl+C でいつでも止められます。

`apix_http::ws::connect` は、HTTP 側が送るのと同じようにリクエストからハンドシェイクを組み立て
（`prepare`：パラメーター、ヘッダー、認証。OAuth 2.0 は先に Bearer トークンになります）、
`[websocket] subprotocols` から `Sec-WebSocket-Protocol` を追加し、tokio-tungstenite で接続します。
`wss://` は `apix_http::tls::client_config` から rustls の設定を受け取ります（通常のルートに加えて
信頼する CA、クライアント証明書、またはどの証明書でも受け入れる検証器）。gRPC が検証なしで使うのと
同じモジュールです。`WsConnection::run` は `WsCommand` のチャネルが求めるものを送信し、どちらかの
側が閉じるまで `WsEvent`（送信、受信、コードと理由付きの切断、エラー）を報告します。サーバーからの
ping には tungstenite が応答します。`[websocket] ping_interval` があればこちらからも ping を送り、
カウンターをペイロードにして、対応する pong の往復時間を報告し（`WsEvent::Pong`）、次の ping の時刻に
なってもまだ応答のない ping があればエラーで終了します。メッセージの `MessageFormat`（text、base64、
hex）は、変数を埋めた後でテキストを `WsData` に変換します。

デスクトップアプリでは、ネットワークランタイムが共有の構造体（送信には `net::Incoming`、ソケットには
`net::SocketIncoming`）を埋め、UI のタスクが 60〜80 ms ごとにそれを読みます。イベントストリームの
レスポンスは、ライブのイベント一覧を持つ `ResponseState::Streaming`（`workspace/stream_ui.rs`）に
なり、上にスクロールしていない限り新しいイベントを追います。Esc を押すと、届いたものが完了した
レスポンスになります。`ws://` の URL はリクエスト欄を **メッセージ** タブ（`workspace/ws_editor.rs`）に、
レスポンス欄を `ResponseState::Socket`（`workspace/socket_ui.rs`）に切り替えます。どちらの状態も
完了したレスポンスと同じようにそのタブに振り分けられるので、ストリームやソケットは背後のタブで
動き続けます。CLI は同じコールバックからイベントとメッセージを出力します。

## Socket.IO（M6）

`apix_http::socketio` は上記の WebSocket 接続の上で Socket.IO 3/4（Engine.IO プロトコル 4）を
話すので、ハンドシェイク、ヘッダー、認証、TLS の設定は同じです。`[socketio]` テーブル
（`SocketIoOptions`：名前空間、パス、認証）を持つリクエストが Socket.IO です。`handshake_request` は
`http(s)://` を `ws(s)://` に変え、名前空間をオプションまたは URL のパスから取り、その場所に
サーバーのパス（`/socket.io/`）を置き、`EIO=4&transport=websocket` を追加します。`connect` は
Engine.IO の open パケットを待ち、名前空間の CONNECT を認証の JSON とともに送り、応答（ソケット id
付きの `40…`、または拒否付きの `44…`）を待ってから戻るので、サーバー、名前空間、トークンが
間違っていればハンドシェイクの失敗と同じように失敗します。

`SioConnection::run` は `WsConnection::run` を、2 つのチャネルの間の変換器と並べて実行します：
`SioCommand::Emit` は `EVENT` パケット（`42…`。求められれば ack id 付きで、id は 0 から数える）
または `BINARY_EVENT`（`45n-…`）になります。後者では `{"$binary": "<base64>"}` の値がプレース
ホルダーに置き換えられ、その後にバイナリフレームとして送られます。受信したパケットは `SioEvent`
（サーバーが求める ack id 付きの受信イベント、こちらの ack、サーバーによる退出、読めないフレーム）
になり、バイナリ添付は集められてプレースホルダーがあった場所に戻されます。Engine.IO の ping（`2`）
には pong（`3`）を返します。

デスクトップアプリは両方で 1 つの **メッセージ** タブを使います：`WsEditor` には
WebSocket/Socket.IO の切り替えと Socket.IO のフィールドがあり、`WsMessage` は `event` と `ack` を
持ちます。`net::SocketIncoming` はイベントの型に対してジェネリックで、`socket_ui.rs` は 1 つの
ループで両方の種類の接続を追い、`SioEvent` をイベント名と ack のチップ付きのログエントリに
変えます。`apix-http/tests/common/socketio_server.rs` のテストサーバーは、`socketio_server` の
サンプル（:8771）でもあります。

## MQTT（M6）

`apix_proto::mqtt` は rumqttc のイベントループを包みます（MQTT 3.1.1。TLS は
`apix_http::tls::client_config` からの設定だけを使うので、独自の暗号プロバイダーは持ち込みません）。
`grpc://` が gRPC にするのと同じように、`mqtt://` または `mqtts://` の URL はリクエストを MQTT に
します。`[mqtt]` にはクライアント id、セッション、keep alive、購読が入ります。`connect` はブローカーの
CONNACK までイベントループを poll するので、ログインが拒否されればハンドシェイクの失敗と同じように
失敗します。

`MqttConnection::run` は 2 つの future を並べて実行し、イベントループの poll を決してキャンセル
しません：一方は `MqttCommand` をクライアントに渡し（キューが満杯なら待つことがあり、そのキューは
poll によって空になります）、もう一方は接続を追って `MqttEvent` を報告します。rumqttc は送出する
各パケットのパケット id を渡された順に報告するので、キューが publish と購読をその id と対応付け、
ブローカーの PUBACK、PUBCOMP、SUBACK、UNSUBACK を、それらが確認するものと対応付けます。メッセージは
そのパケットが送出された時点で送信済みとして報告されます。不正なトピックや拒否された購読は
イベントとなり、接続は続きます。切断とブローカーからの切断は接続を終わらせます。

デスクトップアプリでは `WsEditor` が MQTT のフィールドも持ち（URL が決め、`Workspace` がそれを
伝えます）、`socket_ui.rs` は WebSocket や Socket.IO と同じループで接続を追い、`MqttEvent` を
トピックごとのログ行に変えます。`apix-proto/tests/common/broker.rs` のテスト用ブローカー
（ワイルドカード、QoS 0〜2、保持メッセージ、拒否、任意のストリーム上の TLS）は、`mqtt_broker` の
サンプル（:8772）でもあります。

## SOAP（M6）

SOAP はリクエストの種類を増やしません：`apix_import::wsdl` が WSDL 1.1 を XML ボディを持つ普通の
POST リクエストに変換するので、リクエストで使えるものはすべてそれらでも使えます。`apix_core::soap`
はバージョンによって異なる部分（ヘッダー、エンベロープ）を持ち、フォールトを読みます。
`Response::soap_fault` はそれを XML の応答に適用し、ランナーはテストのないリクエストをフォールトが
あれば、ステータスに関係なく失敗とします。

インポーターは何も取得しません。`Loader` は `wsdl:import`、`xsd:import`、`xsd:include` をたどります：
次にどの location を読むかを伝え、テキストを受け取るので、CLI とデスクトップアプリはそれぞれ自分の
方法でファイルとネットワークを読みます。location はそのドキュメントを基準に解決され、ネットワークから
来たドキュメントはローカルファイルを指せず、読み込みは 64 ドキュメントで止まります。次に `Model` が
すべてのドキュメントからメッセージ、ポートタイプ、バインディング、ポート、スキーマのグローバル定義を
名前空間と名前をキーとして集めます。`Writer` は XSD のノードから例を作り（roxmltree がノードを保持し、
`prefix:name` をノード自身の宣言に対して解決します）、各要素が修飾されるかどうかを `form`、
`elementFormDefault`、グローバルかどうかから決めます。使う名前空間はエンベロープに置かれます。
書き出し中の名前付きの型のスタックが再帰を止めます。

アプリと CLI の URL インポートはテキストを読み、その後はファイルと同じ経路をたどるので、すべての
形式に対応します。`apix-import/tests/fixtures/` のテスト用 WSDL は、`soap_server` のサンプル
（:8773）も動かします。

## GraphQL スキーマ（M6）

`apix_core::graphql` は標準の `INTROSPECTION_QUERY`、その応答から読んだ `Schema`（フィールド、引数、
入力フィールド、列挙値、インターフェース、possible types を持つ型。`TypeRef` はリストと `!` を保持）、
そしてルートの型を先に書き、組み込みのスカラーと `__` の型を省く `to_sdl` を持ちます。エディター用には
クエリをカーソルの位置までだけ読みます：`spot_at` はテキストをトークンに分け（コメント、文字列、
`$variables` は捨てる）、選択、引数リスト、入力オブジェクト、リストのスタックをたどり、エイリアス、
`... on Type`、フラグメント、変数定義に従って、そこに何が入るか（`Spot`）を示します。`suggestions`
はそれを型と説明付きのラベルに変え、前方一致、次に部分一致の順に並べ、非推奨のものは最後にします。
`describe_at` はホバー用の Markdown を返します。検証は何もしません：壊れたクエリでもできる限り補完
します。変数エディターも補完します：`variable_types` は `$name: Type` の宣言を読み、小さな JSON
スキャナーがカーソルまでのキーとリスト要素のパスと、そこにキーと値のどちらが入るかを見つけ、
`variable_suggestions` がそのパスを入力フィールドとリスト要素に沿ってたどり、まだ与えられていない
変数、入力フィールド、列挙値（JSON の文字列として）、`true`/`false` を候補に出します。入力中の文字列は
引用符ごと置き換えられ、エディターが閉じた引用符も含みます。

`apix_http::graphql::fetch_schema` は、リクエストの URL、ヘッダー、認証、設定でイントロスペクション
クエリを POST として送り（先にリクエストを解決します）、スキーマを読むか、ない理由（ボディの冒頭付きの
2xx 以外のステータス、またはサーバーの `errors[0].message`）を示します。

アプリでは、`workspace/graphql_ui.rs` が、クエリエディターが持つ共有の
`Rc<RefCell<Option<Rc<Schema>>>>` の上に gpui-kit の `CompletionProvider` と `HoverProvider` を
実装します。補完は入力中の単語を LSP のテキスト編集で置き換えます。**ボディ** エディターの
**スキーマを取得** は `FetchSchema` を発行します。ワークスペースは送信と同じようにリクエストを
組み立て（継承した認証と設定、変数）、ネットワークランタイムで取得します。各エンドポイントの応答は
`<config>/Apix/graphql/<FNV-1a of the URL>.json` に保存され、GraphQL リクエストを開いたときに
読み込まれます。SDL のビューは読み取り専用のエディターで、表示されたときに一度だけ、描画中に
埋められます。

## gRPC（M6）

`apix-grpc` は生成コードなしで呼び出しを行います。サービスの説明は `prost_reflect::DescriptorPool`
です：リクエストの `.proto` ファイルから protox を通して（Rust、`protoc` なし。インポートは
`import_paths` と各ファイルの隣で探し、Google の well-known types は組み込み）、またはサーバーから
gRPC リフレクションを通して（v1、次に v1alpha。サーバーが省いたファイルは名前で問い合わせ、依存の
順に追加）作ります。メッセージは `DynamicMessage` です：入力の JSON は prost-reflect の serde 対応を
通し、出力の JSON は `.proto` のフィールド名と既定値を表示します。`DynamicCodec` がそれらを tonic に
差し込むので、`tonic::client::Grpc` は 4 種類すべてのメソッド（単項、サーバーストリーミング、
クライアントストリーミング、双方向）を 1 つの `Channel`（HTTP エンジンがすでに使っている hyper と
rustls の上の HTTP/2）で実行します。

`apix_grpc::convert` は、ほかにあるものから説明を作ります。順に試すのは：JSON のディスクリプター
セット、`.proto` のテキスト、生成コードが埋め込む `FileDescriptorProto`（`embedded.rs` は各
ジェネレーターがそのバイト列をどう書くかを知っており、PHP のものは正しいテキストですらないことが
あります）、次にプログラミング言語での宣言（`source.rs`：コメントを除き、ブロックを見つけ、言語の
系統ごとにメンバーを読み、型を正規化し、番号はアノテーションと ts-proto のエンコーダーから取る）
または JSON の例（`json.rs`。キーの順序を保ちます。`serde_json::Value` では失われるためです）です。
最後の 2 つは、`.proto` として出力される小さな `Schema` を経由します。`printer.rs` は正確な説明を
`.proto` のテキストとして書き戻し、そのテキストが同じ説明にコンパイルされる場合にだけ採用します。
そうでない場合や、複数のファイルの場合は、定義は JSON のディスクリプターセットのままです。
`compare` は定義をサーバーの説明と照らし合わせ、メッセージ名は通信に現れないのでフィールドを名前で
対応付けます。`described` は順序を持ちます：リクエストの定義、そのファイル、リフレクション。
推測したものはまずリフレクションを試し、そちらに譲ります。`call` は `DynamicMessage` が配置できなかった
フィールドを報告します（`CallEvent::Note`）。ほかに報告するものがないためです。

リクエストのヘッダーと認証はメタデータになります（Bearer、Basic、API キーのヘッダー、
`apix_http::oauth2` を通した OAuth 2.0。`-bin` のキーは base64）。HTTP リクエストに署名する認証の
種類は、理由を添えて拒否されます。タイムアウト、クライアント証明書、信頼する CA はリクエストの設定から
来ます。TLS の検証がオフのとき、tonic（そのような切り替えを持たない）には普通の `http://` アドレスと
本当の `https` のオリジンが渡され、独自のコネクター（`insecure.rs`）が TLS を担当します：ALPN h2 と、
どの証明書でも受け入れるがハンドシェイクの署名は確認する検証器を使う rustls です。`call` は `Sent`、
`Headers`、`Received` のイベントを起きた時点で報告し、`CallOutcome` で終わります：ステータスコードと
メッセージ、レスポンスのメタデータ、トレーラー、すべてのメッセージです。OK 以外のステータスはエラー
ではなく結果です。エラーとは呼び出しが起きるのを妨げるもの（接続がない、説明がない、型に合わない
メッセージ）です。

アプリでは、`grpc://` の URL は **ボディ** タブを **メッセージ** タブ（`workspace/grpc_editor.rs`）に
変えます：**メソッドを読み込む** で埋まる検索可能なメソッドの選択欄、`.proto` ファイル、そして入力の
型から作った例を持つメッセージです。呼び出しはネットワークランタイムで実行され、UI が 60 ms ごとに
読む共有の構造体を埋めます。レスポンス欄は WebSocket と同じようなログ（`ResponseState::Grpc`、
`workspace/grpc_ui.rs`）で、Esc で呼び出しを破棄します。CLI は単項の応答を丸ごと、ストリームを
1 行ずつ出力し、`apix schema` はサービスを `.proto` 風のテキストとして書き出します。

実行では gRPC リクエストを `apix_grpc::send_for_tests` を通して呼び出します。これは `CallOutcome` を
`apix_http::Response` に変換します（`CallOutcome::to_response`）：コードが表す HTTP ステータス
（Google の対応表）とコードの名前を理由に、メタデータとトレーラーに加えて `grpc-status` と
`grpc-message` をヘッダーに、メッセージ（サーバーストリームなら JSON のリスト）をボディにします。
こうすればテストスクリプト、チェック、レポートに gRPC 固有のものは要りません。履歴は `Summary::grpc`
を設定して呼び出しを保存します：ステータスは gRPC のコード、トレーラーとステータスメッセージは専用の
フィールドに入り、ボディはログ（`[{"at_ms", "sent", "message"}]`）で、`grpc_ui::restored` がそれを
ログのビューに戻します。

## 並行実行とモニター（M7）

`Plan::parallel` が 1 より大きいと、その数の反復を同時に実行します（実行自身のタスク上の
`futures_util` の `buffer_unordered`。スレッドは使いません）：各反復はスクリプトの状態の自分用の
コピーを持って順にリクエストを送り、`bail` のもとでの失敗はほかの反復を止め、レポートは後で反復ごとに
並べ替えられます。進捗は呼び出し元のコールバックを包むミューテックスを通ります。

`apix_core::monitor` は `apix.toml` の `[[monitor]]`（名前、`every`、`run`、環境、Webhook、有効かどうか）
を記述し、間隔を読みます。`apix_runner::monitor` はモニターを 1 つ実行し（`plan` は `apix run` と同じく
変数を解決します：モニターの環境、`APIX_VAR_*`、次に固定した値）、結果をまとめ（`Outcome`）、モニター
ごとに JSON Lines の `Log` を保持し（`<config>/Apix/monitors/<hash of the collection>-<name>.jsonl`、
直近 500 件）、それぞれの実行時期を決め（`Scheduler`：初めて見るモニターは、ログにある最後の実行から
間隔が過ぎたときに実行時期になります）、状態が変わったときに Webhook に知らせます（`change`、
`notify`）。`apix monitor` とデスクトップアプリ（`workspace/monitors_ui.rs`：設定がオンの間は毎秒の
ティッカー、ネットワークランタイムで一度に 1 つずつ実行）の両方がこれを使うので、ログを共有します。

## Git（M7）

`apix_core::git` はすでにインストールされている `git` プログラムを実行します（git のライブラリは
使わないので、バイナリには何も加わらず、ユーザー自身の設定、フック、資格情報が適用されます）。
`Repo` はコレクションまたはプロジェクトのフォルダーで開かれ、すべてのコマンドはそこで `--relative`
付きで実行されるので、パスはそのフォルダーからの相対パスになり、大きなリポジトリの中のコレクションは
自分のファイルだけを見ます。`changes_since(rev)` は `git diff --merge-base rev`（プルリクエストと
同じく、ブランチが `rev` から分かれた地点）に未追跡のファイルを加えたものです。履歴がない場合
（シャロークローン）は `rev` そのものにフォールバックし、そのことを伝えます。`show(rev, path)` は
あるコミットでのファイルを読みます。

`git::affected` は変更されたファイルを、その役割（`git::role_of`）によってリクエストに対応付けます：
リクエストファイルまたはその `<stem>.*.js` スクリプト → そのリクエスト。`folder.toml` または
`_folder.*.js` → そのフォルダー以下のリクエスト。`apix.toml`、`_collection.*.js`、
`environments/*.toml`、プロジェクトファイル、プロジェクトの環境 → すべて。それ以外のファイル →
そのファイルを参照するリクエスト。`apix run --changed` はそれらのリクエストを残し、レポートはその選択を
記録します（`RunReport::selection`）。

`apix_core::diff` はファイルの 2 つのバージョンを意味で比較します：リクエストは `Request` に解析して
JSON にシリアライズし（そのため既定値や書式は関係ありません）、ほかの TOML は値に、スクリプトや
ほかのテキストは行ごとに比較します（共通の先頭と末尾を除いた最長共通部分列）。重複しない `key` または
`name` を持つ行のリスト（ヘッダー、クエリ、`[[example]]`）はそれをキーとするテーブルになるので、
行は名前で対応付けられます。フィールドはパスで名前が付きます（`headers.Accept`、`body.json`）。
同じ内容の削除されたリクエストと追加されたリクエストは、1 つの移動になります。`apix diff` は結果を
テキスト、Markdown、JSON で出力します。

`RunReport::to_markdown` は実行結果を GitHub 用に書き出します（リクエストごとにテーブルの 1 行、
失敗は開いた `<details>` の中に）。先頭は隠しマーカー `<!-- apix-run: <name> -->` で、アクションの
コメントのステップは以前のコメントを編集するためにこれを探します。GitHub の 65 536 文字の上限に
収まります。

デスクトップアプリ（`workspace/git_ui.rs`）は、再読み込みのたびとウィンドウがアクティブになったときに、
開いているフォルダーのブランチと `git status` をバックグラウンドで読み、サイドバーの印のために変更された
パスのマップを保持します。自身のスクリプトが変わったリクエストは変更ありとして数えます。

**Git** ウィンドウ（`workspace/git_panel.rs`）は同じ `Repo` を使います：一覧には `working_changes`、
選択したファイルにはディスク上のファイルに対する `show("HEAD", path)` を `diff::file` で、そして
`commit_folder`（`git add -A -- .`、次に `git commit -- .`。開いているフォルダーだけが対象）、
`branches`、`switch`、`create_branch`（`switch -c`）、`pull`（`--ff-only`）、`push`（上流がなければ
`-u origin <branch>`）、`log`（ハッシュ、時刻、件名）です。git は `GIT_TERMINAL_PROMPT=0` で実行される
ので、パスワードが必要なリモートは入力を待たずに git のメッセージとともに失敗します。資格情報ヘルパーは
引き続き使えます。各操作はバックグラウンドで実行され、ステータスバーを更新します。

## AI アシスト

`apix_core::ai` は 3 つのプロバイダーのいずれか向けのリクエストを手で組み立て（Rust にはそれらの
公式 SDK がありません）、応答を読みます。`Provider`（`anthropic`、`openai`、`google`）は既定の
モデル、キーのキーチェーンのアカウント（`ai:<provider>-api-key`、`apix_vault::keychain::save_text`
経由）、その環境変数、URL を与えます。Anthropic：`x-api-key`、`anthropic-version: 2026-10-21`、
`claude-opus-5-5` を使う `POST /v1/messages`。既定のモデルではサーバー側のフォールバック
（`fallbacks: "default"`、ベータ `server-side-fallback-2026-07-01`）も使います。`refusal` の停止理由は
メッセージになります。OpenAI：Bearer キー、system メッセージと user メッセージを使う
`POST /v1/chat/completions`。`refusal` フィールドは拒否として扱います。Google：`x-goog-api-key`、
`systemInstruction`、`contents` を使う `POST
/v1beta/models/{model}:generateContent`。`SAFETY` による終了とブロックされたプロンプトは拒否として
扱い、思考のパートは飛ばします。どのプロンプトにもやり取りの内容（`Context::describe`：資格情報の
ヘッダーはマスク、ボディは 12 KB で切り詰め）、タスク、アプリの言語コードが入ります。
`models_request` / `models` はキーで使えるものを一覧にします（OpenAI はチャットモデルだけ、Google は
`generateContent` できるモデル）。テストでは `APIX_AI_URL` がベース URL を置き換えます。アプリ
（`workspace/ai_ui.rs`）は `net::send` を通して送信し、`ScriptEditor::append_test` でテストを追加し、
説明はダイアログに表示します。**設定** は `ai_provider` と `ai_model` を保持します。

## MCP サーバー

`apix mcp`（`apix-cli/src/mcp.rs`）は標準入力から 1 行に 1 つの JSON-RPC 2.0 メッセージを読み、
標準出力に 1 行に 1 つの応答を書きます（MCP の stdio トランスポート。最初の行の前のバイトオーダー
マークは読み飛ばします）。`initialize` は、求められたプロトコルバージョンを知っていればそれで
（2025-06-18、2025-03-26、2024-11-05）、知らなければ最新のもので応答し、ツールだけを提供します。
通知には応答せず、不明なメソッドはエラー -32601 になります。`tools/call` の結果は JSON をテキストと
`structuredContent` の両方で持ち、失敗したツールはプロトコルエラーではなく `isError` 付きの結果に
なるので、エージェントは理由を読めます。エージェントからのパスはコレクションのルートに連結され、
`..` を解決し、ルートの外なら拒否します。`send_request` は `apix_runner::exchange`（スクリプト、変数、
テスト）を、`run_requests` は `apix_runner::run` を通ります。変数はほかのコマンドと同じ
`VarArgs::resolve` から来て、エージェントの `variables` は `--var` として扱われます。CLI は 16 MB の
スレッドで動きます。Windows ではコマンドの共有の future がメインスレッドのスタックに収まらなく
なったためです。

## セットアップと Claude

`workspace/setup_ui.rs` は **Apix のセットアップ** を一度だけ（`settings.setup_shown`）、そして
**ヘルプ** から開きます。データベースのボタンは `route_ui::download_geo` を実行します（今月の DB-IP の
ファイル、なければ先月のものを、アプリのクライアントで取得し、UI スレッドの外で展開）。
`apix geo download|status|remove` はターミナルで同じことを行い、インストーラーの任意のステップは
これを呼ぶことになります。`apix_core::claude_mcp` は Claude Desktop の設定（`%APPDATA%\Claude`、
`~/Library/Application Support/Claude`、`~/.config/Claude`。テストでは `APIX_CLAUDE_CONFIG`）を見つけ、
サーバーに `apix-<collection>` という名前を付け、実行中のプログラムの隣（または PATH 上）の `apix` を
`mcp <folder> [--list-only]` で指すようにし、ほかはすべて残したまま `mcpServers` にマージします
（元のファイルは一度だけ `.bak` にコピー）。`claude_code_command` は
`claude mcp add --scope user` です。

## フロー

`apix_core::flow` がファイルです：`Flow` はノード（`NodeKind::Request`、`Check`、`Set`、`Delay`。
それぞれ id と位置とともに `[[node]]` テーブルに平坦化）と、あるノードの結果（`ok`/`fail`、
`yes`/`no`、`next`）から別のノードへのエッジを持ちます。`problems()` はフローの実行を妨げるものを
一覧にします。`Seen` は最後のレスポンスで、`Seen::subject` は `status`、`time`、`body`、
`header:Name`、または `apix_core::filter` を通した JSONPath や jq のフィルターを読みます。`.flow`
ファイルはコレクションのどこに置いてもよく、サイドバーはそれを無視します。

`apix_runner::flow::run` は開始ノードからグラフをたどります。リクエストのノードは実行と同じく
`exchange` を通るので、スクリプト、テスト、`pm.*` の変数が使えます。ノードが保持したものはスクリプトの
状態のローカルスコープに設定され、`pm.variables.set` と同じように、後続のノードでは環境より優先
されます。対象がテキストのチェックは `{{variables}}` を解決し、未定義のものがあれば値なしになります。
実行は各ステップを報告し（`FlowProgress`）、1000 ステップの後、または `Control` によって止まります。
`FlowReport::to_run_report` は実行のレポート（JUnit、HTML、Markdown）を再利用します。デスクトップの
エディター（`workspace/flows_ui.rs`）はノードを配置された要素として、エッジをキャンバス上に描きます：
前向きのエッジは曲線で、戻るエッジはノードを回り込む線で描きます。Runner と同じく、フローを
ネットワークランタイムで実行し、タイマーで進捗を読みます。

## プラグイン

`apix-plugins` は WebAssembly プラグインをインタプリタの `wasmi` 2 で実行します：JIT はないので、
プラグインからネイティブコードが作られることはなく、Rust がビルドできる場所ならどこでもビルド
できます。モジュールは何もインポートできないので、プラグインにはホスト関数がまったくありません
（ネットワーク、ファイル、時計、乱数なし）。プラグインは `memory`、`apix_alloc`、`apix_manifest`、
`apix_call` をエクスポートし、応答は `ptr << 32 | len` として渡される JSON です。呼び出しのたびに
モジュールを新しくインスタンス化し（呼び出し間の状態なし）、2·10⁹ 単位の燃料と 256 MB のメモリ上限で、
64 MB のスタックを持つスレッド上で実行します。燃料切れは "ran too long" として報告されます。
マニフェストはプラグインを読み込むときに一度だけ読まれ、その `name` は `[a-z0-9-]` でなければ
なりません。`installed()` はプラグインフォルダー（`<config>/Apix/plugins`、または `APIX_PLUGINS_DIR`）
を `reload()` までキャッシュします。

フックは Apix のほかの部分と 3 か所で接します：モデルの `Auth::Plugin { plugin,
config }`（ほかの認証と同じく解決され（設定内の変数）、`apix_http::prepare` が最終的なリクエストに
適用）、インポーターの応答をリクエストファイルに変える `apix_import::plugin::plan`、そしてプラグインを
UI スレッドの外で実行してテーブルやテキストを描くレスポンスのビジュアライザー（`VisualMode::Plugin`）
です。wasmi はデバッグビルドでも `opt-level = 3` で、デバッグアサーションなしでビルドされます。
そのディスパッチは末尾呼び出しに頼っており、そうしないとスタックを使い果たすためです。

## レスポンスの比較

`apix_core::compare` は 2 つの `Snapshot`（ラベル、ステータス、ヘッダー、ボディ、時間）を受け取ります。
ヘッダーは小文字の名前で比較し、同じ名前が複数あれば連結します。`VOLATILE` にあるもの（日付、
キャッシュ、リクエスト id やトレース id、レート制限、ボディの送り方）は、省かれたものとして数える
だけです。2 つの JSON ボディは `by_id` を通ります。これは重複しない `id` / `_id` / `uuid` / `key` の
値を持つレコードのリストを `[id=…]` をキーとするテーブルに変え、その後 `diff::fields_as_they_are`
（リクエストの差分と同じたどり方で、key/name による対応付けをしないもの。それはリクエストファイルには
合いますが、データには合いません）を通ります。ほかのテキストは行ごとに比較し、それ以外はサイズだけを
比較します。アプリの **レスポンスを比較** ウィンドウ（`workspace/compare_ui.rs`）は、前の側を履歴
（同じリクエストファイル、または同じメソッドと URL のエントリ）から取るか、別の環境の変数で解決した
リクエストを送信します。`apix compare` は 2 つの環境でリクエストを送信します。

## 負荷テスト

`apix_http::load::Load` は解決済みのリクエスト用に reqwest クライアントを 1 つ作り（リクエストの TLS
設定は `tls::client_config` 経由、そのプロキシ、タイムアウト、リダイレクトポリシー、無制限の
アイドルプール）、`hit()` がそれを送信して、ボディのバイト数を展開せずに数えます。OAuth 2.0 は一度だけ
Bearer トークンになり、Digest は拒否されます。`apix_runner::bench::Bench::run` は `concurrency` 個の
ワーカーを 1 つの `FuturesUnordered` の中で開始します。各ワーカーは、件数、期限、`stop()` のいずれかが
止めるまでアトミックカウンターから次のリクエスト番号を取り、レートの指定があれば
`start + n / rate` まで待ちます。各ヒットはミューテックスのもとで記録されます：マイクロ秒単位の
レイテンシ（リクエストごとに `u32` なので、100 万リクエストで 4 MB）、ステータス、種類ごとのエラー
（"could not connect"、"timed out"、…）、バイト数、秒ごとのバケット。`summary()` は実行中にも
呼べます：レイテンシのコピーを並べ替えて最近順位法のパーセンタイルを求めます。CLI はコアごとに
1 スレッドを持つ専用のランタイムでテストを実行し、1 秒に 1 行出力します。アプリはネットワーク
ランタイムで実行し、1 秒に 4 回再描画します。

## 経路と地図

`apix-route` は、`apix_core::git` が git を起動するのと同じようにシステムのツールを起動します：
Windows では `tracert -d -h N -w 1000`、macOS と Linux では `traceroute -n -q 1 -w 1 -m N`、
traceroute がなければ `tracepath -n -m N`。どれも管理者権限を必要としません。自分で ICMP を送ると、
Windows では管理者権限が必要になります。`parse_line` はどのツールの行も、その数値とアドレスだけで
読みます（最初にホップ番号、すべての `N ms`、アドレスとして解析できる最初の語）。そのため翻訳された
Windows で独自のコードページを使っていても同じように読めます。ホップの時間は試行の中で最良のもので、
`<1 ms` は 0.5 ミリ秒として数えます。`Trace` はツールが出力するとおりにホップを順に返し、破棄または
停止されるとツールを終了させます。

`Geo` は MaxMind DB ファイル（`maxminddb`）を開き、パブリックアドレスに対して `Place`（緯度、経度、
英語の都市名、ISO の国）を返します。プライベート、ループバック、リンクローカル、CGNAT、マルチ
キャストのアドレスは決して検索しません。データベースは `<config>/Apix/geo` で最初に見つかった
`.mmdb` です。`install` はダウンロードしたもの（gzip）を展開し、開けることを確認してから初めて古い
ものを置き換えます。`dbip_urls` は今月と先月の DB-IP Lite のアドレスを返します（`APIX_GEO_URL` で
置き換えられます）。地図は `land::LAND` の正距円筒図法による投影で、大陸と大きな島を約 2 度の精度で
手で描いたもの（黒海とカスピ海は `land::WATER` に）なので、地図データのダウンロードやライセンスは
不要です。デスクトップのウィンドウ（`workspace/route_ui.rs`）はトレースをスレッドで実行し、届いた
ホップから配置し、データベースはアプリの起動ごとに一度だけ読み、GPUI のパスで地図を描きます。
`apix trace` はホップをその場所とともに、100 km を超える各ジャンプの距離とともに出力します。

## アップデートの確認

`apix_core::update` はリクエストを組み立て（11R5/Apix の GitHub の `releases/latest`、
`User-Agent: Apix/<version>`、10 秒のタイムアウト。`APIX_UPDATE_URL` で別の場所を指せます）、応答を
読みます：`Version` は `v1.2.3`、`1.2`、プレリリース（`1.0.0-rc.1` は `1.0.0` より前に並び、数値の
部分は値で比較）を解析します。404 はまだ何も公開されていないことを意味し、ほかのエラーは GitHub の
メッセージを伝えます。CLI（`apix update`）は自分のクライアントで送信します。アプリ
（`workspace/update_ui.rs`）は、`settings.check_updates` がオンなら起動時に `net::send` を通して
（新しいリリースがあるときだけ表示）、また **ヘルプ › アップデートを確認…** から（どの応答も表示）
送信します。何もダウンロードやインストールはしません。

## テーマ

`theme.rs` が色を持ちます。`ThemeDef` は `Colors` とダーク／ライトのフラグを持つテーマです（組み込み、
または `<config>/Apix/themes/*.toml` から読んだもの。ファイルが省いた色は Obsidian または Daylight から
来ます）。`Palette::build` はそれをビューが描画に使うものに変えます：アクセントの設定（ダーク用と、
より濃いライト用の色調を持つ名前付きの色、または `#RRGGBB`）、相対輝度で選ぶアクセント上のテキスト色、
ダークまたはライト用のホバーと入力欄の色合い、そして透明度の設定です：ウィンドウの背景のアルファは
`1 - t`、パネルはその 60% の割合で透明になり、枠線は `t` が大きくなるにつれて白に向かって明るくなり、
ライトテーマは `t` の 70% を使います。`from_settings` は外観の設定に従ってダークまたはライトのテーマを
選び、「システムに従う」の場合は `App::window_appearance` を使います。

gpui-kit のルートビューはウィンドウ全体の下に `Theme::background` を描画し、それでは OS のぼかしが
隠れてしまうので、透明度があるときは透明に設定し、ダイアログには `bidi::DialogBidi` を通して独自の
背景を与えます。すべてのダイアログがこれを使います。ぼかしは OS が行います：
`WindowBackgroundAppearance::Blurred` は Windows ではアクリル（`SetWindowCompositionAttribute`）、
macOS ではバイブランシーです。

ビューはパレットのコピーを持ちません：描画するときに `theme::current()` を呼びます。`theme::set` は
新しいパレットを保存し、gpui-kit のコンポーネントテーマをそれに向け（モードも切り替えるので、
コードエディターの色も追従します）、すべてのウィンドウの背景をぼかしまたは不透明に設定して
ウィンドウを再描画するので、変更はすぐに表示されます。ワークスペースはウィンドウの外観を監視し、
システムがライトとダークを切り替えたときに再びこれを呼びます。

## 言語（M4）

デスクトップ UI のすべてのテキストはキーを通ります：`tr!("menu.file")`（表示用の `SharedString` を
返す `rust_i18n::t!` の薄いラッパー。`tr_plain!` は、新しいリクエストの既定の名前のように保存される
ものに同じテキストを返します）。rust-i18n は gpui-kit が使うのと同じ仕組みなので、どちらもプロセス
全体で 1 つのロケールに従います。中国語は gpui-kit 自身のテキストが使うコードである `zh-CN` を
使います。各言語は 1 つのフラットなファイル `crates/apix-desktop/locales/<code>.yml`（1 行に 1 つの
`key: value`、ソート済み、`%{name}` のプレースホルダー）で、バイナリに組み込まれます。ファイルが
変わると `build.rs` が再ビルドします。テキストがなければ英語にフォールバックし、ある言語にキーが
足りないとき、不明なキーがあるとき、または英語と異なるプレースホルダーを使っているとき、単体テストが
失敗します。言語によって語順が異なるので、文はプレースホルダー付きのキー全体で表し、断片をつなぎ
合わせることはしません。

言語は **設定** で選び（初回起動時はシステムの言語に従います）、すぐに切り替わります：テキストは
描画のたびに読まれ、ウィジェットの中に保持される少数のテキスト（入力欄のプレースホルダー、選択欄の
項目）は `Workspace::retranslate` が設定し直します。日本語、中国語、アラビア語のグリフは、組み込みの
Inter の隣で、プラットフォームのフォントフォールバック（DirectWrite、Core Text、fontconfig）から
来ます。アラビア語のテキストは右から左へ整形・配置されます。Windows ではそのために
`vendor/gpui-pre-windows` が必要です。これはグリフの並びの配置を修正した gpui 0.3.9 の DirectWrite
バックエンドです（ワークスペースの `Cargo.toml` からパッチとして適用。上流で修正されたら外します）。
gpui は右から左へのテキストを折り返せません：段落を 1 行として整形し、左から右へ増えると想定した
グリフの x 位置で切るためです。そのため折り返す可能性のあるテキスト（注記、ヒント、説明、メッセージ）
は `bidi::para` を通ります。これは RTL では読む順にスペースでテキストを区切り、候補の行ごとに幅を
測り、各行を折り返さずに描画します（`src/bidi/paragraph.rs`）。ほかの言語では普通の gpui の
テキストです。gpui はまた、すべての行を左から右の基本方向でレイアウトするので、RTL の言語では
`tr!` が各テキストを右から左への埋め込み（U+202B…U+202C。DirectWrite は新しい isolate を無視します）
で包みます。こうすると、ラテン文字の語（"URL"、"OAuth 2.0"、"Ctrl+S"）で始まる、または終わる文も
右から左に読めます。`i18n::display` はステータスバーのような組み立てたテキストにも同じことを行います。
この印がファイルに入ることはありません：保存するテキストには `tr_plain!` を使います。エンジンの
クレートからのエラーメッセージは英語のままです。

右から左の言語（`Language::rtl`、アラビア語）ではウィンドウが左右反転します：サイドバーは右へ、
レスポンス欄はリクエスト欄の左へ、ウィンドウのコントロールは左へ移り、メニュー、タブの列、フォームの
行、ステータスバーは右から並び、テキストは右揃えになります。GPUI にはレイアウトの方向がないので、
これは `src/bidi.rs` が行います：スタイルを持つすべての要素に対する `Bidi` トレイト（`dir_row` は行を
反転し、`text_start` / `text_end`、`ps` / `pe` のパディング、`border_e_1` は方向によって側を選び、
`ltr` はテキストを左に固定）、gpui-kit の `TabBar` 用の `tab_bar` / `fill_tabs`（タブを逆順で渡し、
インデックスを元に戻す）、ダイアログ用の `bidi_title`（RTL では閉じるボタンが左側のタイトル行に移る）
です。補助関数はビューの描画中に言語を読むので、言語を変えるとすぐにウィンドウが反転します。コードと
レスポンスのビューアー、URL バー、キーと値のテーブル（パラメーター、ヘッダー、フォームのフィールド、
変数）、レスポンスのヘッダーは、どの言語でも左から右のままです。gpui-kit のポップアップメニューと
ボタンは、内部のレイアウトを左から右のまま保ちます（ショートカットは右、アイコンはラベルの前）。
メニューはボタンから左に向かって開きます。

言語の追加：`locales/en.yml` を `locales/<code>.yml` にコピーして値を翻訳し、`src/i18n.rs` の
`LANGUAGES` に 1 行追加します。

ユーザー向けのドキュメントは計画とは別に、言語ごとに 1 つのフォルダー（`Docs/<code>/`：ユーザー
ガイド、アーキテクチャ、技術ノート、ワークフロー、画像）に置かれ、`Docs/languages.toml` に並んで
います。英語が原文です。各翻訳は元にした英語のバージョンを記録しているので、古くなった翻訳が
分かります。

## スレッドモデル（デスクトップ）

- GPUI がメインスレッドを持ち、そこでイベントループと描画を実行します。
- マルチスレッドの **tokio ランタイム** を 1 つ、バックグラウンドスレッドで一度だけ起動します。
  reqwest、tonic、tungstenite にはこれが必要です。
- UI は tokio に仕事を送り、結果を GPUI のタスク（`cx.spawn_in`）で受け取ります。そのタスクを
  破棄するとリクエストが中断されるので、Esc や再送信はきれいにキャンセルされます。
- レスポンスが届くと、UI が見る前に、ボディは GPUI のバックグラウンドエグゼキューターで整形され、
  インデックスが作られます。整形には `apix_core::text::pretty_json` を使います。これはツリーを
  作らず、数値を送られたとおりに保つ、1 パスのインデント付け直し処理です。

## 大きなレスポンスのビューアー

512 KB 以上のボディは、コードエディターの代わりに `large_view.rs` を使います。

- `apix_core::text::RowIndex` は、表示する各行の開始位置を 1 行あたり 8 バイトで記録します。
  2 KB を超える行（minify された JSON）は複数の行に分割されます。
- スクロール位置はピクセルのオフセットではなく**行番号**です。1000 万行を 20 px で並べると約 2 億 px
  になり、`f32` のピクセルが正確な範囲を大きく超えます。ビューアーは画面に収まる約 120 行だけを
  レイアウトします。ホイール、独自のスクロールバー、キーボード（↑ ↓ PgUp PgDn、Ctrl+Home/End）を
  扱います。
- 大きな JSON ボディの **生データ** タブは、開いたときに初めて作られます。
- Ctrl+F はバックグラウンドスレッドでテキスト全体を検索し（文字どおりのクエリに対する、大文字小文字を
  区別しない `regex`）、最大 100,000 個のバイト範囲を保持します。`RowIndex::row_of` は二分探索で
  一致箇所の行を見つけ、画面上の行だけが自分の一致箇所を探してハイライトします。

小さなボディはコードエディターに表示され、その検索パネルが有効になっています。**整形** の上の
フィルター（`workspace/filter_ui.rs`）は、最後のキー入力から 250 ms 後に UI スレッドの外で
`apix_core::filter` を実行します：`$` で始まる式なら `serde_json_path` による JSONPath、それ以外は
`jaq` による jq で、jq の出力は最大 10,000 個です。その結果はほかのボディと同じ `Document` なので、
フィルターをかけた大きなボディがエディターに入ることも、その逆もあります。

Windows のリリースビルドで、ローカルサーバー
（`cargo run -p apix-http --release --example big_json_server`）からの minify された 100 MB の JSON で
計測しました：Enter を押してから約 0.33 秒でレスポンスが画面に表示されます。これには転送、
960 万行への整形、インデックス作成、最初のフレームが含まれます。メモリは、ボディを読み込んだ状態で
約 407 MB、アイドル時は 69 MB です。

## 性能の目標

| 指標                                | 目標              | 計測値（Windows、リリース） |
|-------------------------------------|-------------------|-----------------------------|
| コールドスタートから最初のフレームまで | < 150 ms          | 222 ms（`main` から最初のフレームまで。そのうち約 150 ms はグラフィックスドライバーの読み込み） |
| アイドル時の RAM                    | < 60 MB           | ワーキングセット 88 MB、サンプルコレクションを開いた状態（大半はグラフィックスドライバー） |
| リリースバイナリ                    | < 100 MB          | 54.4 MB |
| 入力から描画までの遅延              | 1 フレーム（120 Hz で ≤ 8 ms） | 未計測 |
| 100 MB の JSON ボディを開いてスクロール | フレーム落ちなし | 送信から 306 ms で画面に表示（ローカルサーバー）。フレーム時間は未計測 |

起動時間とアイドル時のメモリは、現在目標を上回っています。これらのプロファイリングを予定しています。
数値は `scripts/measure.ps1` から得ています（[技術ノート](technical.md#性能の計測)を参照）。

リリースプロファイルはすでに fat LTO、コード生成ユニット 1 つ、シンボル削除を使っています
（ルートの `Cargo.toml` を参照）。
