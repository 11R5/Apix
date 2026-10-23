# Apix user guide

Apix is a desktop API client: you build HTTP requests, send them, and read the
responses. Requests are saved as small text files in a folder, so a collection
can live in git next to your code. This guide walks through the app; the
[workflows](workflows.md) cover common tasks step by step, and the `apix`
command line runs the same requests in a terminal or CI.

![The Apix window](images/main.png)

## Contents

- [Opening a collection](#opening-a-collection)
- [The window](#the-window)
- [Sending a request](#sending-a-request)
- [Params and headers](#params-and-headers)
- [Body](#body)
- [Auth](#auth)
- [Request settings](#request-settings)
- [Cookies](#cookies)
- [Variables and environments](#variables-and-environments)
- [The response](#the-response)
- [The Network tab](#the-network-tab)
- [Tabs](#tabs)
- [Organising requests](#organising-requests)
- [Projects](#projects)
- [Encrypted collections](#encrypted-collections)
- [Copy as and paste curl](#copy-as-and-paste-curl)
- [Importing from Postman](#importing-from-postman)
- [Importing from Insomnia, OpenAPI and HAR](#importing-from-insomnia-openapi-and-har)
- [History](#history)
- [Command palette](#command-palette)
- [Settings and language](#settings-and-language)
- [Keyboard shortcuts](#keyboard-shortcuts)

## Opening a collection

A **collection** is a folder that holds an `apix.toml` file and your requests
(`.apix` files), optionally in sub-folders. Open one with **File › Open…**
(Ctrl+O), or start a new one with **File › New collection…**. If you pick a
folder that is not a collection yet, Apix offers to make it one.

Recently opened collections are under **File › Open recent**. Starting the app
with a folder (`apix-desktop path/to/collection`) opens it directly.

## The window

- **Sidebar (left):** the collection's folders and requests, read from the
  files on disk and kept up to date when they change. The picker under the
  collection name chooses the active environment.
- **Tabs (top):** every request you open gets a tab.
- **Request pane:** method, URL and the Params, Headers, Auth, Body and
  Settings tabs.
- **Response pane (right):** status, timing, size and the response body.
- **Status bar (bottom):** the collection, the file of the open request and the
  most useful shortcuts.
- **Menu bar:** File, Request, Environment, View and Help hold every command.

## Sending a request

1. Pick the method on the left of the URL bar (or press Alt+1…7 for GET, POST,
   PUT, PATCH, DELETE, HEAD, OPTIONS).
2. Type the URL. `{{variables}}` are allowed anywhere (see
   [Variables](#variables-and-environments)).
3. Press **Send** or Ctrl+Enter. Esc cancels a request that is still running.

**New request** (Ctrl+N) opens a GET request; the sidebar's **+** menu offers
every method, and POST, PUT and PATCH start with a JSON body. Save with Ctrl+S.
A dot on the tab and in the sidebar marks unsaved changes.

## Params and headers

**Params** are added to the URL's query string, **Headers** are sent as they
are. Untick a row to keep it in the file without sending it. A row without a
key is ignored. `Content-Type` is set from the body type unless you add your
own.

## Body

The Body tab offers:

| Type | What is sent |
|---|---|
| None | No body. |
| JSON, Text, XML, HTML | The text as written, with syntax highlighting (`application/json`, `text/plain`, `application/xml`, `text/html` unless a header says otherwise). |
| Form | `application/x-www-form-urlencoded` key–value rows. |
| Multipart | `multipart/form-data`. Each field is **Text** or **File**; the folder button picks one or more files. |
| Binary | The bytes of one file, as `application/octet-stream` unless a header says otherwise. Apix shows the file size, or "File not found". |
| GraphQL | A query editor and a JSON variables editor; sent as `{"query", "variables"}` JSON. |

![Multipart body](images/body.png)

File paths inside the collection are saved relative to the request file, so the
collection keeps working on another machine.

## Auth

The Auth tab sets how the request proves who it is:

| Type | Notes |
|---|---|
| Inherit from parent | The default: use the auth of the nearest folder, collection or project that sets one. The tab tells you what that is and where it comes from. |
| No auth | Send nothing, even if a parent sets auth. |
| Bearer token | `Authorization: Bearer <token>`. |
| Basic auth | Username and password. |
| API key | A header or query parameter of your choice. |
| Digest auth | Apix answers the server's Digest challenge (MD5 or SHA-256). |
| OAuth 1.0 | HMAC-SHA1, HMAC-SHA256 or PLAINTEXT; in the header or the query string; a new nonce and timestamp on every send. |
| OAuth 2.0 | Client credentials, password, or authorization code with PKCE. Apix gets the token itself and sends it as a bearer token. |
| AWS Signature | AWS Signature Version 4 with access key, secret key, optional session token, region and service. |

![OAuth 2.0 in the Auth tab](images/auth.png)

**OAuth 2.0 tokens.** The first send fetches a token; later sends reuse it and
renew it with the refresh token when it expires. **Get new token** fetches one
now, **Forget** drops it. Tokens are kept in memory only and never written to
disk. For the authorization code grant, Apix opens the sign-in page in your
browser and receives the answer on `http://127.0.0.1` (a free port, or the
**Redirect URL** you registered for the client).

**Shared auth.** Right-click a folder or collection and choose **Auth…** to set
auth once for everything inside; **Environment › Project auth…** does the same
for a whole project. Requests whose auth is *Inherit from parent* use it.

## Request settings

The Settings tab controls how the request travels:

| Setting | Default |
|---|---|
| Timeout (seconds) | none — a request runs until it finishes or you cancel it; `0` also means no timeout |
| Follow redirects | on |
| Max redirects | 10 |
| Verify TLS certificate | on — turn it off only for test servers you trust |
| Proxy | from the environment (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`); *Off* ignores those; a URL names one explicitly |
| Client certificate | none — a PEM file, with the key in the same file or a separate one |
| Send cookies | on |

Every field left empty (or on *Inherit*) falls back to the nearest folder,
collection or project that sets it, and finally to the defaults above — the
same chain as auth. Right-click a folder or collection and choose **Request
settings…** to set them once for everything inside; **Environment › Project
request settings…** does the same for a whole project. The line under the form
shows what the request ends up using. `{{variables}}` work in the proxy and
certificate fields, and relative certificate paths are resolved against the
file that sets them.

In a request file (and in `folder.toml`, `apix.toml` and `apix-project.toml`)
the settings are a `[settings]` table:

```toml
[settings]
timeout_ms = 5000
tls_verify = false
proxy = ""          # "" = no proxy, even when the environment sets one
client_cert = "certs/client.pem"
```

**Copy as** writes the matching curl flags (`-m`, `-L --max-redirs`, `-k`,
`-x`, `--cert`, `--key`) and pasting a curl command with those flags fills the
tab. `apix send` has `--timeout`, `--insecure` and `--proxy`, which override
the file. Postman's per-request options (`protocolProfileBehavior`) are
imported.

## Cookies

Apix keeps one cookie jar: `Set-Cookie` answers are stored automatically and
sent with later requests to the same site, including across restarts
(`cookies.json` next to the app settings; session cookies die with the app).
**Request › Cookies…** lists them, deletes single cookies, or empties the jar.
The Settings tab's **Send cookies** turns the jar off for a request, a folder,
a collection or a project. The CLI starts with an empty jar on every run.

## Variables and environments

Write `{{name}}` in a URL, header, body or auth field and Apix fills in the
value when sending. In the URL bar each variable is a coloured chip: hover it
to see its value and where it comes from; a red chip is not defined anywhere.

Values come in layers, the later winning:

1. **Project** variables (shared by every collection of a project)
2. **Collection** variables (`apix.toml`)
3. The active **environment** (`environments/<name>.toml`)

Open **Environment › Variables and environments…** (Ctrl+E) to edit them. Tick
**Secret** for tokens and passwords: secret values are stored in
`environments/<name>.local.toml`, which Apix adds to `.gitignore`, so they are
never committed.

![The Variables dialog](images/variables.png)

**Dynamic variables** get a fresh value on every use: `{{$guid}}`,
`{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomEmail}}` and
more; the **Dynamic variables** page of the Variables dialog lists them all with a sample value.

## The response

The response pane shows the status, the time to the first byte and in total,
and the size. **Pretty** formats JSON (fold blocks with the arrows) and
indents and colours XML and HTML; **Raw** shows the body as received,
**Headers** lists the response headers and **Hex** shows the bytes. Very large
bodies (tens of megabytes) open in a fast viewer that only draws the visible
lines.

Apix tells the body's kind from its `Content-Type`, or from the bytes when the
type says nothing useful. Images (PNG, JPEG, GIF, WebP, BMP, SVG) are shown
in **Pretty**; other binary content (a PDF, a zip…) gets a short summary
instead of unreadable text. HTML that leaves out optional closing tags is shown
as it came rather than reformatted.

The **↓** button at the end of the status line saves the body, byte for byte,
to a file. The suggested name comes from the server's `Content-Disposition`
or from the URL, with an extension that fits the content.

## The Network tab

The **Network** tab explains where the time went and what the connection was:

- a timing bar split into DNS, connecting, waiting and downloading
- the DNS answers, the server and local address, the network interface (with a
  hint when a VPN is in use), the gateway, DNS servers and proxy
- every redirect
- the server certificate: issuer, validity and days left

![The Network tab](images/network.png)

## Tabs

Each tab keeps its unsaved edits, its last response, and even a request that is
still running. Ctrl+Tab and Ctrl+Shift+Tab move between tabs, Ctrl+W or a
middle click closes one; Apix asks before closing unsaved changes.

## Organising requests

Right-click in the sidebar to create, rename, duplicate or delete requests and
folders; deleted files go to the system trash. Drag a request or folder onto
another folder to move it.

## Projects

A **project** groups several collections that belong together and shares
environments and auth between them. Create one with **File › New project…**.
In a project, **Environment › Group the sidebar by** can show the sidebar as
Project › Environment › Collections.

## Encrypted collections

An encrypted collection keeps every request, environment and even file name
sealed on disk (XChaCha20-Poly1305 with an Argon2id password key). Git only sees
which encrypted blob changed.

- **File › Encrypt this collection…** or right-click a collection › **Encrypt…**
  encrypts an existing collection in place.
- **File › New encrypted collection…** creates one inside a project.
- You get a **recovery key** once. Keep it somewhere safe: it opens the
  collection if the password is lost.
- Opening the collection asks for the password. It locks again when you choose
  **File › Lock encrypted collections**, quit Apix, or leave it idle for the
  time set in Settings (15 minutes by default).

## Copy as and paste curl

The copy button next to **Send** (or **Request › Copy as**) copies the request
as `curl` for bash, `curl.exe` for the Windows command prompt, or a PowerShell
`Invoke-RestMethod` call, with variables filled in. Requests signed per send
(OAuth 1.0, AWS for PowerShell and code) are signed at the moment you copy.

The same menu copies the request as a program: JavaScript `fetch`, Node.js
axios, Python requests, Go `net/http`, Rust reqwest, C# `HttpClient`, Java
`HttpClient`, PHP cURL, Swift `URLSession` or Kotlin OkHttp. Each one sends
what Apix would send — URL, headers, auth and every body type — and the
request settings the language can express (timeout, redirects, TLS check,
proxy, client certificate). A setting a language has no simple switch for is
listed in a `Not in this snippet` comment at the top instead of being dropped
silently. In the sidebar, right-click a request and use **Copy as code**.

The CLI writes the same: `apix code request.apix --as python` (`apix shell` is
the same command; `--as` takes `bash`, `cmd`, `powershell`, `fetch`, `axios`,
`python`, `go`, `rust`, `csharp`, `java`, `php`, `swift` or `kotlin`).

Paste a `curl` command (for example from your browser's "Copy as cURL") into the
URL field and Apix fills in the whole request: method, URL, query, headers,
auth and body.

## Importing from Postman

**File › Import…**, or drop a Postman export onto the window.
Collections (v2.0 and v2.1) and environments are supported. Folders, requests,
bodies, auth and variables are carried over; anything Apix cannot take over yet
is listed in a report. Pre-request and test scripts are kept as `.js` files next
to their requests, and environment secrets go to the gitignored `.local.toml`.

**Many files at once.** Open or drop a folder that holds Postman exports — or
the zip that Postman's *Export data* produces — and Apix recognises the files
by their content, not their names, and offers to import everything as one
project: a collection per Postman collection, the environments shared by all
of them, and globals as project variables (secret globals are left out, since
project variables are committed). Two environments with the same name are kept
as `name` and `name-2`.

**Export to Postman.** Right-click a collection and choose **Export to
Postman…** (or **File › Export to Postman…**) to write a Postman v2.1
collection file and one environment file per environment, so teammates on
Postman can keep working with it. Folders, auth, bodies, scripts, variables and
the redirect, TLS and cookie settings carry over. Postman has no per-request
timeout, proxy or client certificate, and secret environment values are
exported with their names only; the notification lists what was left out.

On the command line: `apix import <file, folder or zip>` and
`apix export <collection> --out <folder>`.

## Importing from Insomnia, OpenAPI and HAR

**File › Import…** (or dropping the file on the window) also reads these; the
format is recognised by content, and a folder or zip may mix them:

| Format | What you get |
|---|---|
| Insomnia v4 (JSON export) | A collection per workspace, folders with their auth, the base environment as collection variables and every sub-environment as an environment. `{{ _.name }}` becomes `{{name}}`. |
| OpenAPI 3 / Swagger 2 (JSON or YAML) | A request per operation, a folder per tag. The server becomes the `baseUrl` variable (several servers: an environment each), path parameters become variables with their example values, required query and header parameters are switched on, bodies come from examples or are built from the schema, and security schemes become auth with empty credential variables (`bearerToken`, `apiKey`, …) to fill in an environment. |
| HAR (browser DevTools "Save all as HAR") | API calls only, a folder per host. Scripts, styles, images and fonts, `data:` URLs and repeats are left out, and so are `Cookie` headers: they carry the browser's session. |

One collection from one file is imported as a collection; several become a
project. The report lists everything that could not be carried over.

## History

Every request you send is kept with its response. **View › History…**
(Ctrl+H) lists them, newest first, with method, name, URL, status and when it
was sent; type to search by any of these. Pick an entry and the request comes
back in the tab it was sent from, as unsaved edits, together with the response
it got. Press Ctrl+Enter to send it again, Ctrl+S to keep that version. If the
request file is gone, the entry opens in the editor without a tab.

History stays on this computer (the `history` folder next to the settings),
never inside a collection. Requests are stored as written, with `{{variables}}`
unresolved, so environment secrets are not copied into it. The last 300
requests are kept, response bodies up to 256 KB each. Requests from encrypted
collections are never written to the history. **Clear history** in the window
empties it; **File › Settings… › History** turns it off.

## Command palette

Ctrl+K (or Ctrl+P) opens the command palette: type a few letters of a request
name or a command and press Enter. Commands can be found by their English names
in every language.

![The command palette](images/palette.png)

## Settings and language

**File › Settings…** (Ctrl+,):

- **Language:** English, Türkçe, العربية, Deutsch, Français, Русский, 日本語,
  简体中文 or Español. The change applies at once. On first start Apix follows
  the system language. In Arabic the window mirrors: the sidebar is on the
  right and menus, tabs and forms run from the right, while URLs, code and
  key–value tables stay left to right.
- **Theme:** Obsidian or Cyber glass; applies the next time Apix starts.
- **Lock encrypted collections:** after 5, 15 or 30 minutes, 1 hour, or never.
- **History:** on or off (see [History](#history)).

![Settings](images/settings.png)

Settings are saved in `settings.toml` in the Apix folder of your system's
configuration directory (`%APPDATA%\Apix` on Windows,
`~/Library/Application Support/Apix` on macOS, `~/.config/Apix` on Linux).

## Keyboard shortcuts

| Keys | Action |
|---|---|
| Ctrl+Enter | Send |
| Esc | Cancel the request |
| Ctrl+S | Save |
| Ctrl+N | New GET request |
| Ctrl+O | Open a folder, project or collection |
| Ctrl+K or Ctrl+P | Command palette |
| Ctrl+E | Variables and environments |
| Ctrl+, | Settings |
| Ctrl+H | History |
| Alt+1…7 | Switch the method |
| Ctrl+W | Close the tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |

On macOS, use ⌘ instead of Ctrl (Ctrl+Tab stays Ctrl).
