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
- [Sign in with OAuth 2.0](#sign-in-with-oauth-20)
- [Call an AWS API](#call-an-aws-api)
- [Upload a file, send GraphQL](#upload-a-file-send-graphql)
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
2. In Apix, **File › Import from Postman…** or drop the files onto the window.
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

## Run a collection in CI

`apix run` sends every request in a folder and exits with a non-zero code if any
fails, so it works as a CI step:

```sh
apix run my-api --env staging --var token="$API_TOKEN"
```

A GitHub Actions step could look like this:

```yaml
- name: API smoke test
  run: apix run api-tests --env staging
  env:
    APIX_PASSWORD: ${{ secrets.APIX_PASSWORD }}   # only for an encrypted collection
```

`apix send <file>` sends one request and prints the response; add `-i` for the
headers and `--network` for the connection details.

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
2. The timing bar shows whether the time goes into DNS, connecting (TCP and
   TLS), waiting for the server, or downloading.
3. Check the remote address and the local interface: a VPN or proxy shows up
   there.
4. Redirects are listed in order; a certificate close to expiry shows its days
   left.

The same report is available in a terminal:

```sh
apix send my-api/health.apix --network
```
