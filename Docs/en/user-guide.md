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
- [Scripts and tests](#scripts-and-tests)
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

While an environment whose name contains “prod” is active, Apix asks before
sending DELETE, PUT or PATCH and shows the full URL it would hit. **Settings ›
Confirm in production** turns that off.

**New request** (Ctrl+N) opens a GET request; the sidebar's **+** menu offers
every method, and POST, PUT and PATCH start with a JSON body. Save with Ctrl+S.
A dot on the tab and in the sidebar marks unsaved changes.

## Params and headers

**Params** are added to the URL's query string, **Headers** are sent as they
are. Untick a row to keep it in the file without sending it. A row without a
key is ignored. `Content-Type` is set from the body type unless you add your
own. `{{variables}}` in keys and values show as chips, as in the URL bar; hover
one to see its value.

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

To keep them off the disk too, choose **Settings › Environment secrets ›
System keychain**. The values then go to the system's credential store
(Credential Manager on Windows, the Keychain on macOS, the Secret Service on
Linux) and `<name>.local.toml` only keeps the id of that entry, so renaming the
environment or moving the folder keeps working. The `apix` command line reads
them the same way. Changing the setting moves the secrets of the open collection
or project at once; others move the next time you save them. Secrets of an
encrypted collection always stay inside it.

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

## Scripts and tests

The **Scripts** tab holds two JavaScript scripts for the request:

- **Pre-request** runs before the request is sent. It can change the request
  (`pm.request.headers.upsert(…)`, `pm.request.url.query.add(…)`,
  `pm.request.body.raw = …`) and set variables
  (`pm.environment.set("token", …)`).
- **Tests** runs after the response arrives and checks it:

```js
pm.test("status is 200", () => pm.response.to.have.status(200));
pm.test("has users", () => {
  const body = pm.response.json();
  pm.expect(body.users).to.be.an("array").that.is.not.empty;
});
```

The API is Postman's `pm.*` (also available as `apix.*`), so scripts imported
from Postman run as they are: `pm.environment`, `pm.collectionVariables`,
`pm.globals` (the project's variables), `pm.variables`, `pm.request`,
`pm.response`, `pm.test`, `pm.expect` with Chai-style assertions,
`pm.info`, `console.log`, and the older `tests["…"] = …` and
`postman.setEnvironmentVariable(…)`. `pm.sendRequest` and `require(…)` come
later.

Scripts are files next to what they belong to: `<request>.prerequest.js` and
`<request>.test.js` next to the request, `_folder.prerequest.js` /
`_folder.test.js` in a folder, `_collection.prerequest.js` /
`_collection.test.js` in the collection root. They run from the outside in:
collection, folders, then the request. Ctrl+S saves the request's scripts with
it; the editor's text is used even before you save.

After sending, the response pane gets a **Tests** tab (✓ passed, ✗ failed with
the reason) and a **Console** tab with `console.*` output. If a pre-request
script throws, the request is not sent and the error says which file failed.
Scripts stop after 10 seconds.

Values a script sets are not written to any file. They last while Apix is open
(switching environment drops what `pm.environment.set` set) and show up in the
variable cards as “Set by a script”. `pm.variables.set` lasts for one send.

## Tabs

Each tab keeps its unsaved edits, its last response, and even a request that is
still running. Ctrl+Tab and Ctrl+Shift+Tab move between tabs, Ctrl+W or a
middle click closes one; Apix asks before closing unsaved changes. The tabs
that were open come back the next time you open the same collection or
project (tabs of encrypted collections are not remembered, as their file names
are secret too).

## Organising requests

Right-click in the sidebar to create, rename, duplicate or delete requests and
folders; deleted files go to the system trash. Drag a request or folder onto
the middle of another folder to move it there. Drop it on the top or bottom
edge of a row instead to put it before or after that row; a line shows where
it will land. The order is saved in `apix.toml` (for the collection's top
level) and `folder.toml` (for a folder) as an `order` list, so it is shared
through git; items it does not list follow by name. Postman imports keep
Postman's order, and exports to Postman follow yours.

## Projects

A **project** groups several collections that belong together and shares
environments and auth between them. Create one with **File › New project…**.
In a project, **Environment › Group the sidebar by** can show the sidebar as
Project › Environment › Collections.

- Drag a collection's header above or below another one to change their order.
- Right-click a collection › **Colour** gives it a colour; a dot shows it in the
  sidebar and on the tabs of its requests. Colours are saved in
  `apix-project.toml`.
- **File › Move into a project…** (with a single collection open) moves that
  collection's folder into a project you pick. With a project open, dropping a
  collection's folder from the file manager onto the window offers the same.

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
- Tick **Remember on this computer** in the unlock prompt to keep the password
  in the system's credential store (Credential Manager on Windows, the Keychain
  on macOS, the Secret Service on Linux). The collection then opens without
  asking while you are signed in, and the `apix` command line uses the saved
  password too. **File › Forget saved password** (or right-click the
  collection) removes it. If the password was changed elsewhere, the saved one
  is dropped and Apix asks again.

## Copy as and paste curl

The copy button next to **Send** (or **Request › Copy as**) copies the request
as `curl` for bash or fish, `curl.exe` for the Windows command prompt, a
PowerShell `Invoke-RestMethod` call, an HTTPie `http` command (HTTPie 3.0 or
later) or a GNU `wget` command, with variables filled in. wget cannot send
multipart forms; the copied command says so. Requests signed per send
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
- **Environment secrets:** in a `.local.toml` file or in the system keychain
  (see [Variables and environments](#variables-and-environments)).
- **Confirm in production:** ask before DELETE, PUT or PATCH while a “prod”
  environment is active (on by default).
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
