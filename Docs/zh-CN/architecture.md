<!-- translated from en/architecture.md sha256:75b4d247b3b61d0d -->

# Apix 架构

Apix 是用 Rust 编写的原生 API 客户端。界面由 [GPUI](https://github.com/zed-industries/zed)（Zed 的界面引擎）和 [gpui-kit](https://github.com/longbridge/gpui-kit) 组件库在 GPU 上绘制。它没有 WebView，也不用 Electron。

## 原则

1. **本地优先，适合 git。**集合是由小型 TOML 文件组成的文件夹。不需要账户，也不需要云。集合像代码一样被提交、比较差异和评审。
2. **界面从不阻塞。**网络、磁盘、解析和脚本都在界面线程之外运行。界面线程只负责布局和绘制。
3. **一个引擎，多个前端。**桌面应用、`apix` CLI 和将来的前端（TUI、MCP 服务器）共用同一组 crate，因此请求在任何地方的行为都相同，包括在 CI 中。
4. **能处理巨大的响应。**100 MB 的 JSON 响应也必须能流畅滚动。只对可见的行进行布局（虚拟列表），正文按需解析。
5. **轻量且启动快。**速度和功能优先于体积。release 二进制文件保持在 100 MB 以下（目前 54.4 MB；Electron 应用在 300 MB 以上），并在 150 ms 内打开窗口。

## Crate

```
crates/
├── apix-core     请求模型、{{variables}} 和 {{$dynamic}} 值、磁盘格式、
│                 项目、环境和机密、树和文件操作、
│                 GraphQL 模式（内省、SDL）和查询补全、
│                 git：分支、更改的文件、某次更改涉及的内容、请求级别的差异
├── apix-http     HTTP 引擎：reqwest + rustls、计时、响应捕获、网络跟踪、
│                 服务器发送事件、WebSocket（tokio-tungstenite）、Socket.IO、获取 GraphQL 模式
│                 （DNS、建立连接、重定向、证书、本地网卡）
├── apix-import   Postman、Insomnia、OpenAPI、HAR 和 WSDL → Apix 集合
├── apix-codegen  请求 ↔ shell 命令：curl（bash）、curl.exe（cmd）、PowerShell；解析粘贴的 curl；10 种语言的代码
├── apix-vault    加密集合：逐文件 XChaCha20-Poly1305、Argon2id、恢复密钥
├── apix-script   预请求脚本和测试脚本：带 pm.* 兼容 API 的 QuickJS
├── apix-runner   运行集合：顺序、迭代（也可并行）、数据文件、延迟、bail、
│                 报告（也包括用于拉取请求的 Markdown）、监视器：计划、日志、webhook
├── apix-grpc     gRPC 调用：tonic、以 JSON 表示的消息（prost-reflect）、.proto 文件（protox）、从代码或 JSON 得到的定义
│                 或服务器反射
├── apix-proto    其他协议：MQTT（rumqttc）
├── apix-mock     用已保存的示例作答的本地模拟服务器（hyper）
├── apix-capture  录制代理：普通 HTTP、CONNECT 隧道、用本地 CA 打开的 HTTPS（rcgen、rustls）
├── apix-route    到服务器的路由（系统的 traceroute）及各跳所在位置（离线 .mmdb 数据库）
├── apix-plugins  WebAssembly 插件（wasmi）：认证类型、导入器、响应视图；apix-plugin-sdk 用 Rust 编写它们
├── apix-cli      `apix` 二进制：在终端和 CI 中执行 send / run / diff / schema / import / shell
└── apix-desktop  GPUI 桌面应用（一个 crate 支持 Windows、macOS 和 Linux）
tools/
└── icongen       从 Logo/LogoFirst.jpg 生成所有应用图标（ico、icns、hicolor PNG）
```

`apix-desktop/src` 内部：`workspace.rs`（请求编辑器、响应面板、动作和快捷键）、`workspace/collections.rs`（侧边栏、项目、文件操作、导入、文件监视）、`workspace/env_editor.rs`（变量对话框）、`workspace/auth_editor.rs`（“认证”标签页以及文件夹/集合/项目的认证对话框）、`workspace/palette.rs`（命令面板及其模糊匹配器）、`workspace/network.rs`（响应的“网络”标签页）、`workspace/runner_ui.rs`（Runner）、`workspace/vault_ui.rs`（解锁、锁定和自动锁定、加密、解密、修改密码、恢复密钥）、`assets.rs`（gpui-kit 图标加上两个锁图标）、`workspace/menu_bar.rs`（标题栏中的 文件/请求/环境/视图/帮助；这些按钮会 `occlude()` 标题栏的拖动区域，这样 Windows 才会把点击交给它们）、`workspace/tabs.rs`（打开的请求；最前面的标签页存在于编辑器控件中，其他标签页保留各自的草稿、响应和进行中的任务）、`workspace/drag.rs`（拖动请求和文件夹来移动它们）、`workspace/shell_copy.rs`（复制为 / 粘贴 curl）、`workspace/var_chips.rs`（URL 栏中的 `{{variable}}` 标签及其悬停卡片）、`workspace/graphql_ui.rs`（GraphQL 模式的获取、缓存、补全和悬停）、`workspace/grpc_editor.rs` 和 `workspace/grpc_ui.rs`（gRPC 请求的“消息”标签页，以及响应面板中的调用）、`large_view.rs`（用于巨大正文的按行查看器）、`net.rs`（tokio 桥接）、`bidi.rs`（从右到左布局的辅助函数，参见“语言”）、`state.rs`（用户配置文件夹中的最近项目和按文件夹记录的环境）、`theme.rs` 和 `fonts.rs`。

### 一个桌面 crate 支持三种操作系统

GPUI 和 gpui-kit 用同一份代码在 Windows、macOS 和 Linux 上运行，Zed 就是这样发布的。因此没有按操作系统划分的界面文件夹。真正因操作系统而异的代码放在 `crates/apix-desktop/src/platform/{windows,macos,linux}.rs` 中的 `cfg` 之后：标题栏和窗口控制按钮、模糊/毛玻璃效果、macOS 菜单栏、`.apix` 的文件关联，以及钥匙串访问。安装程序和图标在 `packaging/{windows,macos,linux}/` 中。只有当某个操作系统真正需要不同的界面时，才会拆出单独的界面。

依赖方向：`core` ← `http`/`script`/`proto` ← `cli`/`desktop`。没有任何东西依赖 `desktop`。

## 请求的数据流

```
.apix file ──load──▶ Request ──resolve(Variables)──▶ Request (resolved)
                                                        │
                           预请求脚本（以后） ◀─────┤
                                                        ▼
                                         apix-http::HttpClient::send
                                                        │  (tokio runtime，后台线程)
                                                        ▼
                    Response {status, headers, body: Bytes, timing}
                                                        │
                             测试脚本（以后） ◀────────┤
                                                        ▼
                         界面：状态行、虚拟化的正文查看器
```

`{{variables}}` 分层解析。后面的层优先：
**全局 → 集合（`apix.toml`）→ 环境 → 覆盖（CLI 的 `--env-file`、`APIX_VAR_*` 和 `--var`，与集合中的名称匹配时不区分大小写；脚本的 `pm.variables.set`）**。
未定义的名称保持原样并被报告，因此界面可以给它们加下划线，CLI 可以拒绝发送。

### 网络跟踪

每次发送都会创建自己的短生命周期 reqwest 客户端，因此它记录的一切都属于该请求（`apix_http::trace`）。一个解析器为 DNS 计时，包在 reqwest 连接器外面的一个 Tower 层为每个连接计时，重定向策略记录每一跳。reqwest 既不交出它的 TLS 连接，也不允许我们自己的连接器返回一个连接，因此 TLS 详情来自 rustls 内部，通过 Apix 自己构建的客户端配置获取（`use_preconfigured_tls`、`tls::traced_config`、`tls_trace.rs` 中的钩子）：

- 一个包装的证书验证器记录服务器发送的证书链，以及它检查的是哪个版本的签名（TLS 1.2 或 1.3），然后交给常规验证器（或在关闭检查时交给“全部接受”）决定；
- 在写入 client hello 时，会向一个什么也不保存的会话存储请求密钥交换提示，这标志着 TLS 握手的开始：连接在此之前（DNS 之后）的时间算作 TCP，之后算作 TLS；它还会得知服务器选择的组（TLS 1.3）；
- 密码套件是 rustls 自己的，只创建一次，并用一个薄包装层包住它们的加密，记录建立的是哪个套件。套件是 `'static` 且共享的，因此包装层在请求的 future 被轮询期间通过一个线程局部集合找到该请求（`Scoped`）；握手就在这次轮询中运行。

ALPN 由响应的版本推断（只有协商了 `h2` 之后才是 TLS 上的 HTTP/2）。压缩的正文由 Apix 解码（`decode.rs`，gzip、zlib 或原始 deflate、brotli，边到达边解码），而不是交给 reqwest，因为 reqwest 会丢掉 `Content-Encoding` 和传输中的大小。

## 磁盘格式

```
my-api/
├── apix.toml               名称 + 集合级别的 [vars] 和 [auth]
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── folder.toml         可选：此文件夹中请求的 [auth]
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

**认证继承。**没有 `auth` 的请求从最近的、其 `folder.toml` 设置了认证的文件夹继承，然后是集合（`apix.toml`），再然后是项目（`apix-project.toml`）；都没有时什么也不发送。请求（或文件夹）中的 `auth = "none"` 会停止继承。`apix_core::project::inherited_auth` 为桌面应用、CLI 和“复制为”统一完成查找。

**请求设置。**`[settings]` 表（超时、重定向、TLS 验证、代理、客户端证书、受信任的 CA、Cookie）位于同样的四个层级。每个字段都是 `Option`：`None` 会沿同一条链逐字段继承（`apix_core::project::inherited_settings`），仍未设置的则回退到 `RequestSettings::effective` 中的默认值——没有超时、最多跟随 10 次重定向、验证 TLS、代理取自环境、开启 Cookie。引擎在为每次发送构建 `reqwest::Client` 时应用结果（受信任的 CA 文件把其中的证书作为根证书添加）；共享的 Cookie 存储（Cookie 管理器可以列出和编辑的 `cookie_store`）会交给该客户端，除非请求关闭了 Cookie。

**签名认证。**AWS Signature V4、OAuth 1.0a 和 Digest 是 `apix_core::sign` 中的纯函数，已对照 AWS 测试套件、RFC 7616、RFC 7636 和经典的 OAuth 1.0 示例检查，因此引擎和“复制为”的签名方式相同。引擎先构建 `reqwest::Request`，然后对它签名（AWS 的载荷哈希需要最终的正文；multipart 流以 `UNSIGNED-PAYLOAD` 发送）。Digest 先发送一次，读取 401 的质询，再在同一个连接上应答。OAuth 2.0 位于 `apix_http::oauth2`：令牌在发送前获取（客户端凭据、密码，或带 PKCE 的授权码，此时浏览器会回到 127.0.0.1 上的一个一次性监听器），按服务器、客户端、授权类型、作用域和用户保存在内存中，用其刷新令牌续期，从不写入磁盘。

一个请求文件：

```toml
name = "Create user"
method = "POST"
url = "{{base_url}}/orgs/:org/users"   # 不含查询：查询在 [[query]] 中

[[query]]
key = "notify"
value = "yes"

[[path]]                 # :org 路径段的值
key = "org"
value = "{{org}}"

[[example]]              # 与请求一起保存的响应
name = "Created"
status = 201
body = '{ "id": 7 }'
[example.request]        # 发送它的请求，按原样书写
method = "POST"
url = "{{base_url}}/orgs/:org/users?notify=yes"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false          # 保留在文件中，不发送

[body]
json = '''
{ "name": "Ada" }
'''

[auth.bearer]
token = "{{token}}"

[[assert]]               # 无需代码的检查，在测试脚本之后运行
target = "status"
op = "=="
value = "201"
```

**检查。**`[[assert]]` 行把一个目标（`status`、`time`、`size`、`body`、`body.<path>`、`header.<name>`）与一个值比较，使用 `==`、`!=`、`>`、`>=`、`<`、`<=`、`contains`、`not contains`、`exists`、`not exists`、`matches` 或 `is`（JSON 类型）。`apix_script::assertions_script` 把它们变成又一个测试脚本，因此它们在任何地方都像 `pm.test` 一样报告结果。

**正文。**`[body]` 包含以下之一：`json`、`text`、`xml`、`form`（URL 编码的行）、`multipart`（文件字段的行带 `file = true`，可选 `content_type`）、`binary = { file = "…" }` 或 `graphql`（`query` 和 `variables`）。文件路径可以包含变量，并相对于请求文件；在应用和 CLI 中，`Request::with_files_relative_to` 都会在变量解析后把它们变成绝对路径。引擎只在发送时才读取文件，因此缺少文件是发送错误，而不是加载错误。GraphQL 正文以 `{"query", "variables"}` JSON 发送；无效的变量会阻止发送。对于 multipart 正文，不带引擎边界的 `Content-Type: multipart/…` 头（在 Postman 导出中很常见）会被丢弃，以保证正文可读。

**gRPC。**URL 为 `grpc://host:port`（TLS 用 `grpcs://`）的请求在 `[grpc]` 中指明其方法；JSON 正文就是消息，对于接收流的方法，JSON 列表就是消息流。没有 `protos` 时会询问服务器（反射）。路径相对于请求文件。

```toml
name = "Say hello"
url = "grpc://localhost:50051"

[body]
json = '{"name": "Ada"}'

[grpc]
method = "helloworld.Greeter/SayHello"
protos = ["../protos/helloworld.proto"]   # 可选
import_paths = ["../protos"]              # 可选
```

除了文件，`definition` 也可以把描述保存在请求中：`.proto` 文本，或以 JSON 表示的描述符集。它优先于 `protos`。`guessed = true` 标记一个从类型或 JSON 推断出来的描述：服务器反射优先于它，差异会被指出。`protos` 也接受描述符集，二进制或 `.json` 均可。

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

待定问题：
- **机密。**令牌之类的环境值不应提交。`environments/<name>.local.toml` 叠加在 `<name>.toml` 之上。写入它时，如果 `environments/` 旁边的 `.gitignore` 中没有 `*.local.toml`，就会加上。机密可以在共享文件中保留一个占位值，这样团队成员能看到名称。也可以选择（设置 › 环境机密值）把机密改存到操作系统钥匙串：此时 `<name>.local.toml` 只包含 `keychain = "<id>"`，值保存在服务 `Apix` 的条目 `env:<id>` 中（`apix_vault::keychain::save_text`，因 Windows 的 2560 字节限制而拆分到多个条目）。`apix_core::secrets::use_keychain` 决定新的机密写到哪里；读取跟随文件，因此 CLI 不需要任何设置。id 随文件一起移动，因此重命名环境或移动文件夹都不会断开关联。在加密集合内部，机密留在 vault 中。
- **动态变量。**当没有定义同名变量时，`{{$name}}` 由 `apix_core::dynamic` 填充，每次出现都给出一个新值（`$guid`、`$timestamp`、`$isoTimestamp`、`$randomEmail`、……）。不需要额外的 crate：一个小型 xorshift 生成器和一个手写的 UTC 日期格式化器。
- **侧边栏中的顺序。**没有保存任何顺序时，先是文件夹，然后是请求，各自按名称排序。拖动会在文件夹的 `folder.toml`（或集合的 `apix.toml`）中保存一个子项名称组成的 `order` 列表；`store::place` 把一个项目放在另一个之前或之后，重命名和移动会让列表保持同步。列表不认识的名称按默认顺序排在后面，因此新文件永远不会消失。项目中的集合使用 `apix-project.toml` 中的 `order`（`Project::place_collection`）。Postman 导入器只在 Postman 的顺序与默认顺序不同时才写入 `order`。

### 项目

项目把多个集合组织在一起，并拥有它们共用的环境：

```
shop/
├── apix-project.toml       名称、共享的 [vars]、集合顺序、group_by、固定、颜色
├── environments/           test.toml、prod.toml（+ *.local.toml 机密）
├── users/                  一个集合（apix.toml + .apix 文件）
└── payments/               一个集合；可以固定到某些环境
```

`[colors]` 把集合文件夹映射到一个颜色名称（`red`、`orange`、`yellow`、`green`、`teal`、`blue`、`purple`、`pink`）；应用在侧边栏和标签页上把它画成一个圆点。`project::move_collection_into` 把集合文件夹移入项目（跨驱动器时先复制再删除）。

变量层，后面的优先：项目变量 → 集合变量 → 项目的环境 → 集合中同名的环境（每一层都带各自的 `.local.toml` 机密）。CLI（`apix run --env`）和桌面应用使用同一个函数 `apix_core::project::variables`。`group_by = "environment"` 把侧边栏排成 项目 → 环境 → 集合，`[pins]` 把集合限制在某些环境中。

### 加密集合（M4）

格式和引擎位于 `crates/apix-vault`。Apix 的其余部分通过 `apix_core::vfs` 访问它们，这是一个与 `std::fs` 调用相同的薄层：位于已解锁 vault 根目录下的路径从 vault 提供，在内存中解密；其他所有路径都访问磁盘。因此存储层、侧边栏、标签页和 CLI 继续使用普通路径，解密后的内容永远不会写入磁盘。已解锁的 vault 保存在进程级的注册表中；锁定（手动、空闲 15 分钟后或退出时）会丢弃 vault 并清除其密钥。在加密集合内删除是永久性的：系统回收站会保存解密后的文件。

```
my-api/
├── apix-vault.toml   明文头：格式、Argon2id 开销和盐、
│                     由密码和恢复密钥分别包装的数据密钥
├── .gitignore        objects/local/
└── objects/
    ├── index.apixe   所有文件的真实路径，已加密
    ├── 3f9c…e1.apixe 每个文件一个加密文件；随机名称，扁平（没有文件夹结构）
    └── local/        *.local.toml 机密，相同格式，从不提交
```

- 一个随机的 256 位**数据密钥**用 XChaCha20-Poly1305 加密每个文件，每次写入使用新的 nonce。对象 id 作为关联数据，因此数据块被调换或重命名都会被发现。
- 数据密钥保存两次：一次用从**密码**派生的密钥包装（Argon2id，64 MiB，3 遍），一次用从**恢复密钥**派生的密钥包装（160 个随机位，只显示一次，格式为 `XXXX-XXXX-…` 的 Crockford base32）。修改密码只会重新包装密钥；文件不会重新加密。
- 重命名和移动只涉及加密的索引。
- 解锁后的密钥只存在于内存中（锁定时由 `zeroize` 清除）。
- Git 仍然能看到哪个对象改变了，但永远看不到其中的内容。安全性来自密码算法和密码，而不是来自格式保密；Apix 是开源的。
- 密码可以记在操作系统凭据存储中（`apix_vault::keychain`，通过 `keyring` crate：凭据管理器、macOS 钥匙串、Secret Service）。每个 vault 在服务 `Apix` 下有一个条目，名为 `vault:<absolute folder>`（在 Windows 上为小写）。它需要在解锁提示中主动选择；应用和 CLI 在询问之前先尝试它，已无法打开 vault 的密码会被删除并重新询问，修改密码会更新它，解密集合会删除它。没有可用的存储时，Apix 像以前一样直接询问。

## 脚本（M5）

`apix_core::scripts` 找到请求的脚本（`_collection.<event>.js`、每个文件夹的 `_folder.<event>.js`，然后是 `<request>.<event>.js`；事件为 `prerequest` 和 `test`），并保存 `ScriptVars`，即脚本在各个作用域中设置或取消设置的内容。`ScriptVars::apply` 把它们叠加在请求的 `Variables` 之上，因此解析照常工作。脚本设置的任何内容都不会写入磁盘。

`apix-script` 在一个全新的 QuickJS 运行时中运行某个事件的脚本（`rquickjs`；限制为 10 s 和 256 MB）。`pm` 对象是 JavaScript（`prelude.js`）：Rust 以 JSON 传入上下文（未解析的请求、测试时的响应、来自 `project::scope_values` 的作用域值、之前的 `ScriptVars`、`pm.info`），脚本被包在函数中逐个运行，`__apix_output()` 以 JSON 返回修改后的请求和变量、测试结果、控制台输出行、`setNextRequest` 和 `skipRequest`。把 API 留在 JavaScript 中，可以让 Rust 端只需几个调用，也便于与 Postman 的行为保持一致。

`pm.sendRequest` 以 JSON 形式把请求传给原生的 `__apix_send`；`apix_script::run` 的调用方传入一个 `Sender`，由它发送请求并阻塞到响应到达（CLI 在脚本线程上的新运行时中，应用在它的网络运行时中）。脚本被包在 async 函数中，因此回调风格和 `await` 都能用，等待的时间也不计入限制。`require` 来自 `libs.js`：crypto-js 的子集（摘要和 HMAC 通过 `__apix_hash` / `__apix_hmac` 在 Rust 中完成）、uuid、lodash、moment、ajv、tv4 和 chai，它们是为 Apix 编写的，而不是下载的。

带脚本的一次发送：预请求脚本，然后 `ScriptVars::apply` 和解析，HTTP 请求，然后用 `ResponseView` 运行测试脚本。`apix_runner::exchange` 为 `apix send`、`apix run` 和 Runner 完成这些工作；桌面应用的单次发送在 `script_run.rs` 中完成，脚本在后台线程中运行，请求自己的脚本取自编辑器，无论是否已保存。

`apix_runner::run` 接收一个 `Plan`（按顺序的请求文件、变量和脚本作用域、数据行、迭代次数、延迟、`bail`、传输覆盖、起始脚本状态）并逐项执行：每次迭代把数据行（然后是 `--var` 的值）叠加在变量和 `pm.iterationData` 之上，按请求名称跟随 `setNextRequest`，在请求之间等待，并向回调报告 `Progress`（迭代、正在发送、项目完成）。`Control` 可以从另一个线程暂停或停止它。它返回由 `ItemResult` 组成的 `RunReport`，`report.rs` 把它写成 JSON、JUnit XML（每个请求和迭代一个套件，每个测试一个用例，没有测试的请求有一个状态用例）和一个自包含的 HTML 页面。`data.rs` 读取 CSV（RFC 4180、BOM、`;` 分隔的表头）和 JSON 数组。CLI 随时打印进度；桌面 Runner（`workspace/runner_ui.rs`）在网络运行时中运行它，每秒几次从共享的 `Live` 结构中复制出进度；视图保留在工作区中，因此运行的生命周期比它的对话框更长。

## 流和 WebSocket（M6）

`HttpClient::send_with` 在状态和头一到达就报告 `Progress::Head`，然后报告正文的每个 `Progress::Chunk`；`send` 就是没有监听者的 `send_with`，响应仍然包含完整的正文。`apix_http::sse` 逐段读取 `text/event-stream` 正文（`SseParser`：任意行结束符，片段可以在任意位置分割，`id` 会延续，未完成的最后一个事件被丢弃）。解析器还会为重新连接保存最后的 id 和最后的 `retry:`：`reconnected()` 用这两者为下一个连接启动解析器，`reconnect_request` 添加 `Last-Event-ID`。等待和重新发送由应用和 CLI 自己完成，因此 Esc 或 Ctrl+C 总能停止它。

`apix_http::ws::connect` 按 HTTP 端会发送的方式从请求构建握手（`prepare`：参数、头、认证；OAuth 2.0 先变成 bearer 令牌），根据 `[websocket] subprotocols` 添加 `Sec-WebSocket-Protocol`，并用 tokio-tungstenite 连接。`wss://` 从 `apix_http::tls::client_config` 获得 rustls 配置（常规根证书加上受信任的 CA、客户端证书，或接受任何证书的验证器），gRPC 在不验证时也使用同一个模块。`WsConnection::run` 发送 `WsCommand` 通道要求的内容并报告 `WsEvent`（已发送、已接收、带代码和原因的关闭、错误），直到任一方关闭；服务器的 ping 由 tungstenite 应答。设置了 `[websocket] ping_interval` 时，它自己也会发送 ping，以计数器作为载荷，报告对应 pong 的往返时间（`WsEvent::Pong`），并在下一次 ping 到期时上一次仍未应答的情况下以错误结束。消息的 `MessageFormat`（text、base64、hex）在填入变量后把其文本变成 `WsData`。

在桌面应用中，网络运行时填充共享结构（发送用 `net::Incoming`，套接字用 `net::SocketIncoming`），一个界面任务每 60–80 ms 读取一次。事件流响应变成 `ResponseState::Streaming`（`workspace/stream_ui.rs`），带有一个实时事件列表，除非向上滚动，否则会跟随新事件；Esc 把已到达的内容变成一个完成的响应。`ws://` URL 会把请求面板切换到“消息”标签页（`workspace/ws_editor.rs`），把响应面板切换到 `ResponseState::Socket`（`workspace/socket_ui.rs`）。这两种状态都像完成的响应一样被路由到各自的标签页，因此流或套接字会在后台的标签页中继续运行。CLI 通过同样的回调打印事件和消息。

## Socket.IO（M6）

`apix_http::socketio` 在上述 WebSocket 连接之上实现 Socket.IO 3/4（Engine.IO 协议 4），因此握手、头、认证和 TLS 设置都相同。带有 `[socketio]` 表（`SocketIoOptions`：命名空间、路径、认证）的请求就是 Socket.IO 请求；`handshake_request` 把 `http(s)://` 变成 `ws(s)://`，从选项或 URL 的路径中取得命名空间，在其位置放上服务器的路径（`/socket.io/`），并添加 `EIO=4&transport=websocket`。`connect` 等待 Engine.IO 的 open 数据包，用认证 JSON 发送命名空间的 CONNECT，并在返回前等待应答（带套接字 id 的 `40…`，或带拒绝原因的 `44…`），因此错误的服务器、命名空间或令牌会像握手失败一样失败。

`SioConnection::run` 在两个通道之间的转换器旁运行 `WsConnection::run`：`SioCommand::Emit` 变成 `EVENT` 数据包（`42…`，需要时带确认 id，id 从 0 开始计数）或 `BINARY_EVENT`（`45n-…`），其中的 `{"$binary": "<base64>"}` 值被替换为占位符，并在其后以二进制帧发送；收到的数据包变成 `SioEvent`（收到的事件及服务器需要的确认 id、我们的确认、服务器让我们离开、无法读取的帧），二进制附件会被收集起来并放回占位符所在的位置。Engine.IO 的 ping（`2`）会得到 pong（`3`）。

桌面应用为两者共用一个“消息”标签页：`WsEditor` 有一个 WebSocket/Socket.IO 切换开关和 Socket.IO 字段，`WsMessage` 携带 `event` 和 `ack`。`net::SocketIncoming` 对事件类型是泛型的，`socket_ui.rs` 用一个循环跟随两种连接，把 `SioEvent` 变成带事件名称和确认标签的日志条目。`apix-http/tests/common/socketio_server.rs` 中的测试服务器同时也是 `socketio_server` 示例（:8771）。

## MQTT（M6）

`apix_proto::mqtt` 封装了 rumqttc 的事件循环（MQTT 3.1.1；它的 TLS 只通过 `apix_http::tls::client_config` 的配置，因此不会带来自己的加密提供方）。`mqtt://` 或 `mqtts://` URL 让请求成为 MQTT 请求，就像 `grpc://` 之于 gRPC；`[mqtt]` 包含客户端 id、会话、keep alive 和订阅。`connect` 轮询事件循环直到代理返回 CONNACK，因此被拒绝的登录会像握手失败一样失败。

`MqttConnection::run` 并排运行两个 future，从不取消事件循环的轮询：一个把 `MqttCommand` 交给客户端（客户端在队列满时可能等待，而该队列由轮询清空），另一个跟随连接并报告 `MqttEvent`。rumqttc 按交出的顺序报告每个外发数据包的包 id，因此一个队列把发布和订阅与其 id 对应起来，把代理的 PUBACK、PUBCOMP、SUBACK 和 UNSUBACK 与它们确认的内容对应起来。消息的数据包发出后，该消息即报告为已发送。错误的主题或被拒绝的订阅是一个事件，连接继续；关闭以及代理挂断会结束连接。

在桌面应用中，`WsEditor` 也包含 MQTT 字段（由 URL 决定，`Workspace` 告诉它），`socket_ui.rs` 用与 WebSocket 和 Socket.IO 相同的循环跟随连接，把 `MqttEvent` 按主题变成日志行。`apix-proto/tests/common/broker.rs` 中的测试代理（通配符、QoS 0–2、保留消息、拒绝、通过任意流的 TLS）同时也是 `mqtt_broker` 示例（:8772）。

## SOAP（M6）

SOAP 没有增加新的请求类型：`apix_import::wsdl` 把 WSDL 1.1 变成带 XML 正文的普通 POST 请求，因此对请求有效的一切对它们也有效。`apix_core::soap` 包含各版本之间的差异（头、信封）并读取故障；`Response::soap_fault` 把它应用到 XML 应答上，Runner 会让没有测试的请求在遇到故障时失败，无论其状态如何。

导入器不获取任何内容。`Loader` 遍历 `wsdl:import`、`xsd:import` 和 `xsd:include`：它说明下一个要读取的位置，并接收回文本，因此 CLI 和桌面应用各自用自己的方式读取文件和网络。位置相对于其所在文档解析，来自网络的文档不能指向本地文件，读取在 64 个文档时停止。然后 `Model` 从所有文档中收集消息、端口类型、绑定、端口和模式的全局定义，以命名空间和名称为键。`Writer` 从 XSD 节点构建示例（roxmltree 保存这些节点，并根据节点自己的声明解析 `prefix:name`），并根据 `form`、`elementFormDefault` 和是否为全局元素，为每个元素决定是否限定。它用到的命名空间放在信封上。一个记录正在写出的命名类型的栈会阻止递归。

应用和 CLI 中的 URL 导入先读取文本，然后走与文件相同的路径，因此适用于每种格式。`apix-import/tests/fixtures/` 中的测试 WSDL 也驱动着 `soap_server` 示例（:8773）。

## GraphQL 模式（M6）

`apix_core::graphql` 包含标准的 `INTROSPECTION_QUERY`、从其应答读取的 `Schema`（带字段的类型、参数、输入字段、枚举值、接口和可能的类型；`TypeRef` 保留列表和 `!`），以及 `to_sdl`，后者先写出根类型，并省略内置标量和 `__` 类型。为了编辑器，它只把查询读到光标处：`spot_at` 对文本进行分词（丢弃注释、字符串和 `$variables`），并遍历由选择集、参数列表、输入对象和列表组成的栈，跟随别名、`... on Type`、片段和变量定义，以说明那里适合填什么（`Spot`）。`suggestions` 把它变成带类型和描述的标签，按前缀匹配、再按子串匹配排序，已弃用的排在最后；`describe_at` 给出悬停用的 Markdown。不做任何验证：有错误的查询仍会尽可能地补全。变量编辑器也会补全：`variable_types` 读取 `$name: Type` 声明，一个小型 JSON 扫描器找到直到光标处的键和列表项路径，以及那里应该是键还是值，`variable_suggestions` 沿该路径经过输入字段和列表项，建议尚未给出的变量、输入字段、枚举值（作为 JSON 字符串）以及 `true`/`false`；正在输入的字符串会连同引号一起被替换，包括编辑器自动闭合的引号。

`apix_http::graphql::fetch_schema` 用请求的 URL、头、认证和设置以 POST 发送内省查询（请求先被解析），并读取模式，或者说明为什么没有模式（非 2xx 状态及正文开头，或服务器的 `errors[0].message`）。

在应用中，`workspace/graphql_ui.rs` 在查询编辑器持有的共享 `Rc<RefCell<Option<Rc<Schema>>>>` 之上实现 gpui-kit 的 `CompletionProvider` 和 `HoverProvider`；补全通过 LSP 文本编辑替换正在输入的单词。请求体编辑器的 **获取架构** 发出 `FetchSchema`；工作区像发送时一样构建请求（继承的认证和设置、变量），并在网络运行时中获取。每个端点的应答保存在 `<config>/Apix/graphql/<FNV-1a of the URL>.json` 中，并在打开 GraphQL 请求时加载。SDL 视图是一个只读编辑器，在绘制时填充，且只在显示时填充一次。

## gRPC（M6）

`apix-grpc` 不用生成的代码进行调用。服务描述是一个 `prost_reflect::DescriptorPool`：来自请求的 `.proto` 文件，通过 protox（Rust，不需要 `protoc`；导入在 `import_paths` 中和每个文件旁边查找，Google 的知名类型已内置），或者通过 gRPC 反射来自服务器（先 v1，再 v1alpha；服务器漏掉的文件按名称请求，然后按依赖顺序添加）。消息是 `DynamicMessage`：输入的 JSON 通过 prost-reflect 的 serde 支持读取，输出的 JSON 使用 `.proto` 字段名并显示默认值。`DynamicCodec` 把它们接入 tonic，因此 `tonic::client::Grpc` 在一个 `Channel`（基于 HTTP 引擎已使用的 hyper 和 rustls 的 HTTP/2）上运行全部四种方法（一元、服务器流、客户端流、双向）。

`apix_grpc::convert` 从现有的其他内容生成描述，依次尝试：JSON 格式的描述符集；`.proto` 文本；生成的代码中嵌入的 `FileDescriptorProto`（`embedded.rs` 知道每个生成器如何写出这些字节，PHP 写出的甚至可能不是有效文本）；然后是编程语言中的声明（`source.rs`：去掉注释、找到代码块、按语言家族读取成员、规范化类型、从注解和 ts-proto 的编码器中取得编号）或 JSON 示例（`json.rs`，保留键的顺序，而 `serde_json::Value` 会丢失顺序）。后两者经过一个小型 `Schema`，再渲染成 `.proto`。`printer.rs` 把精确的描述写回 `.proto` 文本，只有当该文本编译后得到相同的描述时才采用；否则，或者有多个文件时，定义保持为 JSON 格式的描述符集。`compare` 把定义与服务器的描述逐一对照，按名称匹配字段，因为消息名称从不出现在传输中。`described` 规定顺序：请求的定义、它的文件、反射；推测出的定义会先尝试反射并让位于它。`call` 报告 `DynamicMessage` 无法放置的字段（`CallEvent::Note`），因为其他地方都不会报告。

请求的头和认证变成元数据（Bearer、Basic、API 密钥头、通过 `apix_http::oauth2` 的 OAuth 2.0；`-bin` 键使用 base64）；对 HTTP 请求签名的认证类型会被拒绝并说明原因。超时、客户端证书和受信任的 CA 来自请求设置。关闭 TLS 验证时，tonic（它没有这样的开关）得到一个普通的 `http://` 地址和真实的 `https` 源，由我们自己的连接器（`insecure.rs`）完成 TLS：使用 ALPN h2 的 rustls，以及一个接受任何证书但仍检查握手签名的验证器。`call` 在事件发生时报告 `Sent`、`Headers` 和 `Received` 事件，并以 `CallOutcome` 结束：状态码和消息、响应元数据、trailer 和每条消息。非 OK 的状态是一种结果，而不是错误；错误是阻止调用发生的情况（没有连接、没有描述、消息与其类型不匹配）。

在应用中，`grpc://` URL 会把“请求体”标签页变成“消息”标签页（`workspace/grpc_editor.rs`）：一个可搜索的方法选择器，由 **加载方法** 填充，`.proto` 文件，以及带有根据输入类型构建的示例的消息。调用在网络运行时中运行，并填充一个界面每 60 ms 读取一次的共享结构；响应面板是一个日志（`ResponseState::Grpc`，`workspace/grpc_ui.rs`），与 WebSocket 的类似，Esc 会放弃调用。CLI 完整打印一元应答，逐行打印流，`apix schema` 把服务写成类似 `.proto` 的文本。

运行通过 `apix_grpc::send_for_tests` 调用 gRPC 请求，它把 `CallOutcome` 变成 `apix_http::Response`（`CallOutcome::to_response`）：代码对应的 HTTP 状态（Google 的映射），以代码名称作为原因短语，元数据和 trailer 加上 `grpc-status` 和 `grpc-message` 作为头，消息（服务器流为 JSON 列表）作为正文。这样测试脚本、检查和报告就不需要任何 gRPC 专用的东西。历史记录在保存调用时设置 `Summary::grpc`：状态是 gRPC 代码，trailer 和状态消息有各自的字段，正文是日志（`[{"at_ms", "sent", "message"}]`），`grpc_ui::restored` 会把它还原成日志视图。

## 并行运行和监视器（M7）

`Plan::parallel` 大于 1 时会同时运行这么多次迭代（在运行自己的任务上使用 `futures_util` 的 `buffer_unordered`，不用线程）：每次迭代用自己的一份脚本状态副本按顺序发送请求，在 `bail` 下一个失败会停止其他迭代，报告之后按迭代排序。进度通过包住调用方回调的互斥锁传递。

`apix_core::monitor` 描述 `apix.toml` 中的 `[[monitor]]`（名称、`every`、`run`、环境、webhook、enabled）并读取间隔；`apix_runner::monitor` 运行一个监视器（`plan` 像 `apix run` 一样解析变量：监视器的环境、`APIX_VAR_*`，然后是固定的值），汇总它（`Outcome`），为每个监视器保存一个 JSON 行格式的 `Log`（`<config>/Apix/monitors/<hash of the collection>-<name>.jsonl`，最近 500 条），决定每个监视器何时到期（`Scheduler`：第一次看到的监视器，在距上次记录的运行已过去其间隔时到期），并在状态转变时通知 webhook（`change`、`notify`）。`apix monitor` 和桌面应用（`workspace/monitors_ui.rs`：设置开启时每秒一次计时，在网络运行时中一次运行一个）都使用它，因此它们共享日志。

## Git（M7）

`apix_core::git` 运行已经安装的 `git` 程序（不用 git 库，因此不会给二进制文件增加任何东西，并且用户自己的配置、钩子和凭据都适用）。`Repo` 在集合或项目文件夹处打开，每条命令都带 `--relative` 在那里运行，因此路径相对于该文件夹，位于更大仓库中的集合只看到它自己的文件。`changes_since(rev)` 是 `git diff --merge-base rev`（分支离开 `rev` 的地方，就像拉取请求一样）加上未跟踪的文件；没有历史记录时（浅克隆）它退回到 `rev` 本身，并说明这一点。`show(rev, path)` 读取某次提交时的文件。

`git::affected` 按角色（`git::role_of`）把更改的文件映射到请求：请求文件或其 `<stem>.*.js` 脚本 → 该请求；`folder.toml` 或 `_folder.*.js` → 该文件夹下的请求；`apix.toml`、`_collection.*.js`、`environments/*.toml`、项目文件或项目的环境 → 全部请求；任何其他文件 → 文件中引用了它的请求。`apix run --changed` 保留这些请求，报告记录这次选择（`RunReport::selection`）。

`apix_core::diff` 按含义比较文件的两个版本：请求被解析为 `Request` 并序列化为 JSON（因此默认值和格式无关紧要），其他 TOML 解析为值，脚本和其他文本逐行比较（最长公共子序列，先去掉相同的开头和结尾）。具有不同 `key` 或 `name` 的行列表（头、查询、`[[example]]`）变成以它为键的表，因此行按名称匹配；字段以其路径命名（`headers.Accept`、`body.json`）。内容相同的一个删除的请求和一个新增的请求变成一次移动。`apix diff` 以文本、Markdown 或 JSON 打印结果。

`RunReport::to_markdown` 为 GitHub 写出运行结果（每个请求一行表格，失败放在展开的 `<details>` 中），以一个隐藏的 `<!-- apix-run: <name> -->` 标记开头，action 的评论步骤通过查找它来编辑之前的评论。它保持在 GitHub 的 65 536 字符限制以内。

桌面应用（`workspace/git_ui.rs`）在每次重新加载后以及窗口被激活时，在后台读取打开文件夹的分支和 `git status`，并为侧边栏的标记保存一份更改路径的映射；自身脚本更改了的请求也算作已更改。

Git 窗口（`workspace/git_panel.rs`）使用同一个 `Repo`：列表用 `working_changes`，选中的文件用 `show("HEAD", path)` 通过 `diff::file` 与磁盘上的文件比较，还有 `commit_folder`（`git add -A -- .`，然后 `git commit -- .`，因此只涉及打开的文件夹）、`branches`、`switch`、`create_branch`（`switch -c`）、`pull`（`--ff-only`）、`push`（没有上游时为 `-u origin <branch>`）和 `log`（哈希、时间、主题）。Git 以 `GIT_TERMINAL_PROMPT=0` 运行，因此需要密码的远程仓库会以 git 的消息失败，而不是等待输入；凭据助手仍然有效。每个操作都在后台运行，并刷新状态栏。

## AI 助手

`apix_core::ai` 为三个提供方之一手工构建请求（Rust 没有它们的官方 SDK）并读取应答。`Provider`（`anthropic`、`openai`、`google`）给出默认模型、密钥在钥匙串中的账户（`ai:<provider>-api-key`，通过 `apix_vault::keychain::save_text`）、它的环境变量和 URL。Anthropic：`POST /v1/messages`，带 `x-api-key`、`anthropic-version: 2026-10-21`、`claude-opus-5-5`，对于默认模型还带服务器端回退（`fallbacks: "default"`，beta `server-side-fallback-2026-07-01`）；`refusal` 停止原因会变成一条消息。OpenAI：`POST /v1/chat/completions`，带 bearer 密钥、一条系统消息和一条用户消息；`refusal` 字段视为拒绝。Google：`POST /v1beta/models/{model}:generateContent`，带 `x-goog-api-key`、`systemInstruction` 和 `contents`；`SAFETY` 结束和被拦截的提示视为拒绝，思考部分被跳过。每个提示都包含这次交互（`Context::describe`：凭据头被遮盖，正文截断到 12 KB）、任务和应用的语言代码。`models_request` / `models` 列出密钥可以使用的模型（OpenAI 只列聊天模型，Google 只列支持 `generateContent` 的模型）。`APIX_AI_URL` 在测试时替换基础 URL。应用（`workspace/ai_ui.rs`）通过 `net::send` 发送，用 `ScriptEditor::append_test` 追加测试，并在对话框中显示解释；“设置”保存 `ai_provider` 和 `ai_model`。

## MCP 服务器

`apix mcp`（`apix-cli/src/mcp.rs`）从标准输入每行读取一条 JSON-RPC 2.0 消息，并向标准输出每行写一条应答（MCP 的 stdio 传输；第一行之前的字节顺序标记会被跳过）。`initialize` 在认识所请求的协议版本时（2025-06-18、2025-03-26、2024-11-05）用该版本应答，否则用最新版本应答，并且只提供工具；通知不会得到应答，未知方法返回错误 -32601。`tools/call` 的结果同时以文本和 `structuredContent` 携带 JSON，失败的工具是一个带 `isError` 的结果，而不是协议错误，因此代理可以读到原因。来自代理的路径会拼接到集合根目录上，解析 `..`，超出根目录的路径被拒绝。`send_request` 经过 `apix_runner::exchange`（脚本、变量、测试），`run_requests` 经过 `apix_runner::run`；变量来自与其他命令相同的 `VarArgs::resolve`，代理的 `variables` 作为 `--var` 使用。CLI 在一个 16 MB 的线程上运行，因为在 Windows 上，各命令共用的 future 超出了主线程的栈大小。

## 设置向导和 Claude

`workspace/setup_ui.rs` 会打开一次设置向导（`settings.setup_shown`），之后可从“帮助”中打开。数据库按钮运行 `route_ui::download_geo`（本月的 DB-IP 文件，没有则用上月的，通过应用的客户端下载，在界面线程之外解压）；`apix geo download|status|remove` 在终端中做同样的事，安装程序的可选步骤将调用的就是它。`apix_core::claude_mcp` 找到 Claude Desktop 的配置（`%APPDATA%\Claude`、`~/Library/Application Support/Claude`、`~/.config/Claude`；测试时用 `APIX_CLAUDE_CONFIG`），把服务器命名为 `apix-<collection>`，用 `mcp <folder> [--list-only]` 指向正在运行的程序旁边（或 PATH 上）的 `apix`，并把它合并到 `mcpServers` 中，同时保留其他所有内容（原文件复制为 `.bak` 一次）；`claude_code_command` 是 `claude mcp add --scope user`。

## 流程

`apix_core::flow` 定义文件：一个 `Flow` 包含节点（`NodeKind::Request`、`Check`、`Set`、`Delay`，展开到各自的 `[[node]]` 表中，连同其 id 和位置）以及从一个节点的结果（`ok`/`fail`、`yes`/`no`、`next`）指向另一个节点的边。`problems()` 列出阻止流程运行的问题；`Seen` 是最近的响应，`Seen::subject` 通过 `apix_core::filter` 读取 `status`、`time`、`body`、`header:Name`，或一个 JSONPath 或 jq 过滤器。`.flow` 文件可以放在集合中的任何位置；侧边栏会忽略它们。

`apix_runner::flow::run` 从起始节点开始遍历图。请求节点像运行时一样经过 `exchange`，因此脚本、测试和 `pm.*` 变量都有效；它保存的值设置在脚本状态的局部作用域中，对其后的节点来说优先于环境，就像 `pm.variables.set` 一样。取值对象是文本的检查会解析 `{{variables}}`，某个变量未定义时就没有值。运行会报告每一步（`FlowProgress`），并在 1000 步后或通过 `Control` 停止。`FlowReport::to_run_report` 复用运行报告（JUnit、HTML、Markdown）。桌面编辑器（`workspace/flows_ui.rs`）把节点画成定位的元素，把边画在画布上：向前的边画成曲线，向回的边画成绕过节点的折线；它在网络运行时中运行流程，并像 Runner 一样用定时器读取进度。

## 插件

`apix-plugins` 用解释器 `wasmi` 2 运行 WebAssembly 插件：没有 JIT，因此不会从插件生成原生代码，并且凡是 Rust 能构建的地方它都能构建。模块不能导入任何东西，因此插件根本没有宿主函数（没有网络、文件、时钟或随机数）；它导出 `memory`、`apix_alloc`、`apix_manifest` 和 `apix_call`，应答是以 `ptr << 32 | len` 传递的 JSON。每次调用都会重新实例化模块（调用之间没有状态），带 2·10⁹ 单位的燃料和 256 MB 的内存限制，运行在一个 64 MB 栈的线程上；燃料耗尽报告为 "ran too long"。清单在插件加载时读取一次；它的 `name` 必须由 `[a-z0-9-]` 组成。`installed()` 会缓存插件文件夹（`<config>/Apix/plugins` 或 `APIX_PLUGINS_DIR`），直到调用 `reload()`。

这些钩子在三个地方与 Apix 的其余部分衔接：模型中的 `Auth::Plugin { plugin, config }`，像其他认证一样解析（配置中的变量），并由 `apix_http::prepare` 应用到最终的请求上；`apix_import::plugin::plan`，它把导入器的应答变成请求文件；以及响应的可视化器（`VisualMode::Plugin`），它在界面线程之外运行插件并绘制表格或文本。wasmi 即使在 debug 构建中也以 `opt-level = 3` 且不带调试断言构建，因为它的分派依赖尾调用，否则会耗尽栈空间。

## 比较响应

`apix_core::compare` 接收两个 `Snapshot`（标签、状态、头、正文、时间）。头按小写名称比较，同名的多个头合并在一起；`VOLATILE` 中的头（日期、缓存、请求和跟踪 id、速率限制、正文的传输方式）只计为已省略。两个 JSON 正文经过 `by_id`，它把具有不同 `id` / `_id` / `uuid` / `key` 值的记录列表变成以 `[id=…]` 为键的表，然后经过 `diff::fields_as_they_are`（请求差异的遍历，但不做键/名称匹配，那种匹配适合请求文件而不适合数据）；其他文本逐行比较，其他内容只比较大小。应用的“比较响应”窗口（`workspace/compare_ui.rs`）从历史中取较早的一方（同一请求文件的条目，或方法和 URL 相同的条目），或者用另一个环境的变量解析请求后发送；`apix compare` 在两个环境中发送它。

## 负载测试

`apix_http::load::Load` 为一个已解析的请求构建一个 reqwest 客户端（请求的 TLS 设置通过 `tls::client_config`，以及它的代理、超时和重定向策略，不限大小的空闲连接池），`hit()` 发送它，统计正文的字节数但不解码；OAuth 2.0 只变成一次 bearer 令牌，Digest 被拒绝。`apix_runner::bench::Bench::run` 在一个 `FuturesUnordered` 中启动 `concurrency` 个工作单元；每个工作单元从一个原子计数器取下一个请求编号，直到数量、截止时间或 `stop()` 说不，设置了速率时则等到 `start + n / rate`。每次请求都在互斥锁下记录：以微秒计的延迟（每个请求一个 `u32`，因此一百万个请求占 4 MB）、状态、按类型分类的错误（"could not connect"、"timed out"、……）、字节数，以及按秒划分的桶。`summary()` 可以在运行期间调用：它对延迟的副本排序，以计算最近秩法百分位数。CLI 在自己的运行时上运行测试，每个核心一个线程，每秒打印一行；应用在它的网络运行时上运行，每秒重绘四次。

## 路由与地图

`apix-route` 启动系统自带的工具，就像 `apix_core::git` 启动 git 一样：Windows 上是 `tracert -d -h N -w 1000`，macOS 和 Linux 上是 `traceroute -n -q 1 -w 1 -m N`，没有 traceroute 时是 `tracepath -n -m N`。它们都不需要管理员权限，而在 Windows 上自己发送 ICMP 则需要。`parse_line` 只根据数字和地址读取它们中任意一个的输出行（先是跳数，每个 `N ms`，第一个能解析为地址的词），因此使用自己代码页的翻译版 Windows 也能同样读取；一跳的时间取其几次尝试中最好的一次，`<1 ms` 计为半毫秒。`Trace` 按工具打印的顺序迭代各跳，并在被丢弃或停止时结束该工具进程。

`Geo` 打开一个 MaxMind DB 文件（`maxminddb`），为公网地址给出一个 `Place`（纬度、经度、英文城市名、ISO 国家代码）；私有、环回、链路本地、CGNAT 和组播地址从不查询。数据库是 `<config>/Apix/geo` 中排在最前的 `.mmdb`；`install` 解压下载的文件（gzip），检查它能打开，然后才替换旧文件。`dbip_urls` 给出本月和上月的 DB-IP Lite 地址（`APIX_GEO_URL` 可替换它们）。地图是 `land::LAND` 的等距圆柱投影，各大洲和大岛屿是手工绘制的，精度约两度（黑海和里海在 `land::WATER` 中），因此不需要下载或授权任何地图数据。桌面窗口（`workspace/route_ui.rs`）在一个线程上运行跟踪，各跳一到就放到地图上，每次运行应用只读取一次数据库，并用 GPUI 路径绘制地图。`apix trace` 打印各跳及其位置，以及每段超过 100 km 的跳跃的距离。

## 更新检查

`apix_core::update` 构建请求（11R5/Apix 在 GitHub 上的 `releases/latest`，`User-Agent: Apix/<version>`，10 s 超时；`APIX_UPDATE_URL` 可指向别处）并读取应答：`Version` 解析 `v1.2.3`、`1.2` 和预发布版本（`1.0.0-rc.1` 排在 `1.0.0` 之前，数字部分按数值比较），404 表示还没有发布任何版本，其他错误带有 GitHub 的消息。CLI（`apix update`）用自己的客户端发送；应用（`workspace/update_ui.rs`）在 `settings.check_updates` 开启时于启动时通过 `net::send` 发送（只显示更新的发布版），也可以从 帮助 › 检查更新… 发送（显示每个应答）。不会下载或安装任何东西。

## 主题

`theme.rs` 保存颜色。一个 `ThemeDef` 就是一个主题（内置的，或从 `<config>/Apix/themes/*.toml` 读取；文件中省略的颜色来自 Obsidian 或 Daylight），带有 `Colors` 和一个深色/浅色标志。`Palette::build` 把它变成视图绘制时使用的内容：强调色设置（一个带深色和较深浅色色调的命名颜色，或 `#RRGGBB`）、按相对亮度选择的强调色上的文字颜色、深色或浅色下的悬停和输入框色调，以及透明度设置：窗口背景的 alpha 为 `1 - t`，面板以该比率的 60% 变透明，边框随 `t` 增大而趋向白色，浅色主题使用 `t` 的 70%。`from_settings` 根据外观设置选择深色或浅色主题，“跟随系统”时则根据 `App::window_appearance` 选择。

gpui-kit 的根视图在整个窗口下方绘制 `Theme::background`，这会遮住操作系统的模糊效果，因此只要有透明度，它就被设为透明，对话框则通过 `bidi::DialogBidi`（每个对话框都使用它）获得自己的背景。模糊由操作系统完成：`WindowBackgroundAppearance::Blurred` 在 Windows 上是亚克力效果（`SetWindowCompositionAttribute`），在 macOS 上是 vibrancy。

视图从不保存调色板的副本：它们在绘制时调用 `theme::current()`。`theme::set` 保存新的调色板，把 gpui-kit 的组件主题指向它（切换其模式，以便代码编辑器的颜色跟着变化），把每个窗口的背景设为模糊或不透明，并刷新窗口，因此更改会立即显示。工作区观察窗口外观，并在系统在浅色和深色之间切换时再次调用它。

## 语言（M4）

桌面界面中的每段文本都通过一个键获取：`tr!("menu.file")`（`rust_i18n::t!` 的薄包装，返回用于显示的 `SharedString`；`tr_plain!` 为需要保存的内容给出同样的文本，例如新请求的默认名称）。rust-i18n 与 gpui-kit 使用的是同一个系统，因此两者遵循同一个进程级区域设置；中文使用 `zh-CN`，即 gpui-kit 自己的文本所用的代码。每种语言是一个扁平文件 `crates/apix-desktop/locales/<code>.yml`（每行一个 `key: value`，已排序，使用 `%{name}` 占位符），编译进二进制文件；文件更改时 `build.rs` 会重新构建。缺失的文本回退到英语；当某种语言缺少键、有未知的键，或使用了与英语不同的占位符时，单元测试会失败。句子是带占位符的完整键，从不由片段拼接而成，因为不同语言的语序不同。

语言在“设置”中选择（或在第一次启动时跟随系统语言），并立即切换：每次渲染时都会读取文本，少数保存在控件内部的文本（输入框占位符、选择器条目）由 `Workspace::retranslate` 重新设置。日文、中文和阿拉伯文的字形来自平台的字体回退（DirectWrite、Core Text、fontconfig），与内嵌的 Inter 字体并用。阿拉伯文从右到左进行字形整形和排布；在 Windows 上这需要 `vendor/gpui-pre-windows`，即修复了字形串排布的 gpui 0.3.9 DirectWrite 后端（通过工作区的 `Cargo.toml` 打补丁；上游修复后即可去掉）。gpui 无法对从右到左的文本自动换行：它把一个段落整形为一行，并在它假定从左到右递增的字形 x 位置处截断。因此可能换行的文本（说明、提示、描述、消息）会经过 `bidi::para`，它在 RTL 中按阅读顺序在空格处断行，测量每个候选行，并以不换行的方式绘制每一行（`src/bidi/paragraph.rs`）。在其他语言中，它就是普通的 gpui 文本。gpui 还会以从左到右的基础方向排布每一行，因此在 RTL 语言中，`tr!` 会把每段文本包在一个从右到左的嵌入中（U+202B…U+202C；DirectWrite 会忽略较新的隔离字符）。这样，以拉丁词开头或结尾的句子（"URL"、"OAuth 2.0"、"Ctrl+S"）仍然从右到左阅读，`i18n::display` 对组合文本（例如状态栏）也做同样的处理。这些标记永远不会进入文件：保存的文本使用 `tr_plain!`。引擎 crate 的错误消息保持英文。

在从右到左的语言（`Language::rtl`，阿拉伯语）中，窗口会镜像：侧边栏移到右侧，响应面板移到请求面板的左侧，窗口控制按钮移到左侧，菜单、标签栏、表单行和状态栏从右侧开始排列，文本右对齐。GPUI 没有布局方向，因此这由 `src/bidi.rs` 完成：每个带样式的元素上的 `Bidi` trait（`dir_row` 反转一行，`text_start` / `text_end`、`ps` / `pe` 内边距和 `border_e_1` 按方向选择边，`ltr` 把文本固定在左侧）、用于 gpui-kit `TabBar` 的 `tab_bar` / `fill_tabs`（标签页逆序给出，索引再映射回来），以及用于对话框的 `bidi_title`（在 RTL 中，关闭按钮移到左侧的标题行中）。这些辅助函数在视图渲染时读取语言，因此更改语言会立即镜像窗口。代码和响应查看器、URL 栏、键值表（参数、头、表单字段、变量）和响应头在每种语言中都保持从左到右。gpui-kit 的弹出菜单和按钮保持自己从左到右的内部布局（快捷键在右侧，图标在标签之前）；菜单从其按钮向左打开。

添加一种语言：把 `locales/en.yml` 复制为 `locales/<code>.yml`，翻译其中的值，并在 `src/i18n.rs` 的 `LANGUAGES` 中加一行。

用户文档与计划分开存放，每种语言一个文件夹（`Docs/<code>/`：用户指南、架构、技术说明、工作流程、图片），列在 `Docs/languages.toml` 中。英文是源文本；每份翻译都记录它所依据的英文版本，因此过期的翻译会显示出来。

## 线程模型（桌面）

- GPUI 拥有主线程，主线程运行事件循环和渲染。
- 一个多线程的 **tokio 运行时**在后台线程上启动一次。reqwest、tonic 和 tungstenite 需要它。
- 界面把工作交给 tokio，并在一个 GPUI 任务（`cx.spawn_in`）中接收结果。丢弃该任务会中止请求，因此 Esc 和重新发送都能干净地取消。
- 响应到达后，正文先在 GPUI 的后台执行器上格式化并建立索引，然后才交给界面。格式化使用 `apix_core::text::pretty_json`，这是一个单遍的重新缩进器，不构建树，并且数字完全保持发送时的样子。

## 大型响应查看器

512 KB 及以上的正文使用 `large_view.rs`，而不是代码编辑器：

- `apix_core::text::RowIndex` 记录每个显示行的起始位置，每行 8 字节。超过 2 KB 的行（压缩过的 JSON）会被拆分成多行。
- 滚动位置是一个**行号**，而不是像素偏移。一千万行、每行 20 px 约为 2 亿 px，远远超出 `f32` 像素保持精确的范围。查看器只布局屏幕上能容纳的约 120 行。它自己处理滚轮、自己的滚动条和键盘（↑ ↓ PgUp PgDn、Ctrl+Home/End）。
- 大型 JSON 正文的“原始”标签页只在打开时才构建。
- Ctrl+F 在后台线程上搜索整个文本（对字面查询使用不区分大小写的 `regex`），最多保留 100,000 个字节范围；`RowIndex::row_of` 用二分查找找到匹配项所在的行，只有屏幕上的行才查找各自的匹配项并高亮它们。

较小的正文放在代码编辑器中，并开启编辑器自带的搜索面板。“美化”上方的过滤器（`workspace/filter_ui.rs`）在最后一次按键 250 ms 后，在界面线程之外运行 `apix_core::filter`：以 `$` 开头的表达式通过 `serde_json_path` 作为 JSONPath 处理，否则通过 `jaq` 作为 jq 处理，jq 最多输出 10,000 项。它的结果和任何正文一样是一个 `Document`，因此过滤后的大型正文可以进入编辑器，反之亦然。

在 Windows 上用 release 构建测量，从本地服务器（`cargo run -p apix-http --release --example big_json_server`）获取 100 MB 压缩过的 JSON：按下 Enter 后约 0.33 s 响应就显示在屏幕上。这包括传输、格式化为 960 万行、建立索引和第一帧。加载正文后内存约为 407 MB，空闲时为 69 MB。

## 性能预算

| 指标                                | 目标              | 实测（Windows，release） |
|-------------------------------------|-------------------|-----------------------------|
| 冷启动到第一帧                      | < 150 ms          | 222 ms（从 `main` 到第一帧；其中约 150 ms 是图形驱动加载） |
| 空闲内存                            | < 60 MB           | 88 MB 工作集，打开示例集合（大部分是图形驱动） |
| Release 二进制文件                  | < 100 MB          | 54.4 MB |
| 从输入到绘制的延迟                  | 1 帧（120 Hz 时 ≤ 8 ms） | 未测量 |
| 打开并滚动 100 MB 的 JSON 正文      | 不掉帧            | 发送后 306 ms 显示在屏幕上（本地服务器）；帧时间未测量 |

目前启动时间和空闲内存超出预算；计划对它们进行性能分析。这些数字来自 `scripts/measure.ps1`（参见[技术说明](technical.md#测量性能)）。

release 配置已经使用 fat LTO、单个 codegen 单元，并剥离符号（参见根目录的 `Cargo.toml`）。
