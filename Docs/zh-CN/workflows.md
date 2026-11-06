<!-- translated from en/workflows.md sha256:96cb14d5eeb691a0 -->

# 工作流程

常见任务的分步做法。[用户指南](user-guide.md)介绍应用的每个部分；[技术说明](technical.md)介绍如何构建和测量 Apix。

- [发送并保存第一个请求](#发送并保存第一个请求)
- [用 git 管理集合](#用-git-管理集合)
- [在 dev、staging 和 production 之间工作](#在-devstaging-和-production-之间工作)
- [分享包含机密的集合](#分享包含机密的集合)
- [从 Postman 迁移](#从-postman-迁移)
- [在 CI 中运行集合](#在-ci-中运行集合)
- [用监视器监控 API](#用监视器监控-api)
- [用 OAuth 2.0 登录](#用-oauth-20-登录)
- [调用 AWS API](#调用-aws-api)
- [上传文件、发送 GraphQL](#上传文件发送-graphql)
- [调用 gRPC 服务](#调用-grpc-服务)
- [重现浏览器中的请求](#重现浏览器中的请求)
- [找出请求慢或失败的原因](#找出请求慢或失败的原因)
- [让 AI 代理使用你的集合（MCP）](#让-ai-代理使用你的集合mcp)

## 发送并保存第一个请求

1. **文件 › 新建集合…**，选择一个空文件夹并起个名字。
2. Ctrl+N 打开一个新的 GET 请求。输入 `https://jsonplaceholder.typicode.com/users`。
3. Ctrl+Enter 发送；响应显示在右侧。
4. Ctrl+S 把它保存为集合文件夹中的一个新文件。在侧边栏中右键单击它 › **重命名**，起个名字，例如“List users”。

文件是普通的 TOML，可以直接阅读和编辑：

```toml
name = "List users"
url = "https://jsonplaceholder.typicode.com/users"
```

## 用 git 管理集合

集合就是一个文件夹：

```
my-api/
├── apix.toml                集合名称、变量、共享认证
├── environments/
│   ├── staging.toml         共享的值
│   └── staging.local.toml   机密值（被 gitignore 忽略）
├── users/
│   ├── folder.toml          可选：此文件夹中请求的认证
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

提交除 `*.local.toml` 以外的所有文件；Apix 第一次写入机密值时，会自己把这个模式加入 `.gitignore`。每个请求都是一个独立的小文件，因此评审和合并都很清楚。

## 在 dev、staging 和 production 之间工作

1. 打开 **环境 › 变量和环境…**（Ctrl+E）。
2. 添加名为 `dev`、`staging` 和 `prod` 的环境，每个都有一个 `base_url`。
3. 在请求中使用 `{{base_url}}/users`。
4. 在侧边栏中集合名称下方选择环境。

不应提交的值（令牌、密码）勾选 **机密**。每位团队成员保留自己的 `*.local.toml`。在集合级别定义的变量是默认值，环境可以覆盖它。

## 分享包含机密的集合

如果整个集合都是敏感的（内部 URL、客户样本数据、长期有效的密钥），就加密它：

1. **文件 › 加密此集合…**，然后选择一个密码。
2. 把显示给你的**恢复密钥**保存到密码管理器中。
3. 提交文件夹。在磁盘上它是一组加密对象；连文件名都被隐藏。

团队成员在 Apix 中打开该文件夹并输入密码。在终端或 CI 中：

```sh
APIX_PASSWORD='…' apix run my-api --env staging
```

`apix decrypt my-api --out my-api-plain` 写出一份明文副本；加上 `--recovery` 则用恢复密钥代替密码。

## 从 Postman 迁移

1. 在 Postman 中导出集合（Collection v2.1）和每个环境。
2. 在 Apix 中选择 **文件 › 导入…**，或把文件拖到窗口上。也可以在终端中：

   ```sh
   apix import "My API.postman_collection.json"            # 创建 ./my-api
   apix import "Staging.postman_environment.json" --into my-api
   ```

3. 阅读导入报告：无法迁移过来的内容都会列在那里（例如 NTLM 认证或保存的示例响应）。
4. 预请求脚本和测试脚本作为 `.js` 文件保存在各自请求的旁边；Apix 目前还不运行它们。
5. 对于使用授权码模式的 OAuth 2.0，在身份提供方处注册一个环回重定向 URL，例如 `http://127.0.0.1:8765/callback`，并在“认证”标签页中设置它。

要迁移整个工作区？使用 Postman 的 **Settings › Data › Export data**，然后在 Apix 中打开或拖入该文件夹（或 zip）：所有集合、环境和全局变量会作为一个项目迁移过来（`apix import postman-export.zip` 效果相同）。继续使用 Postman 的团队成员可以通过 **导出到 Postman…** 拿回你的集合。

## 在 CI 中运行集合

`apix run` 按侧边栏顺序发送集合中的每个请求，只要有一个失败就以非零代码退出，因此可以作为 CI 步骤使用：

```sh
apix run my-api --env staging --var token="$API_TOKEN"
```

带测试（脚本或检查）的请求按测试结果判定通过或失败；其他请求按状态码判定。输出把每个测试列在其请求下方：

```text
create.apix                              201 Created  466.7 ms · 42 B
  │ new id 101
  ✓ created
get.apix                                 200 OK  174.7 ms · 292 B
  ✓ status 200
  ✗ has the right id expected 1 to equal 2

1 passed, 1 failed (1 failed tests)
```

脚本设置的值（`pm.environment.set`、`pm.variables.set`）会传给本次运行中后续的请求，`pm.execution.setNextRequest("name")` 会跳转到另一个请求（`null` 结束本次迭代）。

选项：

| 选项 | 作用 |
|---|---|
| `apix run my-api/users` | 只运行一个文件夹中的请求（或一个请求文件） |
| `-d, --data users.csv` | 数据驱动的运行：CSV 或 JSON 的每一行是一次迭代，各列是变量（`{{email}}`、`pm.iterationData.get("email")`） |
| `-n, --iterations 5` | 把请求运行 5 遍（默认：每个数据行一遍，没有数据时一遍） |
| `--delay 200` | 请求之间等待 200 ms |
| `--bail` | 在第一个失败的请求处停止 |
| `-p, --parallel 4` | 同时运行 4 次迭代，就像 4 个用户；每次迭代按顺序发送请求，并把脚本设置的值留给自己 |
| `--junit report.xml` | JUnit XML 报告，CI 服务会把它显示为测试列表 |
| `--json report.json` / `--html report.html` | 全部内容以 JSON 输出，或生成一个可在任何浏览器中打开的页面 |
| `--markdown report.md` | 用于拉取请求评论或 CI 任务摘要的总结（见下文） |
| `--changed` / `--changed=origin/main` | 只运行某次更改涉及的请求（见下文） |
| `--timeout 10`、`-k`、`--proxy URL` | 覆盖请求的传输设置 |
| `--env-file .env` | 来自 dotenv 文件的变量（见下文） |

数据文件是这样的；来自集合外部的值（`--var` 和下面介绍的方式）优先于它的列：

```csv
id,name
1,Leanne Graham
2,Ervin Howell
```

### CI 中的机密

机密不进入 git（`.local.toml` 文件被忽略，钥匙串在你的电脑上），因此流水线通过以下三种方式之一传入它们，后者优先于前者：

1. `--env-file .env`：由 `NAME=value` 行组成的 dotenv 文件（允许 `#` 注释、引号和 `export`）。
2. `APIX_VAR_<NAME>` 环境变量：`APIX_VAR_TOKEN` 填充 `{{token}}`。CI 机密通常用大写字母书写，因此名称与集合定义的或其请求使用的变量匹配时不区分大小写。
3. `--var name=value`。

这三种方式都优先于环境中的值（也优先于数据文件的列）。`apix` 会在 stderr 上打印它从哪里取了哪些名称，但从不打印值。

```sh
APIX_VAR_TOKEN="$API_TOKEN" apix run api --env staging
```

### GitHub Actions

本仓库同时也是一个 GitHub Action。它运行集合并写出 JUnit 和 HTML 报告。在发布标签上使用时，它下载该发布版的 `apix`（几秒钟）；在分支或提交上使用时，它第一次构建 `apix` 并缓存，而 `apix-version: latest`（或 `0.1.1`）会改为下载一个发布版：

```yaml
- uses: 11R5/Apix@v0.1.1        # 发布版：apix 是下载的，不是构建的
  with:
    path: api
    environment: staging        # 另有：data, iterations, delay, bail, env-file, junit, html, json, markdown, changed, diff, comment, args, apix-version
  env:
    APIX_VAR_TOKEN: ${{ secrets.API_TOKEN }}
```

同一个 action 以 [`11R5/apix-action`](https://github.com/11R5/apix-action) 的名称列在 GitHub Marketplace 上。它总是下载已发布的 `apix`：`uses: 11R5/apix-action@v1` 使用最新发布版，`@v0.1.1` 使用该版本，`apix-version` 优先于两者。

[examples/ci/github-actions.yml](../../examples/ci/github-actions.yml) 是一个完整的工作流：它对每个环境运行一次集合（`test` 和 `staging` 组成的矩阵），在运行页面上显示 JUnit 结果，并保留 HTML 报告。action 还会把它的 Markdown 摘要添加到任务页面上。

## 只运行改动的部分（基于 git 的运行）

集合是 git 中的一个文件夹，因此 `apix` 可以向 git 询问某次更改涉及了什么。`--changed` 只发送这些请求：

```sh
apix run api --changed                 # 尚未提交的内容
apix run api --changed=origin/main     # 此分支离开 main 以来更改的内容
```

以下情况下请求会重新运行：它自己的文件或脚本更改了；它上层的 `folder.toml` 或文件夹脚本更改了（该文件夹中的每个请求）；`apix.toml`、集合脚本、环境或项目文件更改了（每个请求）；或者它引用的另一个文件更改了（正文文件、`.proto`）。新文件在提交前就会计入；被删除的请求会被列出。不带值时，`--changed` 在 GitHub Actions 上与拉取请求的基础分支（`origin/$GITHUB_BASE_REF`）比较，其他情况下与 `HEAD` 比较。用 `=` 写出值，这样它就不会被误认为要运行的文件夹。

```text
2 of 3 requests changed since origin/main
  users/create-user.apix  ← users/create-user.test.js
  users/list-users.apix
```

和拉取请求一样，比较从分支离开基础分支的地方开始，因此这期间合入 main 的内容不计入。这需要历史记录：在浅克隆中（`actions/checkout` 的默认方式）`apix` 会发出警告，并直接与基础分支本身比较；请用 `fetch-depth: 0` 检出。加密集合无法比较，因为 git 只能看到它们的加密文件。

### 更改了什么：`apix diff`

`apix diff` 以评审者阅读的方式显示更改：显示请求的字段而不是 TOML 的行，按名称显示头和查询参数，逐行显示正文和脚本，按变量显示环境。重新格式化文件不算更改；没有其他编辑而被移动或重命名的请求算作一次移动。

```sh
apix diff                       # HEAD 与当前文件比较
apix diff main                  # main 与当前比较
apix diff v1.2 v1.3 -C api      # 两个标签之间，针对 api/ 中的集合
```

```text
Between main and the working tree: 1 script, 1 request

A  script  users/create-user.test.js
     + pm.test("ok", () => pm.response.to.have.status(200));
M  GET List users  users/list-users.apix
     query._limit: 3 → 5
     headers.Accept: (none) → application/json
```

`--markdown` 为拉取请求输出，`--json` 为工具输出，`--exit-code` 在有更改时以 1 退出。

### 拉取请求上的摘要

`--markdown report.md` 把运行结果写成 GitHub Markdown：总计、每个请求一行，以及在下方展开的失败内容和原因。第一行是一个带集合名称的隐藏标记，因此 CI 步骤可以找到它上次写的评论并编辑它，而不是再添加一条。

action 会完成以上所有工作。在拉取请求上，它只发送拉取请求涉及的内容，在摘要下添加 `apix diff`，并在拉取请求上保持一条最新的评论：

```yaml
on: pull_request
permissions:
  contents: read
  pull-requests: write          # 用于评论
jobs:
  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0        # 历史记录，用于找到分支的起点
      - uses: 11R5/Apix@main
        with:
          path: api
          changed: "true"       # 或者一个提交、分支或标签
          diff: "true"
          comment: "true"
```

在推送时，`changed: "true"` 与推送之前的提交比较。摘要还会写入 `apix-report.md`（输入 `markdown`）并显示在任务页面上。

在桌面应用中，状态栏显示分支以及自上次提交以来更改了多少文件，侧边栏用 **M**（已更改）或 **A**（新增）标记更改的请求；参见[用户指南](user-guide.md#git)。

### Docker

对于其他 CI 系统，有一个只包含 `apix` 命令的容器镜像（打版本标签时发布到 `ghcr.io/11r5/apix`；也可以用 `docker build -t apix .` 自己构建）：

```sh
docker run --rm -v "$PWD:/work" -e APIX_VAR_TOKEN ghcr.io/11r5/apix run api --env staging --junit reports/apix.xml
```

[examples/ci/gitlab-ci.yml](../../examples/ci/gitlab-ci.yml) 和 [examples/ci/azure-pipelines.yml](../../examples/ci/azure-pipelines.yml) 在 GitLab CI（使用该镜像）和 Azure Pipelines（构建一次 `apix` 并缓存）中做同样的事；两者都会发布 JUnit 结果。

桌面应用的 **Runner**（Ctrl+Shift+R）在窗口中进行同样的运行；参见[用户指南](user-guide.md#runner)。

`apix send <file>` 发送一个请求并打印响应；加 `-i` 显示头，加 `--network` 显示连接详情。

## 用监视器监控 API

监视器按计划运行一个集合（或其中的一个文件夹或请求），并在它开始失败和恢复通过时通知一个 webhook。监视器写在集合的 `apix.toml` 中，可以手写，也可以在应用中通过 **视图 › 监视器…** 创建（参见[用户指南](user-guide.md#监视器)）：

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "health.apix"
environment = "production"
webhook = "{{alerts_url}}"
```

`apix monitor` 只要一直运行就会运行这些监视器，每个按自己的计划运行，并在运行过程中重新读取 `apix.toml`，因此修改会生效：

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

| 选项 | 作用 |
|---|---|
| `--once` | 把每个监视器运行一次后退出，有一个失败时退出码为 1：用于 cron、任务计划程序或 CI 计划 |
| `--list` | 监视器及其最近的结果 |
| `--only NAME` | 只运行这个监视器（可重复）；与 `--once` 一起使用时，即使它已关闭也会运行 |
| `--var`、`--env-file` | 覆盖监视器环境的变量，与 `apix run` 相同（也会读取 `APIX_VAR_*`） |
| `--log-dir DIR` | 把日志保存在别处（在 CI 中，一个缓存的文件夹） |
| `--timeout`、`-k`、`--proxy`、`--cacert` | 传输设置，与 `apix run` 相同 |

日志在 `<config>/Apix/monitors` 中保留每个监视器最近的 500 条结果；桌面应用读取同样的文件，因此会显示同一台电脑上 `apix monitor` 发现的情况。只有当监视器的状态转变时（通过后失败，或第一次运行时失败；失败后通过）才会通知 webhook，正文是 Slack、Discord、Mattermost 和 Google Chat 都能显示为消息的 JSON。

在服务器上，把 `apix monitor api` 作为服务启动，或者把 `apix monitor api --once` 放进 cron（`*/5 * * * *`）。在 GitHub 上，由定时工作流来完成：[examples/ci/monitors.yml](../../examples/ci/monitors.yml) 通过 action（`monitor: "true"`）每 15 分钟运行一次监视器，并缓存日志，以便发现恢复。

## 用 OAuth 2.0 登录

**客户端凭据**（服务对服务）：

1. “认证”标签页 › **OAuth 2.0**，授权类型选 **客户端凭据**。
2. 填写 **令牌 URL**、**客户端 ID**、**客户端密钥** 和 **作用域**。使用变量（`{{client_secret}}`，标记为机密），而不是直接写入机密。
3. 发送。Apix 获取令牌，然后复用并续期它。

**带 PKCE 的授权码**（用户登录）：

1. 为你的客户端把 `http://127.0.0.1:<port>/callback` 注册为重定向 URL（如果提供方接受任意环回端口，也可以让 **重定向 URL** 留空）。
2. 授权类型选 **授权码**，填写 **授权 URL**、**令牌 URL**、**客户端 ID** 和 **作用域**；保持勾选 **使用 PKCE**。
3. 按 **获取新令牌** 或直接发送。浏览器会打开登录页面；登录后，令牌出现在 Apix 中。

在集合上设置一次 OAuth 2.0（右键单击 › **认证…**），每个继承认证的请求都会使用同一个令牌。令牌保存在内存中；重启 Apix 后需要重新登录。

## 调用 AWS API

1. “认证”标签页 › **AWS 签名**。
2. **访问密钥** 和 **秘密访问密钥** 使用变量（`{{aws_access_key}}`、`{{aws_secret_key}}`，标记为机密），临时凭据填写 **会话令牌**，再填写 **区域**（例如 `eu-central-1`）和 **服务**（例如 `execute-api`、`s3`、`lambda`）。
3. 发送。每个请求都会在发出前一刻用 Signature Version 4 签名。

**复制为 › cURL** 写出 `curl --aws-sigv4`，因此运行时 curl 会重新签名；PowerShell 副本包含复制时生成的签名，几分钟内有效。

## 上传文件、发送 GraphQL

**Multipart：**请求体 › **Multipart**，添加一个字段，把它切换为 **文件** 并选择文件。文本字段保持 **文本**。把测试文件放在集合内：路径相对于请求保存，因此在每台机器上都能用。

**二进制：**请求体 › **二进制** 把一个文件作为整个正文发送。

**GraphQL：**请求体 › **GraphQL**，编写查询，把变量以 JSON 形式写在下方。Apix 用 `Content-Type: application/json` 发送 `{"query": …, "variables": …}`。先设置认证，然后按 **获取架构**：从此查询编辑器会建议字段、参数和枚举值，鼠标悬停会显示它们的文档。架构按 URL 保存，因此下次打开请求时它已经在那里。要在拉取请求中检查架构的变化，把 `apix schema api/graphql.apix > schema.graphql` 保存到仓库中，让 diff 显示变化。

## 调用 gRPC 服务

1. **请求 › 新建请求**，然后输入带 `grpc://` 的地址（TLS 之后用 `grpcs://`），例如 `grpc://localhost:50051`。
2. 在 **消息** 标签页中按 **加载方法**。如果服务器关闭了反射，先添加 `.proto` 文件（**添加文件**）；把它们放在集合中，这样路径在每台机器上都能用。
3. 选择方法，按 **示例消息**，填写值。
4. 在 **认证** 中设置令牌（Bearer），或在 **请求头** 中添加元数据，然后按 **发送**。

在流水线中，同一个文件也可以用于命令行：

```sh
apix send api/say-hello.apix --env staging   # 状态不是 OK 时退出码为 1
apix schema api/say-hello.apix > greeter.txt   # 服务提供的内容，用于在评审中比较
```

## 重现浏览器中的请求

1. 在浏览器的开发者工具中，右键单击一个请求 › **Copy as cURL**（bash 或 cmd 都可以）。
2. 在 Apix 中单击 URL 字段并粘贴。方法、URL、查询、头、Cookie、认证和正文都会被填好。
3. 发送、修改、保存。

反过来，**发送** 旁边的复制按钮会把请求变成 `curl`、`curl.exe` 或 PowerShell，用于粘贴到终端或错误报告中。

## 找出请求慢或失败的原因

1. 发送请求，打开响应的 **网络** 标签页。
2. 计时条显示时间花在 DNS、建立 TCP 连接、TLS 握手、等待服务器还是下载上。TCP 连接慢说明是网络问题；握手慢（与只需一次往返的 TCP 相比）说明是服务器问题，或是 TLS 1.2 额外的一次往返。
3. 检查远程地址和本地网卡：VPN 或代理会在这里显示出来。
4. 重定向按顺序列出。在“加密”下：TLS 版本、密码套件以及是否协商了 HTTP/2；证书链显示每个证书的颁发者和剩余天数，因此缺少中间证书或即将过期的证书会很显眼。压缩的正文会显示节省了多少。

在终端中也能得到同样的报告：

```sh
apix send my-api/health.apix --network
```

## 让 AI 代理使用你的集合（MCP）

`apix mcp` 通过 Model Context Protocol 提供一个集合，因此 AI 代理（Claude、编辑器的助手、任何 MCP 客户端）可以像你在 Apix 中那样使用你的 API。它通过标准输入和输出使用 MCP；由代理启动它。使用 Claude Code：

```bash
claude mcp add my-api -- apix mcp /path/to/my-api
```

其他客户端在其配置中使用同样的命令，例如：

```json
{ "mcpServers": { "my-api": { "command": "apix", "args": ["mcp", "/path/to/my-api"] } } }
```

代理会得到四个工具：

| 工具 | 作用 |
|---|---|
| `list_requests` | 整个集合或一个文件夹中的请求（路径、方法、名称、带 `{{variables}}` 的 URL） |
| `list_environments` | 环境及其变量的名称，从不包含值 |
| `send_request` | 在某个环境中、用代理设置的变量发送一个请求，并运行其脚本和测试；返回状态、时间、头、正文（最多 64 KB）和测试结果 |
| `run_requests` | 运行一个文件夹或整个集合及其测试，并报告哪些通过了 |

只能访问该集合内的请求。`--list-only` 保留两个列出工具，去掉会发送请求的工具。机密留在原处：代理可以要求用某个令牌发送请求，但它永远看不到令牌的值（回显令牌的响应则是另一回事，所以要注意你允许代理发送什么）。对于加密集合，请在 `APIX_PASSWORD` 中提供密码。
