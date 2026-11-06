<!-- translated from en/workflows.md sha256:96cb14d5eeb691a0 -->

# Flujos de trabajo

Recetas paso a paso para tareas habituales. La [guía de usuario](user-guide.md)
describe cada parte de la aplicación; las [notas técnicas](technical.md) tratan
la compilación y la medición de Apix.

- [Enviar y guardar tu primera solicitud](#enviar-y-guardar-tu-primera-solicitud)
- [Guardar una colección en git](#guardar-una-colección-en-git)
- [Trabajar con dev, staging y producción](#trabajar-con-dev-staging-y-producción)
- [Compartir una colección que contiene secretos](#compartir-una-colección-que-contiene-secretos)
- [Migrar desde Postman](#migrar-desde-postman)
- [Ejecutar una colección en CI](#ejecutar-una-colección-en-ci)
- [Vigilar una API con monitores](#vigilar-una-api-con-monitores)
- [Iniciar sesión con OAuth 2.0](#iniciar-sesión-con-oauth-20)
- [Llamar a una API de AWS](#llamar-a-una-api-de-aws)
- [Subir un archivo, enviar GraphQL](#subir-un-archivo-enviar-graphql)
- [Llamar a un servicio gRPC](#llamar-a-un-servicio-grpc)
- [Reproducir una solicitud del navegador](#reproducir-una-solicitud-del-navegador)
- [Averiguar por qué una solicitud es lenta o falla](#averiguar-por-qué-una-solicitud-es-lenta-o-falla)
- [Dejar que un agente de IA use tu colección (MCP)](#dejar-que-un-agente-de-ia-use-tu-colección-mcp)

## Enviar y guardar tu primera solicitud

1. **Archivo › Nueva colección…**, elige una carpeta vacía y ponle un nombre.
2. Ctrl+N abre una nueva solicitud GET. Escribe `https://jsonplaceholder.typicode.com/users`.
3. Ctrl+Enter la envía; la respuesta aparece a la derecha.
4. Ctrl+S la guarda como un archivo nuevo en la carpeta de la colección. Haz
   clic derecho sobre ella en la barra lateral › **Cambiar nombre** para darle
   un nombre como "List users".

El archivo es TOML simple que puedes leer y editar:

```toml
name = "List users"
url = "https://jsonplaceholder.typicode.com/users"
```

## Guardar una colección en git

Una colección es una carpeta:

```
my-api/
├── apix.toml                nombre de la colección, variables, autenticación compartida
├── environments/
│   ├── staging.toml         valores compartidos
│   └── staging.local.toml   valores secretos (en .gitignore)
├── users/
│   ├── folder.toml          opcional: autenticación para las solicitudes de esta carpeta
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

Haz commit de todo excepto `*.local.toml`; Apix añade ese patrón a
`.gitignore` por sí mismo la primera vez que escribe un secreto. Cada solicitud
es un archivo pequeño propio, así que las revisiones y las fusiones siguen
siendo legibles.

## Trabajar con dev, staging y producción

1. Abre **Entorno › Variables y entornos…** (Ctrl+E).
2. Añade entornos llamados `dev`, `staging` y `prod`, cada uno con un `base_url`.
3. Usa `{{base_url}}/users` en tus solicitudes.
4. Elige el entorno en la barra lateral, bajo el nombre de la colección.

Los valores que no deben ir en un commit (tokens, contraseñas) llevan la marca
**Secreto**. Cada compañero de equipo tiene su propio `*.local.toml`. Una
variable definida a nivel de colección es un valor predeterminado que los
entornos pueden sobrescribir.

## Compartir una colección que contiene secretos

Cuando toda la colección es sensible (URL internas, datos de clientes de
ejemplo, claves de larga duración), cífrala:

1. **Archivo › Cifrar esta colección…** y elige una contraseña.
2. Guarda la **clave de recuperación** que se te muestra en tu gestor de
   contraseñas.
3. Haz commit de la carpeta. En el disco es un conjunto de objetos cifrados;
   incluso los nombres de archivo quedan ocultos.

Tus compañeros abren la carpeta en Apix y escriben la contraseña. En una
terminal o en CI:

```sh
APIX_PASSWORD='…' apix run my-api --env staging
```

`apix decrypt my-api --out my-api-plain` escribe una copia sin cifrar; con
`--recovery` se usa la clave de recuperación en lugar de la contraseña.

## Migrar desde Postman

1. En Postman, exporta la colección (Collection v2.1) y cada entorno.
2. En Apix, **Archivo › Importar…** o suelta los archivos en la ventana.
   O en una terminal:

   ```sh
   apix import "My API.postman_collection.json"            # crea ./my-api
   apix import "Staging.postman_environment.json" --into my-api
   ```

3. Lee el informe de importación: todo lo que no se pudo trasladar aparece ahí
   (por ejemplo, la autenticación NTLM o las respuestas de ejemplo guardadas).
4. Los scripts previos a la solicitud y de prueba se conservan como archivos
   `.js` junto a sus solicitudes; Apix todavía no los ejecuta.
5. Para OAuth 2.0 con el flujo de código de autorización, registra en tu
   proveedor de identidad una URL de redirección de loopback como
   `http://127.0.0.1:8765/callback` y ponla en la pestaña Autenticación.

¿Migras todo un espacio de trabajo? Usa **Settings › Data › Export data** de
Postman y después abre o suelta la carpeta (o el zip) en Apix: cada colección,
entorno y variable global llega como un único proyecto
(`apix import postman-export.zip` hace lo mismo). Los compañeros que siguen en
Postman recuperan tu colección con **Exportar a Postman…**.

## Ejecutar una colección en CI

`apix run` envía cada solicitud de una colección en el orden de la barra
lateral y termina con un código distinto de cero si alguna falla, así que
sirve como paso de CI:

```sh
apix run my-api --env staging --var token="$API_TOKEN"
```

Las solicitudes con pruebas (scripts o Comprobaciones) pasan o fallan según sus
pruebas; las demás, según su código de estado. La salida lista cada prueba bajo
su solicitud:

```text
create.apix                              201 Created  466.7 ms · 42 B
  │ new id 101
  ✓ created
get.apix                                 200 OK  174.7 ms · 292 B
  ✓ status 200
  ✗ has the right id expected 1 to equal 2

1 passed, 1 failed (1 failed tests)
```

Lo que define un script (`pm.environment.set`, `pm.variables.set`) se
mantiene en las siguientes solicitudes de la ejecución, y
`pm.execution.setNextRequest("name")` salta a otra solicitud (`null` termina
la iteración).

Opciones:

| Opción | Qué hace |
|---|---|
| `apix run my-api/users` | Solo las solicitudes de una carpeta (o un archivo de solicitud) |
| `-d, --data users.csv` | Una ejecución basada en datos: cada fila CSV o JSON es una iteración y sus columnas son variables (`{{email}}`, `pm.iterationData.get("email")`) |
| `-n, --iterations 5` | Recorrer las solicitudes 5 veces (por defecto: una vez por fila de datos, o si no, una vez) |
| `--delay 200` | Esperar 200 ms entre solicitudes |
| `--bail` | Parar en la primera solicitud que falle |
| `-p, --parallel 4` | Ejecutar 4 iteraciones a la vez, como 4 usuarios; cada una envía sus solicitudes en orden y se queda para sí lo que definen sus scripts |
| `--junit report.xml` | Un informe JUnit XML, que los servicios de CI muestran como lista de pruebas |
| `--json report.json` / `--html report.html` | Todo como JSON, o una página que se abre en cualquier navegador |
| `--markdown report.md` | Un resumen para un comentario en un pull request o para el resumen de un trabajo de CI (ver más abajo) |
| `--changed` / `--changed=origin/main` | Solo las solicitudes a las que afecta un cambio (ver más abajo) |
| `--timeout 10`, `-k`, `--proxy URL` | Sobrescribir los ajustes de transporte de las solicitudes |
| `--env-file .env` | Variables de un archivo dotenv (ver más abajo) |

Un archivo de datos tiene este aspecto; los valores de fuera de la colección
(`--var` y los de más abajo) tienen prioridad sobre sus columnas:

```csv
id,name
1,Leanne Graham
2,Ervin Howell
```

### Secretos en CI

Los secretos se quedan fuera de git (los archivos `.local.toml` se ignoran, el
llavero está en tu equipo), así que un pipeline los entrega de una de estas
tres formas, cada una con prioridad sobre la anterior:

1. `--env-file .env`: un archivo dotenv con líneas `NAME=value` (se admiten
   comentarios con `#`, comillas y `export`).
2. Variables de entorno `APIX_VAR_<NAME>`: `APIX_VAR_TOKEN` rellena
   `{{token}}`. Los secretos de CI suelen escribirse en mayúsculas, así que un
   nombre coincide con una variable que la colección define o que usan sus
   solicitudes, sin importar mayúsculas y minúsculas.
3. `--var name=value`.

Las tres tienen prioridad sobre los valores del entorno (y sobre las columnas
de un archivo de datos). `apix` imprime en stderr qué nombres tomó y de dónde,
nunca los valores.

```sh
APIX_VAR_TOKEN="$API_TOKEN" apix run api --env staging
```

### GitHub Actions

El repositorio también es una GitHub Action. Ejecuta la colección y escribe
informes JUnit y HTML. Usada en una etiqueta de versión, descarga el `apix` de
esa versión (segundos); en una rama o un commit compila `apix` la primera vez
y lo guarda en caché, y `apix-version: latest` (o `0.1.1`) descarga en su
lugar una versión publicada:

```yaml
- uses: 11R5/Apix@v0.1.1        # una versión: apix se descarga, no se compila
  with:
    path: api
    environment: staging        # también: data, iterations, delay, bail, env-file, junit, html, json, markdown, changed, diff, comment, args, apix-version
  env:
    APIX_VAR_TOKEN: ${{ secrets.API_TOKEN }}
```

La misma action aparece en el GitHub Marketplace como
[`11R5/apix-action`](https://github.com/11R5/apix-action). Siempre descarga un
`apix` publicado: `uses: 11R5/apix-action@v1` toma la última versión,
`@v0.1.1` esa versión concreta, y `apix-version` tiene prioridad sobre ambas.

[examples/ci/github-actions.yml](../../examples/ci/github-actions.yml) es un
workflow completo: ejecuta la colección una vez por entorno (una matriz de
`test` y `staging`), muestra los resultados JUnit en la página de la ejecución
y conserva los informes HTML. La action además añade su resumen en Markdown a
la página del trabajo.

## Solo lo que cambió: ejecuciones que conocen git

Una colección es una carpeta en git, así que `apix` puede preguntar a git a
qué afectó un cambio. `--changed` envía solo esas solicitudes:

```sh
apix run api --changed                 # lo que todavía no tiene commit
apix run api --changed=origin/main     # lo que esta rama cambió desde que salió de main
```

Una solicitud se vuelve a ejecutar cuando cambió su propio archivo o sus
scripts, cuando cambió un `folder.toml` o un script de carpeta por encima de
ella (todas las solicitudes de esa carpeta), cuando cambió `apix.toml`, un
script de la colección, un entorno o el archivo del proyecto (todas las
solicitudes), o cuando nombra otro archivo cambiado (un archivo de cuerpo, un
`.proto`). Los archivos nuevos cuentan antes de tener commit; las solicitudes
eliminadas se listan. Sin valor, `--changed` compara con la rama base del pull
request en GitHub Actions (`origin/$GITHUB_BASE_REF`) y, si no, con `HEAD`.
Escribe el valor con `=`, para que no se confunda con la carpeta que se va a
ejecutar.

```text
2 of 3 requests changed since origin/main
  users/create-user.apix  ← users/create-user.test.js
  users/list-users.apix
```

Como en un pull request, la comparación empieza donde la rama salió de la
base, así que lo que llegó a main mientras tanto no cuenta. Para eso hace falta
el historial: en un clon superficial (lo predeterminado en `actions/checkout`)
`apix` avisa y compara con la propia base; haz el checkout con
`fetch-depth: 0`. Las colecciones cifradas no se pueden comparar, porque git
solo ve sus archivos cifrados.

### Qué cambió: `apix diff`

`apix diff` muestra un cambio tal como lo lee un revisor: los campos de una
solicitud en lugar de líneas de TOML, los encabezados y los parámetros de la
query por nombre, el cuerpo y los scripts línea a línea, los entornos por
variable. Reformatear un archivo no es un cambio, y una solicitud movida o
renombrada sin otras ediciones es un único movimiento.

```sh
apix diff                       # HEAD frente a los archivos tal como están ahora
apix diff main                  # main frente a ahora
apix diff v1.2 v1.3 -C api      # entre dos etiquetas, para la colección de api/
```

```text
Between main and the working tree: 1 script, 1 request

A  script  users/create-user.test.js
     + pm.test("ok", () => pm.response.to.have.status(200));
M  GET List users  users/list-users.apix
     query._limit: 3 → 5
     headers.Accept: (none) → application/json
```

`--markdown` lo imprime para un pull request, `--json` para herramientas, y
`--exit-code` termina con 1 cuando algo cambió.

### Un resumen en el pull request

`--markdown report.md` escribe la ejecución como Markdown de GitHub: los
totales, una fila por solicitud y, desplegado debajo, qué falló y por qué. Su
primera línea es un marcador oculto con el nombre de la colección, para que un
paso de CI pueda encontrar el comentario que escribió la última vez y editarlo
en lugar de añadir otro.

La action hace todo esto. En un pull request envía solo lo que toca el pull
request, añade `apix diff` bajo el resumen y mantiene actualizado un único
comentario en el pull request:

```yaml
on: pull_request
permissions:
  contents: read
  pull-requests: write          # para el comentario
jobs:
  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0        # historial, para encontrar dónde empezó la rama
      - uses: 11R5/Apix@main
        with:
          path: api
          changed: "true"       # o un commit, una rama o una etiqueta
          diff: "true"
          comment: "true"
```

En un push, `changed: "true"` compara con el commit anterior al push. El
resumen también se escribe en `apix-report.md` (entrada `markdown`) y se
muestra en la página del trabajo.

En la aplicación de escritorio, la barra de estado muestra la rama y cuántos
archivos cambiaron desde el último commit, y la barra lateral marca las
solicitudes cambiadas con **M** (cambiada) o **A** (nueva); consulta la
[guía de usuario](user-guide.md#git).

### Docker

Para otros sistemas de CI hay una imagen de contenedor que solo contiene el
comando `apix` (publicada en `ghcr.io/11r5/apix` cuando se etiqueta una
versión; compílala tú mismo con `docker build -t apix .`):

```sh
docker run --rm -v "$PWD:/work" -e APIX_VAR_TOKEN ghcr.io/11r5/apix run api --env staging --junit reports/apix.xml
```

[examples/ci/gitlab-ci.yml](../../examples/ci/gitlab-ci.yml) y
[examples/ci/azure-pipelines.yml](../../examples/ci/azure-pipelines.yml) hacen
lo mismo en GitLab CI (con la imagen) y en Azure Pipelines (compilando `apix`
una vez y guardándolo en caché); ambos publican los resultados JUnit.

El **Runner** de la aplicación de escritorio (Ctrl+Shift+R) hace las mismas
ejecuciones con una ventana; consulta la [guía de usuario](user-guide.md#runner).

`apix send <file>` envía una solicitud e imprime la respuesta; añade `-i` para
los encabezados y `--network` para los detalles de la conexión.

## Vigilar una API con monitores

Un monitor ejecuta una colección (o una carpeta o una solicitud de ella) según
una programación y avisa a un webhook cuando empieza a fallar y cuando vuelve a
pasar. Los monitores se escriben en el `apix.toml` de la colección, a mano o
desde **Ver › Monitores…** en la aplicación (consulta la
[guía de usuario](user-guide.md#monitores)):

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "health.apix"
environment = "production"
webhook = "{{alerts_url}}"
```

`apix monitor` los ejecuta mientras se deje en marcha, cada uno con su propia
programación, y vuelve a leer `apix.toml` sobre la marcha, así que las
ediciones surten efecto:

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

| Opción | Qué hace |
|---|---|
| `--once` | Ejecutar cada monitor una vez y salir, con 1 si alguno falla: para cron, el Programador de tareas o una programación de CI |
| `--list` | Los monitores y sus últimos resultados |
| `--only NAME` | Solo este monitor (se puede repetir); con `--once` se ejecuta aunque esté desactivado |
| `--var`, `--env-file` | Variables por encima de los entornos de los monitores, como en `apix run` (también se leen las `APIX_VAR_*`) |
| `--log-dir DIR` | Guardar el registro en otro lugar (en CI, una carpeta en caché) |
| `--timeout`, `-k`, `--proxy`, `--cacert` | Ajustes de transporte, como en `apix run` |

El registro guarda los últimos 500 resultados de cada monitor en
`<config>/Apix/monitors`; la aplicación de escritorio lee los mismos archivos,
así que muestra lo que encontró `apix monitor` en el mismo equipo. Solo se
avisa a un webhook cuando cambia el estado de un monitor (falla después de
pasar, o en su primera ejecución; pasa después de fallar), con un cuerpo JSON
que Slack, Discord, Mattermost y Google Chat muestran como mensaje.

En un servidor, inicia `apix monitor api` como servicio, o pon
`apix monitor api --once` en cron (`*/5 * * * *`). En GitHub lo hace un
workflow programado:
[examples/ci/monitors.yml](../../examples/ci/monitors.yml) ejecuta los
monitores cada 15 minutos mediante la action (`monitor: "true"`) y guarda el
registro en caché para que se noten las recuperaciones.

## Iniciar sesión con OAuth 2.0

**Credenciales de cliente** (de servicio a servicio):

1. Pestaña Autenticación › **OAuth 2.0**, tipo de concesión **Credenciales de
   cliente**.
2. Rellena **URL del token**, **ID de cliente**, **Secreto de cliente** y
   **Ámbito**. Usa variables (`{{client_secret}}`, marcada como secreta) en
   lugar de secretos escritos literalmente.
3. Envía. Apix obtiene un token y después lo reutiliza y lo renueva.

**Código de autorización con PKCE** (un usuario inicia sesión):

1. Registra `http://127.0.0.1:<port>/callback` como URL de redirección de tu
   cliente (o deja **URL de redirección** vacía si el proveedor acepta
   cualquier puerto de loopback).
2. Tipo de concesión **Código de autorización**, rellena **URL de
   autorización**, **URL del token**, **ID de cliente** y **Ámbito**; deja
   marcada **Usar PKCE (recomendado)**.
3. Pulsa **Obtener nuevo token** o envía. Tu navegador abre la página de inicio
   de sesión; después de iniciar sesión, el token aparece en Apix.

Configura OAuth 2.0 una sola vez en la colección (clic derecho ›
**Autenticación…**) y todas las solicitudes que heredan la autenticación usan
el mismo token. Los tokens se quedan en memoria; al reiniciar Apix se vuelve a
iniciar sesión.

## Llamar a una API de AWS

1. Pestaña Autenticación › **AWS Signature**.
2. **Clave de acceso** y **Clave secreta** como variables (`{{aws_access_key}}`,
   `{{aws_secret_key}}` marcadas como secretas), **Token de sesión** para
   credenciales temporales, **Región** (por ejemplo `eu-central-1`) y
   **Servicio** (por ejemplo `execute-api`, `s3`, `lambda`).
3. Envía. Cada solicitud se firma con Signature Version 4 justo antes de salir.

**Copiar como › cURL** escribe `curl --aws-sigv4`, así que curl la vuelve a
firmar cuando la ejecutas; la copia de PowerShell contiene una firma hecha en
el momento de copiar, válida durante unos minutos.

## Subir un archivo, enviar GraphQL

**Multipart:** Cuerpo › **Multipart**, añade un campo, cámbialo a **Archivo** y
elige el archivo. Los campos de texto se quedan en **Texto**. Guarda los
archivos de prueba dentro de la colección: la ruta se guarda relativa a la
solicitud, así que funciona en cualquier máquina.

**Binario:** Cuerpo › **Binario** envía un archivo como cuerpo completo.

**GraphQL:** Cuerpo › **GraphQL**, escribe la consulta y pon las variables
como JSON debajo. Apix envía `{"query": …, "variables": …}` con
`Content-Type: application/json`. Configura primero la autenticación y
después pulsa **Obtener esquema**: a partir de ahí el editor de consultas
sugiere campos, argumentos y valores de enum, y al pasar el ratón muestra su
documentación. El esquema se guarda por URL, así que la próxima vez que abras
la solicitud ya estará ahí. Para revisar un cambio de esquema en un pull
request, guarda `apix schema api/graphql.apix > schema.graphql` en el
repositorio y deja que el diff muestre qué cambió.

## Llamar a un servicio gRPC

1. **Solicitud › Nueva solicitud** y escribe la dirección con `grpc://` (o
   `grpcs://` detrás de TLS), por ejemplo `grpc://localhost:50051`.
2. En la pestaña **Mensaje**, pulsa **Cargar métodos**. Si la reflection del
   servidor está desactivada, añade antes los archivos `.proto` (**Añadir
   archivo**); guárdalos en la colección para que las rutas funcionen en
   cualquier máquina.
3. Elige el método, pulsa **Mensaje de ejemplo** y rellena los valores.
4. Pon el token en **Autenticación** (Bearer) o añade metadatos en
   **Encabezados**, y después **Enviar**.

En un pipeline, el mismo archivo funciona con la línea de comandos:

```sh
apix send api/say-hello.apix --env staging   # código de salida 1 salvo que el estado sea OK
apix schema api/say-hello.apix > greeter.txt   # lo que ofrece el servicio, para comparar en las revisiones
```

## Reproducir una solicitud del navegador

1. En las herramientas para desarrolladores del navegador, haz clic derecho en
   una solicitud › **Copy as cURL** (bash o cmd, funcionan las dos).
2. En Apix, haz clic en el campo de la URL y pega. Se rellenan el método, la
   URL, la query, los encabezados, las cookies, la autenticación y el cuerpo.
3. Envía, cambia, guarda.

En sentido contrario, el botón de copiar junto a **Enviar** te da la solicitud
como `curl`, `curl.exe` o PowerShell para pegarla en una terminal o en un
informe de error.

## Averiguar por qué una solicitud es lenta o falla

1. Envía la solicitud y abre la pestaña **Red** de la respuesta.
2. La barra de tiempos muestra si el tiempo se va en el DNS, en abrir la
   conexión TCP, en el handshake TLS, en esperar al servidor o en la descarga.
   Una conexión TCP lenta apunta a la red; un handshake lento (comparado con
   TCP, que tarda un viaje de ida y vuelta) apunta al servidor o al viaje de
   ida y vuelta adicional de TLS 1.2.
3. Comprueba la dirección remota y la interfaz local: una VPN o un proxy se
   ven ahí.
4. Las redirecciones se listan en orden. En Cifrado: la versión de TLS, el
   conjunto de cifrado y si se acordó HTTP/2; la cadena de certificados
   muestra el emisor de cada certificado y los días que le quedan, así que
   destaca un intermedio que falta o uno a punto de caducar. Un cuerpo
   comprimido muestra cuánto se ahorró.

El mismo informe está disponible en una terminal:

```sh
apix send my-api/health.apix --network
```

## Dejar que un agente de IA use tu colección (MCP)

`apix mcp` sirve una colección mediante el Model Context Protocol, para que un
agente de IA (Claude, el asistente de un editor, cualquier cliente MCP) pueda
trabajar con tu API como tú lo haces en Apix. Habla MCP por la entrada y la
salida estándar; el agente lo inicia. Con Claude Code:

```bash
claude mcp add my-api -- apix mcp /path/to/my-api
```

Otros clientes aceptan el mismo comando en su configuración, por ejemplo:

```json
{ "mcpServers": { "my-api": { "command": "apix", "args": ["mcp", "/path/to/my-api"] } } }
```

El agente obtiene cuatro herramientas:

| Herramienta | Qué hace |
|---|---|
| `list_requests` | las solicitudes (ruta, método, nombre, URL con `{{variables}}`) de toda la colección o de una carpeta |
| `list_environments` | los entornos y los nombres de sus variables, nunca los valores |
| `send_request` | envía una solicitud con sus scripts y pruebas, en un entorno, con las variables que define el agente; devuelve el estado, el tiempo, los encabezados, el cuerpo (hasta 64 KB) y los resultados de las pruebas |
| `run_requests` | ejecuta una carpeta o toda la colección con sus pruebas e informa de lo que las superó |

Solo se puede llegar a las solicitudes de esa colección. `--list-only`
mantiene las dos herramientas de listado y quita las que envían. Los secretos
se quedan donde están: un agente puede pedir que se envíe una solicitud con un
token, pero nunca ve el valor del token (una respuesta que lo repita es otra
cuestión, así que vigila lo que dejas enviar a un agente). Para una colección
cifrada, da la contraseña en `APIX_PASSWORD`.
