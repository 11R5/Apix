<!-- translated from en/workflows.md sha256:ced46a4d6c27e0a9 -->

# Workflows

Des recettes pas à pas pour les tâches courantes. Le [guide de l’utilisateur](user-guide.md)
décrit chaque partie de l’application ; les [notes techniques](technical.md)
traitent de la compilation et de la mesure d’Apix.

- [Envoyer et enregistrer votre première requête](#envoyer-et-enregistrer-votre-première-requête)
- [Garder une collection dans git](#garder-une-collection-dans-git)
- [Travailler avec dev, staging et production](#travailler-avec-dev-staging-et-production)
- [Partager une collection qui contient des secrets](#partager-une-collection-qui-contient-des-secrets)
- [Passer de Postman à Apix](#passer-de-postman-à-apix)
- [Exécuter une collection en CI](#exécuter-une-collection-en-ci)
- [Surveiller une API avec des moniteurs](#surveiller-une-api-avec-des-moniteurs)
- [Se connecter avec OAuth 2.0](#se-connecter-avec-oauth-20)
- [Appeler une API AWS](#appeler-une-api-aws)
- [Envoyer un fichier, envoyer du GraphQL](#envoyer-un-fichier-envoyer-du-graphql)
- [Appeler un service gRPC](#appeler-un-service-grpc)
- [Reproduire une requête du navigateur](#reproduire-une-requête-du-navigateur)
- [Comprendre pourquoi une requête est lente ou échoue](#comprendre-pourquoi-une-requête-est-lente-ou-échoue)
- [Laisser un agent IA utiliser votre collection (MCP)](#laisser-un-agent-ia-utiliser-votre-collection-mcp)

## Envoyer et enregistrer votre première requête

1. **Fichier › Nouvelle collection…**, choisissez un dossier vide et donnez-lui un nom.
2. Ctrl+N ouvre une nouvelle requête GET. Tapez `https://jsonplaceholder.typicode.com/users`.
3. Ctrl+Enter l’envoie ; la réponse apparaît à droite.
4. Ctrl+S l’enregistre comme nouveau fichier dans le dossier de la collection.
   Clic droit dessus dans la barre latérale › **Renommer** pour lui donner un
   nom, par exemple « List users ».

Le fichier est du TOML simple, que vous pouvez lire et modifier :

```toml
name = "List users"
url = "https://jsonplaceholder.typicode.com/users"
```

## Garder une collection dans git

Une collection est un dossier :

```
my-api/
├── apix.toml                nom de la collection, variables, auth partagée
├── environments/
│   ├── staging.toml         valeurs partagées
│   └── staging.local.toml   valeurs secrètes (ignorées par git)
├── users/
│   ├── folder.toml          facultatif : auth des requêtes de ce dossier
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

Commitez tout sauf `*.local.toml` ; Apix ajoute lui-même ce motif à
`.gitignore` la première fois qu’il écrit un secret. Chaque requête est un
petit fichier à part, si bien que les revues et les fusions restent lisibles.

## Travailler avec dev, staging et production

1. Ouvrez **Environnement › Variables et environnements…** (Ctrl+E).
2. Ajoutez des environnements nommés `dev`, `staging` et `prod`, chacun avec un `base_url`.
3. Utilisez `{{base_url}}/users` dans vos requêtes.
4. Choisissez l’environnement dans la barre latérale, sous le nom de la collection.

Les valeurs qui ne doivent pas être commitées (jetons, mots de passe) reçoivent
la coche **Secret**. Chaque membre de l’équipe garde son propre `*.local.toml`.
Une variable définie au niveau de la collection est une valeur par défaut que
les environnements peuvent remplacer.

## Partager une collection qui contient des secrets

Quand toute la collection est sensible (URL internes, données clients
d’exemple, clés à longue durée de vie), chiffrez-la :

1. **Fichier › Chiffrer cette collection…** et choisissez un mot de passe.
2. Rangez la **clé de récupération** qui s’affiche dans votre gestionnaire de mots de passe.
3. Commitez le dossier. Sur le disque, c’est un ensemble d’objets chiffrés ;
   même les noms de fichiers sont cachés.

Les membres de l’équipe ouvrent le dossier dans Apix et tapent le mot de passe.
Dans un terminal ou en CI :

```sh
APIX_PASSWORD='…' apix run my-api --env staging
```

`apix decrypt my-api --out my-api-plain` écrit une copie en clair ; avec
`--recovery`, il utilise la clé de récupération au lieu du mot de passe.

## Passer de Postman à Apix

1. Dans Postman, exportez la collection (Collection v2.1) et chaque environnement.
2. Dans Apix, **Fichier › Importer (Postman, Insomnia, OpenAPI, HAR, WSDL)…**
   ou déposez les fichiers sur la fenêtre. Ou dans un terminal :

   ```sh
   apix import "My API.postman_collection.json"            # crée ./my-api
   apix import "Staging.postman_environment.json" --into my-api
   ```

3. Lisez le rapport d’import : tout ce qui n’a pas pu être repris y est listé
   (par exemple l’auth NTLM ou les réponses d’exemple enregistrées).
4. Les scripts de pré-requête et de test sont gardés en fichiers `.js` à côté
   de leurs requêtes ; Apix ne les exécute pas encore.
5. Pour OAuth 2.0 avec le flux code d’autorisation, enregistrez une URL de
   redirection de bouclage, par exemple `http://127.0.0.1:8765/callback`, auprès
   de votre fournisseur d’identité et indiquez-la dans l’onglet Auth.

Vous déplacez tout un espace de travail ? Utilisez **Settings › Data › Export
data** dans Postman, puis ouvrez ou déposez le dossier (ou le zip) dans Apix :
chaque collection, environnement et variable globale arrive sous forme d’un seul
projet (`apix import postman-export.zip` fait la même chose). Les membres de
l’équipe qui restent sur Postman récupèrent votre collection avec
**Exporter vers Postman…**.

## Exécuter une collection en CI

`apix run` envoie toutes les requêtes d’une collection dans l’ordre de la barre
latérale et se termine avec un code non nul si l’une d’elles échoue ; il
fonctionne donc comme étape de CI :

```sh
apix run my-api --env staging --var token="$API_TOKEN"
```

Les requêtes qui ont des tests (scripts ou Vérifications) réussissent ou
échouent selon leurs tests ; les autres selon leur code de statut. La sortie
liste chaque test sous sa requête :

```text
create.apix                              201 Created  466.7 ms · 42 B
  │ new id 101
  ✓ created
get.apix                                 200 OK  174.7 ms · 292 B
  ✓ status 200
  ✗ has the right id expected 1 to equal 2

1 passed, 1 failed (1 failed tests)
```

Ce qu’un script définit (`pm.environment.set`, `pm.variables.set`) passe aux
requêtes suivantes de l’exécution, et `pm.execution.setNextRequest("name")`
saute à une autre requête (`null` termine l’itération).

Options :

| Option | Ce qu’elle fait |
|---|---|
| `apix run my-api/users` | Seulement les requêtes d’un dossier (ou un seul fichier de requête) |
| `-d, --data users.csv` | Une exécution pilotée par les données : chaque ligne CSV ou JSON est une itération, ses colonnes sont des variables (`{{email}}`, `pm.iterationData.get("email")`) |
| `-n, --iterations 5` | Parcourir les requêtes 5 fois (par défaut : une fois par ligne de données, sinon une fois) |
| `--delay 200` | Attendre 200 ms entre les requêtes |
| `--bail` | S’arrêter à la première requête qui échoue |
| `-p, --parallel 4` | Lancer 4 itérations à la fois, comme 4 utilisateurs ; chacune envoie ses requêtes dans l’ordre et garde pour elle ce que ses scripts définissent |
| `--junit report.xml` | Un rapport JUnit XML, que les services de CI affichent comme une liste de tests |
| `--json report.json` / `--html report.html` | Tout en JSON, ou une page qui s’ouvre dans n’importe quel navigateur |
| `--markdown report.md` | Un résumé pour un commentaire de pull request ou un résumé de tâche CI (voir plus bas) |
| `--changed` / `--changed=origin/main` | Seulement les requêtes touchées par une modification (voir plus bas) |
| `--timeout 10`, `-k`, `--proxy URL` | Remplacer les réglages de transport des requêtes |
| `--env-file .env` | Variables tirées d’un fichier dotenv (voir plus bas) |

Un fichier de données ressemble à ceci ; les valeurs venues de l’extérieur de la
collection (`--var` et celles ci-dessous) l’emportent sur ses colonnes :

```csv
id,name
1,Leanne Graham
2,Ervin Howell
```

### Les secrets en CI

Les secrets restent hors de git (les fichiers `.local.toml` sont ignorés, le
trousseau est sur votre ordinateur) ; un pipeline les transmet donc de l’une de
ces trois façons, chacune l’emportant sur la précédente :

1. `--env-file .env` : un fichier dotenv de lignes `NAME=value` (commentaires
   `#`, guillemets et `export` acceptés).
2. Les variables d’environnement `APIX_VAR_<NAME>` : `APIX_VAR_TOKEN` remplit
   `{{token}}`. Les secrets de CI s’écrivent souvent en majuscules ; un nom
   correspond donc à une variable que la collection définit ou que ses requêtes
   utilisent, quelle que soit sa casse.
3. `--var name=value`.

Les trois l’emportent sur les valeurs de l’environnement (et sur les colonnes
d’un fichier de données). `apix` affiche sur stderr quels noms il a pris et
d’où, jamais les valeurs.

```sh
APIX_VAR_TOKEN="$API_TOKEN" apix run api --env staging
```

### GitHub Actions

Le dépôt est aussi une GitHub Action. Elle exécute la collection et écrit des
rapports JUnit et HTML. Utilisée sur un tag de version, elle télécharge l’`apix`
de cette version (quelques secondes) ; sur une branche ou un commit, elle
compile `apix` la première fois et le met en cache, et `apix-version: latest`
(ou `0.1.1`) télécharge plutôt une version publiée :

```yaml
- uses: 11R5/Apix@v0.1.1        # une version publiée : apix est téléchargé, pas compilé
  with:
    path: api
    environment: staging        # aussi : data, iterations, delay, bail, env-file, junit, html, json, markdown, changed, diff, comment, args, apix-version
  env:
    APIX_VAR_TOKEN: ${{ secrets.API_TOKEN }}
```

[examples/ci/github-actions.yml](../../examples/ci/github-actions.yml) est un
workflow complet : il exécute la collection une fois par environnement (une
matrice de `test` et `staging`), affiche les résultats JUnit sur la page de
l’exécution et garde les rapports HTML. L’action ajoute aussi son résumé
Markdown à la page de la tâche.

## Seulement ce qui a changé : exécutions qui connaissent git

Une collection est un dossier dans git ; `apix` peut donc demander à git ce
qu’une modification a touché. `--changed` n’envoie que ces requêtes :

```sh
apix run api --changed                 # ce qui n’est pas encore commité
apix run api --changed=origin/main     # ce que cette branche a changé depuis qu’elle a quitté main
```

Une requête est relancée quand son propre fichier ou ses scripts ont changé,
quand un `folder.toml` ou un script de dossier au-dessus d’elle a changé
(toutes les requêtes de ce dossier), quand `apix.toml`, un script de
collection, un environnement ou le fichier de projet a changé (toutes les
requêtes), ou quand elle nomme un autre fichier modifié (un fichier de corps,
un `.proto`). Les nouveaux fichiers comptent avant d’être commités ; les
requêtes supprimées sont listées. Sans valeur, `--changed` compare avec la
branche de base de la pull request sous GitHub Actions
(`origin/$GITHUB_BASE_REF`) et sinon avec `HEAD`. Écrivez la valeur avec `=`,
pour qu’elle ne soit pas prise pour le dossier à exécuter.

```text
2 of 3 requests changed since origin/main
  users/create-user.apix  ← users/create-user.test.js
  users/list-users.apix
```

Comme pour une pull request, la comparaison part de l’endroit où la branche a
quitté la base : ce qui est arrivé sur main entre-temps ne compte pas. Cela
demande l’historique : dans un clone superficiel (le réglage par défaut
d’`actions/checkout`), `apix` avertit et compare avec la base elle-même ;
faites le checkout avec `fetch-depth: 0`. Les collections chiffrées ne peuvent
pas être comparées, car git ne voit que leurs fichiers chiffrés.

### Ce qui a changé : `apix diff`

`apix diff` montre une modification comme un relecteur la lit : les champs
d’une requête plutôt que des lignes de TOML, les en-têtes et les paramètres de
query par nom, le corps et les scripts ligne par ligne, les environnements par
variable. Remettre en forme un fichier n’est pas une modification, et une
requête déplacée ou renommée sans autre changement compte pour un seul
déplacement.

```sh
apix diff                       # HEAD par rapport aux fichiers tels qu’ils sont
apix diff main                  # main par rapport à maintenant
apix diff v1.2 v1.3 -C api      # entre deux tags, pour la collection dans api/
```

```text
Between main and the working tree: 1 script, 1 request

A  script  users/create-user.test.js
     + pm.test("ok", () => pm.response.to.have.status(200));
M  GET List users  users/list-users.apix
     query._limit: 3 → 5
     headers.Accept: (none) → application/json
```

`--markdown` l’affiche pour une pull request, `--json` pour les outils, et
`--exit-code` se termine avec 1 quand quelque chose a changé.

### Un résumé sur la pull request

`--markdown report.md` écrit l’exécution en Markdown GitHub : les totaux, une
ligne par requête et, dépliés en dessous, ce qui a échoué et pourquoi. Sa
première ligne est un marqueur caché avec le nom de la collection ; une étape
de CI peut ainsi retrouver le commentaire qu’elle a écrit la dernière fois et
le modifier au lieu d’en ajouter un autre.

L’action fait tout cela. Sur une pull request, elle n’envoie que ce que la
pull request touche, ajoute `apix diff` sous le résumé et tient à jour un seul
commentaire sur la pull request :

```yaml
on: pull_request
permissions:
  contents: read
  pull-requests: write          # pour le commentaire
jobs:
  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0        # l’historique, pour trouver où la branche a commencé
      - uses: 11R5/Apix@main
        with:
          path: api
          changed: "true"       # ou un commit, une branche ou un tag
          diff: "true"
          comment: "true"
```

Sur un push, `changed: "true"` compare avec le commit d’avant le push. Le
résumé est aussi écrit dans `apix-report.md` (entrée `markdown`) et affiché sur
la page de la tâche.

Dans l’application de bureau, la barre d’état montre la branche et le nombre
de fichiers modifiés depuis le dernier commit, et la barre latérale marque les
requêtes modifiées avec **M** (modifiée) ou **A** (nouvelle) ; voir le
[guide de l’utilisateur](user-guide.md#git).

### Docker

Pour les autres systèmes de CI, il existe une image de conteneur qui ne
contient que la commande `apix` (publiée sur `ghcr.io/11r5/apix` quand une
version est taguée ; construisez-la vous-même avec `docker build -t apix .`) :

```sh
docker run --rm -v "$PWD:/work" -e APIX_VAR_TOKEN ghcr.io/11r5/apix run api --env staging --junit reports/apix.xml
```

[examples/ci/gitlab-ci.yml](../../examples/ci/gitlab-ci.yml) et
[examples/ci/azure-pipelines.yml](../../examples/ci/azure-pipelines.yml) font
la même chose dans GitLab CI (avec l’image) et Azure Pipelines (en compilant
`apix` une fois et en le mettant en cache) ; tous deux publient les résultats
JUnit.

Le **Runner…** de l’application de bureau (Ctrl+Shift+R) fait les mêmes
exécutions avec une fenêtre ; voir le [guide de l’utilisateur](user-guide.md#runner).

`apix send <file>` envoie une requête et affiche la réponse ; ajoutez `-i` pour
les en-têtes et `--network` pour les détails de la connexion.

## Surveiller une API avec des moniteurs

Un moniteur exécute une collection (ou un de ses dossiers ou une de ses
requêtes) selon un planning et prévient un webhook quand elle commence à
échouer et quand elle réussit de nouveau. Les moniteurs sont écrits dans
l’`apix.toml` de la collection, à la main ou depuis **Affichage › Moniteurs…**
dans l’application (voir le [guide de l’utilisateur](user-guide.md#moniteurs)) :

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "health.apix"
environment = "production"
webhook = "{{alerts_url}}"
```

`apix monitor` les exécute tant qu’on le laisse tourner, chacun selon son
propre planning, et relit `apix.toml` au fur et à mesure, pour que les
modifications prennent effet :

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

| Option | Ce qu’elle fait |
|---|---|
| `--once` | Exécuter chaque moniteur une fois puis quitter, avec 1 si l’un échoue : pour cron, le Planificateur de tâches ou un planning de CI |
| `--list` | Les moniteurs et leurs derniers résultats |
| `--only NAME` | Seulement ce moniteur (répétable) ; avec `--once`, il tourne même quand il est désactivé |
| `--var`, `--env-file` | Variables par-dessus les environnements des moniteurs, comme pour `apix run` (`APIX_VAR_*` sont lues aussi) |
| `--log-dir DIR` | Garder le journal ailleurs (en CI, un dossier mis en cache) |
| `--timeout`, `-k`, `--proxy`, `--cacert` | Réglages de transport, comme pour `apix run` |

Le journal garde les 500 derniers résultats de chaque moniteur dans
`<config>/Apix/monitors` ; l’application de bureau lit les mêmes fichiers et
montre donc ce qu’`apix monitor` a trouvé sur le même ordinateur. Un webhook
n’est prévenu que quand l’état d’un moniteur bascule (en échec après une
réussite, ou à sa première exécution ; en réussite après un échec), avec un
corps JSON que Slack, Discord, Mattermost et Google Chat affichent comme un
message.

Sur un serveur, lancez `apix monitor api` comme service, ou mettez
`apix monitor api --once` dans cron (`*/5 * * * *`). Sur GitHub, un workflow
planifié s’en charge :
[examples/ci/monitors.yml](../../examples/ci/monitors.yml) exécute les
moniteurs toutes les 15 minutes via l’action (`monitor: "true"`) et met le
journal en cache pour que les rétablissements soient remarqués.

## Se connecter avec OAuth 2.0

**Identifiants client** (de service à service) :

1. Onglet Auth › **OAuth 2.0**, type d’octroi **Identifiants client**.
2. Remplissez **URL du jeton**, **ID client**, **Secret client** et **Portée**.
   Utilisez des variables (`{{client_secret}}`, marquée secrète) plutôt que des
   secrets en clair.
3. Envoyez. Apix obtient un jeton, puis le réutilise et le renouvelle.

**Code d’autorisation avec PKCE** (un utilisateur se connecte) :

1. Enregistrez `http://127.0.0.1:<port>/callback` comme URL de redirection pour
   votre client (ou laissez **URL de redirection** vide si le fournisseur
   accepte n’importe quel port de bouclage).
2. Type d’octroi **Code d’autorisation**, remplissez **URL d’autorisation**,
   **URL du jeton**, **ID client** et **Portée** ; laissez
   **Utiliser PKCE (recommandé)** coché.
3. Appuyez sur **Obtenir un nouveau jeton** ou envoyez. Votre navigateur ouvre
   la page de connexion ; une fois connecté, le jeton apparaît dans Apix.

Définissez OAuth 2.0 une seule fois sur la collection (clic droit ›
**Authentification…**) et chaque requête qui hérite de l’auth utilise le même
jeton. Les jetons restent en mémoire ; au redémarrage d’Apix, il faut se
reconnecter.

## Appeler une API AWS

1. Onglet Auth › **AWS Signature**.
2. **Clé d’accès** et **Clé secrète** en variables (`{{aws_access_key}}`,
   `{{aws_secret_key}}` marquées secrètes), **Jeton de session** pour des
   identifiants temporaires, **Région** (par exemple `eu-central-1`) et
   **Service** (par exemple `execute-api`, `s3`, `lambda`).
3. Envoyez. Chaque requête est signée avec Signature Version 4 juste avant de
   partir.

**Copier en › cURL** écrit `curl --aws-sigv4`, pour que curl la signe de
nouveau quand vous la lancez ; la copie PowerShell contient une signature faite
au moment de la copie, valable quelques minutes.

## Envoyer un fichier, envoyer du GraphQL

**Multipart :** Corps › **Multipart**, ajoutez un champ, passez-le en
**Fichier** et choisissez le fichier. Les champs texte restent en **Texte**.
Gardez les fichiers de test dans la collection : le chemin est enregistré
relativement à la requête, il fonctionne donc sur toutes les machines.

**Binaire :** Corps › **Binaire** envoie un seul fichier comme corps entier.

**GraphQL :** Corps › **GraphQL**, écrivez la requête GraphQL, mettez les
variables en JSON en dessous. Apix envoie `{"query": …, "variables": …}` avec
`Content-Type: application/json`. Réglez d’abord l’auth, puis appuyez sur
**Récupérer le schéma** : à partir de là, l’éditeur de requête suggère champs,
arguments et valeurs d’enum, et le survol affiche leur documentation. Le schéma
est gardé par URL : la prochaine fois que vous ouvrez la requête, il est déjà
là. Pour vérifier un changement de schéma dans une pull request, enregistrez
`apix schema api/graphql.apix > schema.graphql` dans le dépôt et laissez le diff
montrer ce qui a changé.

## Appeler un service gRPC

1. **Requête › Nouvelle requête**, puis tapez l’adresse avec `grpc://` (ou
   `grpcs://` derrière TLS), par exemple `grpc://localhost:50051`.
2. Dans l’onglet **Message**, appuyez sur **Charger les méthodes**. Si la
   réflexion du serveur est désactivée, ajoutez d’abord les fichiers `.proto`
   (**Ajouter un fichier**) ; gardez-les dans la collection pour que les chemins
   fonctionnent sur toutes les machines.
3. Choisissez la méthode, appuyez sur **Message d'exemple**, remplissez les valeurs.
4. Indiquez le jeton dans **Auth** (Bearer) ou ajoutez des métadonnées dans
   **En-têtes**, puis **Envoyer**.

Dans un pipeline, le même fichier fonctionne avec la ligne de commande :

```sh
apix send api/say-hello.apix --env staging   # code de sortie 1 sauf si le statut est OK
apix schema api/say-hello.apix > greeter.txt   # ce que le service propose, à comparer dans les revues
```

## Reproduire une requête du navigateur

1. Dans les outils de développement du navigateur, clic droit sur une requête ›
   **Copy as cURL** (bash ou cmd, les deux fonctionnent).
2. Dans Apix, cliquez dans le champ URL et collez. Méthode, URL, query,
   en-têtes, cookies, auth et corps sont remplis.
3. Envoyez, modifiez, enregistrez.

Dans l’autre sens, le bouton de copie à côté d’**Envoyer** vous donne la
requête en `curl`, `curl.exe` ou PowerShell, à coller dans un terminal ou un
rapport de bug.

## Comprendre pourquoi une requête est lente ou échoue

1. Envoyez la requête et ouvrez l’onglet **Réseau** de la réponse.
2. La barre des temps montre si le temps passe dans le DNS, l’ouverture de la
   connexion TCP, la poignée de main TLS, l’attente du serveur ou le
   téléchargement. Une connexion TCP lente désigne le réseau ; une poignée de
   main lente (comparée à TCP, qui prend un aller-retour) désigne le serveur ou
   l’aller-retour supplémentaire de TLS 1.2.
3. Vérifiez l’adresse distante et l’interface locale : un VPN ou un proxy s’y
   voit.
4. Les redirections sont listées dans l’ordre. Sous Chiffrement : la version de
   TLS, la suite de chiffrement et si HTTP/2 a été négocié ; la chaîne de
   certificats montre l’émetteur de chaque certificat et les jours restants, si
   bien qu’un intermédiaire manquant ou un certificat proche de l’expiration
   saute aux yeux. Un corps compressé montre combien il a fait gagner.

Le même rapport est disponible dans un terminal :

```sh
apix send my-api/health.apix --network
```

## Laisser un agent IA utiliser votre collection (MCP)

`apix mcp` sert une collection via le Model Context Protocol, pour qu’un agent
IA (Claude, l’assistant d’un éditeur, n’importe quel client MCP) puisse
travailler avec votre API comme vous le faites dans Apix. Il parle MCP sur
l’entrée et la sortie standard ; c’est l’agent qui le lance. Avec Claude Code :

```bash
claude mcp add my-api -- apix mcp /path/to/my-api
```

Les autres clients prennent la même commande dans leur configuration, par
exemple :

```json
{ "mcpServers": { "my-api": { "command": "apix", "args": ["mcp", "/path/to/my-api"] } } }
```

L’agent reçoit quatre outils :

| Outil | Ce qu’il fait |
|---|---|
| `list_requests` | les requêtes (chemin, méthode, nom, URL avec `{{variables}}`), de toute la collection ou d’un dossier |
| `list_environments` | les environnements et les noms de leurs variables, jamais les valeurs |
| `send_request` | envoie une requête avec ses scripts et ses tests, dans un environnement, avec des variables que l’agent définit ; renvoie statut, temps, en-têtes, corps (jusqu’à 64 KB) et résultats des tests |
| `run_requests` | exécute un dossier ou toute la collection avec ses tests et indique ce qui a réussi |

Seules les requêtes de cette collection sont accessibles. `--list-only` garde
les deux outils de liste et retire ceux qui envoient. Les secrets restent où ils
sont : un agent peut demander qu’une requête soit envoyée avec un jeton, mais
on ne lui montre jamais la valeur du jeton (une réponse qui la renvoie est une
autre affaire : faites attention à ce que vous laissez un agent envoyer). Pour
une collection chiffrée, donnez le mot de passe dans `APIX_PASSWORD`.
