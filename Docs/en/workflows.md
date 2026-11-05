# Workflows

Step-by-step recipes for common tasks. The [user guide](user-guide.md) describes
each part of the app; the [technical notes](technical.md) cover building and
measuring Apix.

- [Send and save your first request](#send-and-save-your-first-request)
- [Keep a collection in git](#keep-a-collection-in-git)
- [Work with dev, staging and production](#work-with-dev-staging-and-production)
- [Share a collection that holds secrets](#share-a-collection-that-holds-secrets)
- [Move from Postman](#move-from-postman)
- [Run a collection in CI](#run-a-collection-in-ci)
- [Watch an API with monitors](#watch-an-api-with-monitors)
- [Sign in with OAuth 2.0](#sign-in-with-oauth-20)
- [Call an AWS API](#call-an-aws-api)
- [Upload a file, send GraphQL](#upload-a-file-send-graphql)
- [Call a gRPC service](#call-a-grpc-service)
- [Reproduce a request from the browser](#reproduce-a-request-from-the-browser)
- [Find out why a request is slow or failing](#find-out-why-a-request-is-slow-or-failing)

## Send and save your first request

1. **File › New collection…**, pick an empty folder and give it a name.
2. Ctrl+N opens a new GET request. Type `https://jsonplaceholder.typicode.com/users`.
3. Ctrl+Enter sends it; the response appears on the right.
4. Ctrl+S saves it as a new file in the collection folder. Right-click it in the
   sidebar › **Rename** to give it a name such as "List users".

The file is plain TOML you can read and edit:

```toml
name = "List users"
url = "https://jsonplaceholder.typicode.com/users"
```

## Keep a collection in git

A collection is a folder:

```
my-api/
├── apix.toml                collection name, variables, shared auth
├── environments/
│   ├── staging.toml         shared values
│   └── staging.local.toml   secret values (gitignored)
├── users/
│   ├── folder.toml          optional: auth for the requests in this folder
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

Commit everything except `*.local.toml`; Apix adds that pattern to
`.gitignore` itself the first time it writes a secret. Each request is its own
small file, so reviews and merges stay readable.

## Work with dev, staging and production

1. Open **Environment › Variables and environments…** (Ctrl+E).
2. Add environments named `dev`, `staging` and `prod`, each with a `base_url`.
3. Use `{{base_url}}/users` in your requests.
4. Pick the environment in the sidebar, under the collection name.

Values that should not be committed (tokens, passwords) get the **Secret** tick.
Each teammate keeps their own `*.local.toml`. A variable defined at collection
level is a default that environments can override.

## Share a collection that holds secrets

When the whole collection is sensitive (internal URLs, sample customer data,
long-lived keys), encrypt it:

1. **File › Encrypt this collection…** and choose a password.
2. Store the **recovery key** you are shown in your password manager.
3. Commit the folder. On disk it is a set of encrypted objects; even file names
   are hidden.

Teammates open the folder in Apix and type the password. In a terminal or CI:

```sh
APIX_PASSWORD='…' apix run my-api --env staging
```

`apix decrypt my-api --out my-api-plain` writes a plain copy, with
`--recovery` to use the recovery key instead of the password.

## Move from Postman

1. In Postman, export the collection (Collection v2.1) and each environment.
2. In Apix, **File › Import…** or drop the files onto the window.
   Or in a terminal:

   ```sh
   apix import "My API.postman_collection.json"            # creates ./my-api
   apix import "Staging.postman_environment.json" --into my-api
   ```

3. Read the import report: anything that could not be carried over is listed
   there (for example NTLM auth or saved example responses).
4. Pre-request and test scripts are kept as `.js` files next to their requests;
   Apix does not run them yet.
5. For OAuth 2.0 with the authorization code grant, register a loopback
   redirect URL such as `http://127.0.0.1:8765/callback` with your identity
   provider and set it in the Auth tab.

Moving a whole workspace? Use Postman's **Settings › Data › Export data**,
then open or drop the folder (or the zip) in Apix: every collection,
environment and global comes over as one project
(`apix import postman-export.zip` does the same). Teammates who stay on
Postman get your collection back with **Export to Postman…**.

## Run a collection in CI

`apix run` sends every request of a collection in sidebar order and exits
with a non-zero code if any fails, so it works as a CI step:

```sh
apix run my-api --env staging --var token="$API_TOKEN"
```

Requests with tests (scripts or Checks) pass or fail by their tests; the
others by their status code. The output lists each test under its request:

```text
create.apix                              201 Created  466.7 ms · 42 B
  │ new id 101
  ✓ created
get.apix                                 200 OK  174.7 ms · 292 B
  ✓ status 200
  ✗ has the right id expected 1 to equal 2

1 passed, 1 failed (1 failed tests)
```

What a script sets (`pm.environment.set`, `pm.variables.set`) carries on to
the next requests of the run, and `pm.execution.setNextRequest("name")`
jumps to another request (`null` ends the iteration).

Options:

| Option | What it does |
|---|---|
| `apix run my-api/users` | Only the requests in a folder (or one request file) |
| `-d, --data users.csv` | A data-driven run: each CSV or JSON row is one iteration, its columns are variables (`{{email}}`, `pm.iterationData.get("email")`) |
| `-n, --iterations 5` | Go through the requests 5 times (default: once per data row, else once) |
| `--delay 200` | Wait 200 ms between requests |
| `--bail` | Stop at the first request that fails |
| `-p, --parallel 4` | Run 4 iterations at once, like 4 users; each sends its requests in order and keeps what its scripts set to itself |
| `--junit report.xml` | A JUnit XML report, which CI services show as a test list |
| `--json report.json` / `--html report.html` | Everything as JSON, or a page that opens in any browser |
| `--markdown report.md` | A summary for a pull request comment or a CI job summary (see below) |
| `--changed` / `--changed=origin/main` | Only the requests a change touches (see below) |
| `--timeout 10`, `-k`, `--proxy URL` | Override the requests' transport settings |
| `--env-file .env` | Variables from a dotenv file (see below) |

A data file looks like this; values from outside the collection (`--var` and
the ones below) win over its columns:

```csv
id,name
1,Leanne Graham
2,Ervin Howell
```

### Secrets in CI

Secrets stay out of git (`.local.toml` files are ignored, the keychain is on
your computer), so a pipeline hands them over in one of three ways, each
winning over the one before:

1. `--env-file .env`: a dotenv file of `NAME=value` lines (`#` comments,
   quotes, `export` allowed).
2. `APIX_VAR_<NAME>` environment variables: `APIX_VAR_TOKEN` fills
   `{{token}}`. CI secrets are usually written in capitals, so a name matches
   a variable the collection defines or its requests use whatever its case.
3. `--var name=value`.

All three win over the environment's values (and over a data file's
columns). `apix` prints which names it took from where on stderr, never the
values.

```sh
APIX_VAR_TOKEN="$API_TOKEN" apix run api --env staging
```

### GitHub Actions

The repository is also a GitHub Action. It builds `apix` the first time
(cached afterwards), runs the collection and writes JUnit and HTML reports:

```yaml
- uses: 11R5/Apix@main          # better: a tag or commit
  with:
    path: api
    environment: staging        # also: data, iterations, delay, bail, env-file, junit, html, json, markdown, changed, diff, comment, args
  env:
    APIX_VAR_TOKEN: ${{ secrets.API_TOKEN }}
```

[examples/ci/github-actions.yml](../../examples/ci/github-actions.yml) is a
complete workflow: it runs the collection once per environment (a matrix of
`test` and `staging`), shows the JUnit results on the run's page and keeps the
HTML reports. The action also adds its Markdown summary to the job's page.

## Only what changed: git-aware runs

A collection is a folder in git, so `apix` can ask git what a change
touched. `--changed` sends only those requests:

```sh
apix run api --changed                 # what is not committed yet
apix run api --changed=origin/main     # what this branch changed since it left main
```

A request runs again when its own file or its scripts changed, when a
`folder.toml` or folder script above it changed (every request in that
folder), when `apix.toml`, a collection script, an environment or the
project file changed (every request), or when it names another changed file
(a body file, a `.proto`). New files count before they are committed;
deleted requests are listed. Without a value, `--changed` compares with the
pull request's base branch on GitHub Actions (`origin/$GITHUB_BASE_REF`) and
otherwise with `HEAD`. Write the value with `=`, so it cannot be mistaken
for the folder to run.

```text
2 of 3 requests changed since origin/main
  users/create-user.apix  ← users/create-user.test.js
  users/list-users.apix
```

Like a pull request, the comparison starts where the branch left the base,
so what landed on main meanwhile does not count. That needs the history: in
a shallow clone (the default of `actions/checkout`) `apix` warns and compares
with the base itself; check out with `fetch-depth: 0`. Encrypted collections
cannot be compared, because git only sees their encrypted files.

### What changed: `apix diff`

`apix diff` shows a change the way a reviewer reads it: a request's fields
rather than lines of TOML, headers and query parameters by name, the body
and scripts line by line, environments by variable. Reformatting a file is
not a change, and a request moved or renamed without other edits is one
move.

```sh
apix diff                       # HEAD against the files as they are now
apix diff main                  # main against now
apix diff v1.2 v1.3 -C api      # between two tags, for the collection in api/
```

```text
Between main and the working tree: 1 script, 1 request

A  script  users/create-user.test.js
     + pm.test("ok", () => pm.response.to.have.status(200));
M  GET List users  users/list-users.apix
     query._limit: 3 → 5
     headers.Accept: (none) → application/json
```

`--markdown` prints it for a pull request, `--json` for tools, and
`--exit-code` exits with 1 when something changed.

### A summary on the pull request

`--markdown report.md` writes the run as GitHub Markdown: the totals, one
row per request and, open below, what failed and why. Its first line is a
hidden marker with the collection's name, so a CI step can find the comment
it wrote last time and edit it instead of adding another.

The action does all of this. On a pull request it sends only what the pull
request touches, adds `apix diff` under the summary, and keeps one comment on
the pull request up to date:

```yaml
on: pull_request
permissions:
  contents: read
  pull-requests: write          # for the comment
jobs:
  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0        # history, to find where the branch started
      - uses: 11R5/Apix@main
        with:
          path: api
          changed: "true"       # or a commit, branch or tag
          diff: "true"
          comment: "true"
```

On a push, `changed: "true"` compares with the commit before the push. The
summary is also written to `apix-report.md` (input `markdown`) and shown on
the job's page.

In the desktop app the status bar shows the branch and how many files
changed since the last commit, and the sidebar marks changed requests with
**M** (changed) or **A** (new); see the [user guide](user-guide.md#git).

### Docker

For other CI systems there is a container image with just the `apix` command
(published to `ghcr.io/11r5/apix` when a version is tagged; build it yourself
with `docker build -t apix .`):

```sh
docker run --rm -v "$PWD:/work" -e APIX_VAR_TOKEN ghcr.io/11r5/apix run api --env staging --junit reports/apix.xml
```

[examples/ci/gitlab-ci.yml](../../examples/ci/gitlab-ci.yml) and
[examples/ci/azure-pipelines.yml](../../examples/ci/azure-pipelines.yml) do
the same in GitLab CI (with the image) and Azure Pipelines (building `apix`
once and caching it); both publish the JUnit results.

The desktop app's **Runner** (Ctrl+Shift+R) does the same runs with a window;
see the [user guide](user-guide.md#runner).

`apix send <file>` sends one request and prints the response; add `-i` for the
headers and `--network` for the connection details.

## Watch an API with monitors

A monitor runs a collection (or a folder or request of it) on a schedule and
tells a webhook when it starts failing and when it passes again. Monitors are
written in the collection's `apix.toml`, by hand or from **View › Monitors…**
in the app (see the [user guide](user-guide.md#monitors)):

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "health.apix"
environment = "production"
webhook = "{{alerts_url}}"
```

`apix monitor` runs them for as long as it is left running, each on its own
schedule, and reads `apix.toml` again as it goes, so edits take effect:

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

| Option | What it does |
|---|---|
| `--once` | Run every monitor once and exit, 1 when one fails: for cron, the Task Scheduler or a CI schedule |
| `--list` | The monitors and their last results |
| `--only NAME` | Only this monitor (repeatable); with `--once` it runs even when switched off |
| `--var`, `--env-file` | Variables over the monitors' environments, like `apix run` (`APIX_VAR_*` are read too) |
| `--log-dir DIR` | Keep the log elsewhere (in CI, a cached folder) |
| `--timeout`, `-k`, `--proxy`, `--cacert` | Transport settings, as for `apix run` |

The log keeps each monitor's last 500 results in `<config>/Apix/monitors`;
the desktop app reads the same files, so it shows what `apix monitor` found
on the same computer. A webhook is told only when a monitor's state turns
(failing after passing, or on its first run; passing after failing), with a
JSON body Slack, Discord, Mattermost and Google Chat show as a message.

On a server, start `apix monitor api` as a service, or put
`apix monitor api --once` in cron (`*/5 * * * *`). On GitHub, a scheduled
workflow does it:
[examples/ci/monitors.yml](../../examples/ci/monitors.yml) runs the monitors
every 15 minutes through the action (`monitor: "true"`) and caches the log so
recoveries are noticed.

## Sign in with OAuth 2.0

**Client credentials** (service to service):

1. Auth tab › **OAuth 2.0**, grant type **Client credentials**.
2. Fill in **Token URL**, **Client ID**, **Client secret** and **Scope**. Use
   variables (`{{client_secret}}`, marked secret) instead of literal secrets.
3. Send. Apix fetches a token, then reuses and renews it.

**Authorization code with PKCE** (a user signs in):

1. Register `http://127.0.0.1:<port>/callback` as a redirect URL for your client
   (or leave **Redirect URL** empty if the provider accepts any loopback port).
2. Grant type **Authorization code**, fill in **Auth URL**, **Token URL**,
   **Client ID** and **Scope**; keep **Use PKCE** ticked.
3. Press **Get new token** or send. Your browser opens the sign-in page; after
   you sign in, the token appears in Apix.

Set OAuth 2.0 once on the collection (right-click › **Auth…**) and every request
that inherits auth uses the same token. Tokens stay in memory; restarting Apix
signs in again.

## Call an AWS API

1. Auth tab › **AWS Signature**.
2. **Access key** and **Secret key** as variables (`{{aws_access_key}}`,
   `{{aws_secret_key}}` marked secret), **Session token** for temporary
   credentials, **Region** (for example `eu-central-1`) and **Service** (for
   example `execute-api`, `s3`, `lambda`).
3. Send. Every request is signed with Signature Version 4 just before it goes out.

**Copy as › cURL** writes `curl --aws-sigv4`, so curl signs it again when you
run it; the PowerShell copy contains a signature made when you copied, valid for
a few minutes.

## Upload a file, send GraphQL

**Multipart:** Body › **Multipart**, add a field, switch it to **File** and pick
the file. Text fields stay **Text**. Keep test files inside the collection: the
path is saved relative to the request, so it works on every machine.

**Binary:** Body › **Binary** sends one file as the whole body.

**GraphQL:** Body › **GraphQL**, write the query, put variables as JSON below.
Apix sends `{"query": …, "variables": …}` with `Content-Type: application/json`.
Set the auth first, then press **Fetch schema**: the query editor suggests
fields, arguments and enum values from then on, and hovering shows their
docs. The schema is kept per URL, so the next time you open the request it is
already there. To check a schema change in a pull request, save
`apix schema api/graphql.apix > schema.graphql` in the repository and let
the diff show what changed.

## Call a gRPC service

1. **Request › New request**, then type the address with `grpc://` (or
   `grpcs://` behind TLS), for example `grpc://localhost:50051`.
2. In the **Message** tab press **Load methods**. With server reflection
   turned off, add the `.proto` files first (**Add .proto**); keep them in the
   collection so the paths work on every machine.
3. Pick the method, press **Example message**, fill in the values.
4. Set the token in **Auth** (Bearer) or add metadata in **Headers**, then
   **Send**.

In a pipeline, the same file works with the command line:

```sh
apix send api/say-hello.apix --env staging   # exit code 1 unless the status is OK
apix schema api/say-hello.apix > greeter.txt   # what the service offers, to diff in reviews
```

## Reproduce a request from the browser

1. In the browser's developer tools, right-click a request › **Copy as cURL**
   (bash or cmd, both work).
2. In Apix, click the URL field and paste. Method, URL, query, headers, cookies,
   auth and body are filled in.
3. Send, change, save.

The other way round, the copy button next to **Send** gives you the request as
`curl`, `curl.exe` or PowerShell to paste into a terminal or a bug report.

## Find out why a request is slow or failing

1. Send the request and open the response's **Network** tab.
2. The timing bar shows whether the time goes into DNS, opening the TCP
   connection, the TLS handshake, waiting for the server, or downloading. A
   slow TCP connection points at the network; a slow handshake (compared with
   TCP, which takes one round trip) at the server or TLS 1.2's extra round trip.
3. Check the remote address and the local interface: a VPN or proxy shows up
   there.
4. Redirects are listed in order. Under Encryption: the TLS version, cipher
   suite and whether HTTP/2 was agreed; the certificate chain shows each
   certificate's issuer and days left, so a missing intermediate or one close
   to expiry stands out. A compressed body shows how much it saved.

The same report is available in a terminal:

```sh
apix send my-api/health.apix --network
```
