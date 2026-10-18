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
- [Variables and environments](#variables-and-environments)
- [The response](#the-response)
- [The Network tab](#the-network-tab)
- [Tabs](#tabs)
- [Organising requests](#organising-requests)
- [Projects](#projects)
- [Encrypted collections](#encrypted-collections)
- [Copy as and paste curl](#copy-as-and-paste-curl)
- [Importing from Postman](#importing-from-postman)
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
- **Request pane:** method, URL and the Params, Headers, Auth and Body tabs.
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
| JSON, Text, XML | The text as written, with syntax highlighting. |
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
and the size. **Pretty** formats JSON (fold blocks with the arrows), **Raw**
shows the body as received, **Headers** lists the response headers. Very large
bodies (tens of megabytes) open in a fast viewer that only draws the visible
lines.

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
(OAuth 1.0, AWS for PowerShell) are signed at the moment you copy.

Paste a `curl` command (for example from your browser's "Copy as cURL") into the
URL field and Apix fills in the whole request: method, URL, query, headers,
auth and body.

## Importing from Postman

**File › Import from Postman…**, or drop a Postman export onto the window.
Collections (v2.0 and v2.1) and environments are supported. Folders, requests,
bodies, auth and variables are carried over; anything Apix cannot take over yet
is listed in a report. Pre-request and test scripts are kept as `.js` files next
to their requests, and environment secrets go to the gitignored `.local.toml`.

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
| Alt+1…7 | Switch the method |
| Ctrl+W | Close the tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |

On macOS, use ⌘ instead of Ctrl (Ctrl+Tab stays Ctrl).
