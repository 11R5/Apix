<!-- translated from en/user-guide.md sha256:302bcb1b0e41cc49 -->

# Guía de usuario de Apix

Apix es un cliente de API de escritorio: creas solicitudes HTTP, las envías y
lees las respuestas. Las solicitudes se guardan como pequeños archivos de texto
en una carpeta, así que una colección puede vivir en git junto a tu código. Esta
guía recorre la aplicación; los [flujos de trabajo](workflows.md) explican paso
a paso las tareas habituales, y la línea de comandos `apix` ejecuta las mismas
solicitudes en una terminal o en CI.

![La ventana de Apix](../en/images/main.png)

## Contenido

- [Instalar Apix](#instalar-apix)
- [Abrir una colección](#abrir-una-colección)
- [La ventana](#la-ventana)
- [Enviar una solicitud](#enviar-una-solicitud)
- [Parámetros y encabezados](#parámetros-y-encabezados)
- [Cuerpo](#cuerpo)
- [Autenticación](#autenticación)
- [Ajustes de la solicitud](#ajustes-de-la-solicitud)
- [Cookies](#cookies)
- [Variables y entornos](#variables-y-entornos)
- [La respuesta](#la-respuesta)
- [La pestaña Red](#la-pestaña-red)
- [WebSocket y eventos enviados por el servidor](#websocket-y-eventos-enviados-por-el-servidor)
- [Socket.IO](#socketio)
- [MQTT](#mqtt)
- [SOAP](#soap)
- [gRPC](#grpc)
- [Scripts y pruebas](#scripts-y-pruebas)
- [Runner](#runner)
- [Flujos](#flujos)
- [Pruebas de carga](#pruebas-de-carga)
- [Monitores](#monitores)
- [Servidor simulado](#servidor-simulado)
- [Documentación de la API](#documentación-de-la-api)
- [Proxy de captura](#proxy-de-captura)
- [Pestañas](#pestañas)
- [Organizar las solicitudes](#organizar-las-solicitudes)
- [Proyectos](#proyectos)
- [Git](#git)
- [Colecciones cifradas](#colecciones-cifradas)
- [Copiar como y pegar curl](#copiar-como-y-pegar-curl)
- [Importar desde Postman](#importar-desde-postman)
- [Importar desde Insomnia, OpenAPI y HAR](#importar-desde-insomnia-openapi-y-har)
- [Historial](#historial)
- [Comparar respuestas](#comparar-respuestas)
- [Asistente de IA](#asistente-de-ia)
- [Complementos](#complementos)
- [Paleta de comandos](#paleta-de-comandos)
- [Configuración inicial](#configuración-inicial)
- [Configuración e idioma](#configuración-e-idioma)
- [Atajos de teclado](#atajos-de-teclado)

## Instalar Apix

La página de descargas, **https://11r5.github.io/Apix/**, ofrece la última
versión para el sistema que usas; todos los archivos están también en la
[página de versiones](https://github.com/11R5/Apix/releases). Cada uno incluye
la aplicación de escritorio y la línea de comandos `apix`.

| Sistema | Archivo | Cómo |
|---|---|---|
| Windows 10/11 | `Apix-<version>-windows-x64-setup.exe` | Ejecútalo. Se instala para tu usuario sin permisos de administrador (o para todos, si lo eliges), añade Apix al menú Inicio y puede poner `apix` en el PATH y abrir los archivos `.apix`. `…-windows-x64.zip` es lo mismo sin instalación. |
| macOS 11+ | `Apix-<version>-macos-universal.dmg` | Ábrelo y arrastra Apix a Aplicaciones; una sola app para Apple silicon e Intel. La línea de comandos es `/Applications/Apix.app/Contents/MacOS/apix`. O bien `brew install --cask 11r5/apix/apix`, que además pone `apix` en el PATH. |
| Debian, Ubuntu, Mint, … | `apix_<version>_amd64.deb` | `sudo apt install ./apix_<version>_amd64.deb` |
| Arch, Manjaro, EndeavourOS, … | `apix-<version>-1-x86_64.pkg.tar.zst` | `sudo pacman -U ./apix-<version>-1-x86_64.pkg.tar.zst`, o `makepkg -si` con el `PKGBUILD` de la versión |
| Otro Linux (glibc 2.35+) | `apix-<version>-linux-x64.tar.gz` | Descomprímelo y ejecuta `./install.sh` (para tu usuario; `--system` para todos, `--uninstall` para quitarlo) |
| Cualquier Linux, un solo archivo | `Apix-<version>-x86_64.AppImage` | Dale permiso con `chmod +x` y ejecútalo; `Apix-….AppImage cli …` ejecuta la línea de comandos |

Las compilaciones no están firmadas (firmar cuesta dinero cada año, así que de
momento no está previsto). En Windows, SmartScreen puede detener el
instalador: **Más información › Ejecutar de todas formas**. En macOS, abre Apix
la primera vez con clic derecho › **Abrir**, o ejecuta `xattr -dr com.apple.quarantine
/Applications/Apix.app`. En Linux, Apix dibuja con Vulkan, así que el
controlador gráfico tiene que admitir Vulkan (Mesa lo ofrece en la mayoría de
los sistemas). `SHA256SUMS.txt` en cada versión lista las sumas de comprobación
de los archivos.

## Abrir una colección

Una **colección** es una carpeta que contiene un archivo `apix.toml` y tus
solicitudes (archivos `.apix`), opcionalmente en subcarpetas. Abre una con
**Archivo › Abrir…** (Ctrl+O), o empieza una nueva con **Archivo › Nueva
colección…**. Si eliges una carpeta que todavía no es una colección, Apix se
ofrece a convertirla en una.

Las colecciones abiertas recientemente están en **Archivo › Abrir recientes**.
Si inicias la aplicación con una carpeta (`apix-desktop path/to/collection`),
la abre directamente.

## La ventana

- **Barra lateral (izquierda):** las carpetas y solicitudes de la colección,
  leídas de los archivos en disco y actualizadas cuando cambian. El selector
  bajo el nombre de la colección elige el entorno activo.
- **Pestañas (arriba):** cada solicitud que abres tiene su pestaña.
- **Panel de la solicitud:** el método, la URL y las pestañas Parámetros,
  Encabezados, Autenticación, Cuerpo y Ajustes.
- **Panel de la respuesta (derecha):** el estado, los tiempos, el tamaño y el
  cuerpo de la respuesta.
- **Barra de estado (abajo):** la colección, el archivo de la solicitud abierta
  y los atajos más útiles.
- **Barra de menús:** Archivo, Solicitud, Entorno, Ver y Ayuda contienen todos
  los comandos.

## Enviar una solicitud

1. Elige el método a la izquierda de la barra de URL (o pulsa Alt+1…7 para GET,
   POST, PUT, PATCH, DELETE, HEAD, OPTIONS).
2. Escribe la URL. Se admiten `{{variables}}` en cualquier parte (consulta
   [Variables](#variables-y-entornos)).
3. Pulsa **Enviar** o Ctrl+Intro. Esc cancela una solicitud que aún está en
   curso.

Mientras está activo un entorno cuyo nombre contiene «prod», Apix pregunta antes
de enviar DELETE, PUT o PATCH y muestra la URL completa a la que llamaría.
**Configuración › Confirmar en producción** lo desactiva.

**Nueva solicitud** (Ctrl+N) abre una solicitud GET; el menú **+** de la barra
lateral ofrece todos los métodos, y POST, PUT y PATCH empiezan con un cuerpo
JSON. Guarda con Ctrl+S. Un punto en la pestaña y en la barra lateral marca los
cambios sin guardar.

## Parámetros y encabezados

**Parámetros** es la cadena de consulta de la URL; **Encabezados** se envían tal
cual. Desmarca una fila para conservarla en el archivo sin enviarla. Una fila
sin clave se ignora. `Content-Type` se define según el tipo de cuerpo, salvo que
añadas uno propio. Las `{{variables}}` en claves y valores se muestran como
chips, igual que en la barra de URL; pasa el puntero por encima de una para ver
su valor.

La barra de URL y la tabla de Parámetros van a la par: escribir
`?page=2&sort=name` en la URL rellena la tabla, y editar, añadir, desmarcar o
eliminar una fila reescribe la URL. Los valores se muestran decodificados
(`%20` se convierte en un espacio) y se codifican al enviar la solicitud. El
archivo guarda la URL sin su consulta, y las filas como `[[query]]`.

**Variables de ruta.** Un segmento de ruta escrito como `:name`, como en
`{{base}}/users/:id/posts`, tiene una fila en **Variables de ruta** en la
pestaña Parámetros. Su valor sustituye al segmento al enviar la solicitud,
codificado como un único segmento (`a b/c` se convierte en `a%20b%2Fc`), y las
`{{variables}}` funcionan en él. Una variable que se deja vacía se envía tal
como está escrita. Los valores se guardan como `[[path]]` en el archivo, y las
colecciones de Postman traen los suyos (y los recuperan al exportar).

![Parámetros y variables de ruta](../en/images/path-variables.png)

**Sugerencias de encabezados.** Mientras escribes el nombre de un encabezado,
aparecen bajo la fila los nombres habituales que encajan (primero los que
empiezan por lo escrito, y ninguno que ya esté en la tabla); su valor recibe los
valores habituales de ese encabezado, como `application/json` para
`Content-Type`. Intro toma el primero, un clic toma cualquiera, y después de un
nombre sigue el valor.

![Nombres de encabezados sugeridos al escribir](../en/images/header-hints.png)

## Cuerpo

La pestaña Cuerpo ofrece:

| Tipo | Qué se envía |
|---|---|
| Ninguno | Sin cuerpo. |
| JSON, Texto, XML, HTML | El texto tal como está escrito, con resaltado de sintaxis (`application/json`, `text/plain`, `application/xml`, `text/html`, salvo que un encabezado diga otra cosa). |
| Formulario | Filas clave–valor `application/x-www-form-urlencoded`. |
| Multipart | `multipart/form-data`. Cada campo es **Texto** o **Archivo**; el botón de carpeta elige uno o varios archivos. El campo de tipo que hay al lado define el Content-Type propio del campo (como `application/json` o `image/png`); vacío deja que Apix lo elija. |
| Binario | Los bytes de un archivo, como `application/octet-stream` salvo que un encabezado diga otra cosa. Apix muestra el tamaño del archivo, o «Archivo no encontrado». |
| GraphQL | Un editor de consultas y un editor de variables JSON; se envía como JSON `{"query", "variables"}`. |

![Cuerpo multipart](../en/images/body.png)

Las rutas de archivos dentro de la colección se guardan relativas al archivo de
la solicitud, así que la colección sigue funcionando en otro equipo.

### Esquema y autocompletado de GraphQL

**Obtener esquema**, encima de la consulta, pide al servidor su esquema (la
consulta de introspección estándar), con la URL, los encabezados, la
autenticación y el entorno de la solicitud, así que funciona también un esquema
protegido por un token. Después:

- Al escribir en la consulta se sugiere lo que encaja donde está el cursor: los
  campos del tipo que se está seleccionando (con sus tipos), los argumentos de
  un campo después de `(`, los valores de enum y `true`/`false` después de `:`,
  los campos de un objeto de entrada, fragmentos `... on Type` para uniones e
  interfaces, y nombres de tipo después de `on` o en `$variable: Type`. Los
  campos obsoletos van al final. Ctrl+Espacio abre las sugerencias sin escribir.
- Al pasar el puntero sobre un campo, argumento o valor de enum se muestran su
  tipo y su descripción.
- En el editor de variables, las variables que declara la consulta
  (`$role: Role`) se sugieren como claves, y luego los campos de los objetos de
  entrada, los valores de enum (como cadenas JSON) y `true`/`false` para sus
  valores.
- **Mostrar esquema** pone todo el esquema junto a la consulta, en SDL.

![Autocompletado a partir del esquema](../en/images/graphql.png)

Apix guarda el esquema de cada endpoint (en la carpeta de configuración, en
`graphql/`) y lo vuelve a cargar cuando abres una solicitud a esa URL, así que
**Actualizar esquema** solo hace falta cuando cambia la API. Un servidor con la
introspección desactivada responde con un error, que se muestra en lugar del
estado del esquema.

Desde la línea de comandos, `apix schema request.apix` imprime el esquema en SDL
(`--json` imprime la respuesta del servidor tal como llegó).

## Autenticación

La pestaña Autenticación define cómo demuestra la solicitud quién es:

| Tipo | Notas |
|---|---|
| Heredar del nivel superior | El valor por defecto: usar la autenticación de la carpeta, colección o proyecto más cercano que defina una. La pestaña te dice cuál es y de dónde viene. |
| Sin autenticación | No enviar nada, aunque un nivel superior defina autenticación. |
| Token Bearer | `Authorization: Bearer <token>`. |
| Autenticación básica | Nombre de usuario y contraseña. |
| Clave de API | Un encabezado o parámetro de consulta que tú eliges. |
| Autenticación Digest | Apix responde al desafío Digest del servidor (MD5 o SHA-256). |
| OAuth 1.0 | HMAC-SHA1, HMAC-SHA256 o PLAINTEXT; en el encabezado o en la cadena de consulta; un nonce y una marca de tiempo nuevos en cada envío. |
| OAuth 2.0 | Credenciales de cliente, contraseña, o código de autorización con PKCE. Apix obtiene el token por sí mismo y lo envía como token Bearer. |
| AWS Signature | AWS Signature Version 4 con clave de acceso, clave secreta, token de sesión opcional, región y servicio. |

![OAuth 2.0 en la pestaña Autenticación](../en/images/auth.png)

**Tokens de OAuth 2.0.** El primer envío obtiene un token; los envíos
siguientes lo reutilizan y lo renuevan con el token de actualización cuando
caduca. **Obtener nuevo token** obtiene uno ahora y **Olvidar** lo descarta. Los
tokens se guardan solo en memoria y nunca se escriben en disco. Para la
concesión de código de autorización, Apix abre la página de inicio de sesión en
tu navegador y recibe la respuesta en `http://127.0.0.1` (un puerto libre, o la
**URL de redirección** que registraste para el cliente).

**Autenticación compartida.** Haz clic derecho en una carpeta o colección y
elige **Autenticación…** para definir la autenticación una sola vez para todo
lo que contiene; **Entorno › Autenticación del proyecto…** hace lo mismo para
todo un proyecto. Las solicitudes cuya autenticación es *Heredar del nivel
superior* la usan.

## Ajustes de la solicitud

La pestaña Ajustes controla cómo viaja la solicitud:

| Ajuste | Valor por defecto |
|---|---|
| Tiempo límite (segundos) | ninguno: una solicitud dura hasta que termina o la cancelas; `0` también significa sin tiempo límite |
| Seguir redirecciones | activado |
| Redirecciones máx. | 10 |
| Verificar el certificado TLS | activado; desactívalo solo para servidores de prueba de confianza |
| Proxy | del entorno (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`); *Desactivado* los ignora; una URL indica uno de forma explícita |
| Certificado de cliente | ninguno; un archivo PEM, con la clave en el mismo archivo o en otro aparte |
| CA de confianza | solo los certificados raíz habituales; un archivo PEM añade autoridades de certificación propias (de una empresa o autofirmadas), así que el servidor se sigue comprobando |
| Enviar cookies | activado |

Cada campo que se deja vacío (o en *Heredar*) toma el valor de la carpeta,
colección o proyecto más cercano que lo defina y, por último, los valores por
defecto de arriba: la misma cadena que la autenticación. Haz clic derecho en una
carpeta o colección y elige **Ajustes de petición…** para definirlos una sola
vez para todo lo que contiene; **Entorno › Ajustes de petición del proyecto…**
hace lo mismo para todo un proyecto. La línea bajo el formulario muestra lo que
la solicitud acaba usando. Las `{{variables}}` funcionan en los campos de proxy
y de certificados, y las rutas relativas de certificados se resuelven respecto
al archivo que las define.

En un archivo de solicitud (y en `folder.toml`, `apix.toml` y
`apix-project.toml`) los ajustes son una tabla `[settings]`:

```toml
[settings]
timeout_ms = 5000
tls_verify = false
proxy = ""          # "" = sin proxy, aunque el entorno defina uno
client_cert = "certs/client.pem"
ca_cert = "certs/company-ca.pem"
```

**Copiar como** escribe las opciones de curl correspondientes (`-m`,
`-L --max-redirs`, `-k`, `-x`, `--cert`, `--key`, `--cacert`), y al pegar un
comando curl con esas opciones se rellena la pestaña. `apix send` y `apix run`
tienen `--timeout`, `--insecure`, `--proxy` y `--cacert`, que tienen prioridad
sobre el archivo. Estos ajustes se aplican también a las solicitudes gRPC. Las
opciones por solicitud de Postman (`protocolProfileBehavior`) se importan.

## Cookies

Apix tiene un único almacén de cookies: las respuestas `Set-Cookie` se guardan
automáticamente y se envían con las solicitudes posteriores al mismo sitio,
también entre reinicios (`cookies.json` junto a la configuración de la
aplicación; las cookies de sesión desaparecen al cerrar la aplicación).
**Solicitud › Cookies…** las lista, elimina cookies sueltas o vacía el almacén.
**Enviar cookies** en la pestaña Ajustes desactiva el almacén para una
solicitud, una carpeta, una colección o un proyecto. La CLI empieza con un
almacén vacío en cada ejecución.

## Variables y entornos

Escribe `{{name}}` en una URL, un encabezado, un cuerpo o un campo de
autenticación y Apix rellena el valor al enviar. En la barra de URL cada
variable es un chip de color: pasa el puntero por encima para ver su valor y de
dónde viene; un chip rojo no está definido en ninguna parte.

Los valores vienen en capas, y la posterior gana:

1. Variables del **proyecto** (compartidas por todas las colecciones de un
   proyecto)
2. Variables de la **colección** (`apix.toml`)
3. El **entorno** activo (`environments/<name>.toml`)

Abre **Entorno › Variables y entornos…** (Ctrl+E) para editarlas. Marca
**Secreto** para tokens y contraseñas: los valores secretos se guardan en
`environments/<name>.local.toml`, que Apix añade a `.gitignore`, así que nunca
se incluyen en un commit.

Para que tampoco queden en el disco, elige **Configuración › Valores secretos de
los entornos › Llavero del sistema**. Los valores van entonces al almacén de
credenciales del sistema (el Administrador de credenciales en Windows, el
Llavero en macOS, el Secret Service en Linux) y `<name>.local.toml` solo guarda
el id de esa entrada, así que cambiar el nombre del entorno o mover la carpeta
sigue funcionando. La línea de comandos `apix` los lee de la misma manera.
Cambiar el ajuste mueve al momento los secretos de la colección o el proyecto
abiertos; los demás se mueven la próxima vez que los guardes. Los secretos de
una colección cifrada siempre se quedan dentro de ella.

![El diálogo Variables](../en/images/variables.png)

Las **variables dinámicas** obtienen un valor nuevo en cada uso: `{{$guid}}`,
`{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomEmail}}` y
otras; la página **Variables dinámicas** del diálogo Variables las lista todas
con un valor de ejemplo.

## La respuesta

El panel de la respuesta muestra el estado, el tiempo hasta el primer byte y el
total, y el tamaño. **Formateado** da formato al JSON (pliega bloques con las
flechas) y sangra y colorea XML y HTML; **Sin formato** muestra el cuerpo tal
como se recibió, **Encabezados** lista los encabezados de la respuesta y
**Hex** muestra los bytes. Los cuerpos muy grandes (decenas de megabytes) se
abren en un visor rápido que solo dibuja las líneas visibles.

Apix deduce el tipo del cuerpo de su `Content-Type`, o de los bytes cuando el
tipo no dice nada útil. Las imágenes (PNG, JPEG, GIF, WebP, BMP, SVG) se
muestran en **Formateado**; el resto del contenido binario (un PDF, un zip…)
recibe un breve resumen en lugar de texto ilegible. El HTML que omite etiquetas
de cierre opcionales se muestra tal como llegó, sin reformatear.

El botón **↓** al final de la línea de estado guarda el cuerpo, byte a byte, en
un archivo. El nombre sugerido viene del `Content-Disposition` del servidor o
de la URL, con una extensión acorde al contenido.

**Búsqueda.** Ctrl+F en el cuerpo, o la lupa de la línea de estado, busca en
**Formateado**, **Sin formato** y **Hex**, sin distinguir mayúsculas. Intro (o
F3) va a la siguiente coincidencia y Mayús+Intro (Mayús+F3) a la anterior; el
contador muestra dónde estás. El visor rápido de cuerpos muy grandes busca en
todo el cuerpo en segundo plano y deja de contar en 100.000 coincidencias.

**Filtro.** Una respuesta JSON tiene un cuadro de filtro encima de
**Formateado**. Una expresión que empieza por `$` es JSONPath (RFC 9535) y
cualquier otra es jq, así que funcionan las dos:

```text
$.users[?@.active].email
.users[] | select(.active) | {name, email}
```

![Una respuesta JSON filtrada con JSONPath](../en/images/response-filter.png)

El resultado sustituye al cuerpo en **Formateado** mientras el cuadro contiene
una expresión (el contador de al lado dice cuántos valores hay); **Sin formato**
y **Hex** siguen mostrando el cuerpo entero, y ✕ lo recupera. JSONPath da una
lista de sus coincidencias; jq da sus salidas una tras otra. El filtro se
mantiene para la siguiente respuesta de la misma solicitud, así que puedes
enviarla una y otra vez mientras observas un valor.
`apix send --filter '<expression>'` imprime solo lo que selecciona el filtro.

**Tabla y gráfico.** **Texto**, **Tabla** y **Gráfico**, al final de la fila del
filtro, muestran una respuesta JSON (o lo que seleccionó el filtro) como texto,
como una tabla de sus registros o como un gráfico de barras. Los registros son
el array del cuerpo, o el array de objetos más largo que contiene (`$.items`,
`$.data`); cada clave es una columna, y los objetos anidados se muestran como
JSON compacto. El gráfico dibuja una columna numérica (**Valor**) frente a una
columna que nombra las filas (**Etiqueta**); ambas se pueden elegir. Los cuerpos
de más de 8 MB no se convierten en tabla: fíltralos primero.
`apix send --table` imprime la misma tabla en una terminal (con `--filter`, de
lo que selecciona). Las plantillas `pm.visualizer.set` de Postman no se dibujan,
porque Apix no tiene vista web; los scripts que lo llaman reciben una nota en la
consola.

![La vista Tabla de una respuesta filtrada](../en/images/visual-table.png)

![La vista Gráfico](../en/images/visual-chart.png)

**Ejemplos.** El botón de la bandeja en la línea de estado guarda la respuesta
como ejemplo de la solicitud: su estado, encabezados y cuerpo, y la solicitud
de la que vino tal como está escrita (las `{{variables}}` se mantienen, así que
ningún token acaba en el archivo). Las cookies que define el servidor se
omiten, y los cuerpos de más de 2 MB se rechazan. La pestaña **Ejemplos** de la
solicitud los lista: cambia el nombre de uno ahí mismo, ▷ lo muestra en el panel
de la respuesta (marcado como «Ejemplo: …») y la papelera lo quita. Forman parte
de la solicitud, así que Ctrl+S los escribe en su archivo como `[[example]]`, y
las respuestas guardadas de Postman llegan como ejemplos al importar y vuelven
al exportar. `apix send --save-example '<name>'` guarda la respuesta desde la
línea de comandos.

![Un ejemplo mostrado en el panel de la respuesta](../en/images/examples.png)

Cuando el panel es demasiado estrecho para todas las pestañas de la solicitud,
la flecha al final de la barra de pestañas las lista todas.

## La pestaña Red

La pestaña **Red** explica en qué se fue el tiempo y cómo era la conexión:

- una barra de tiempos dividida en DNS, conexión TCP, negociación TLS, espera y
  descarga
- las respuestas DNS, la dirección del servidor y la local, la interfaz de red
  (con un aviso cuando se usa una VPN), la puerta de enlace, los servidores DNS
  y el proxy
- cada redirección
- un cuerpo comprimido: su codificación, el tamaño transferido y el
  descomprimido
- el cifrado: versión de TLS, conjunto de cifrado, intercambio de claves, ALPN
  (`h2` cuando el servidor habla HTTP/2), el nombre de servidor enviado (SNI) y
  cuánto tardó la negociación
- la cadena de certificados tal como la envió el servidor: para cada
  certificado, para quién es y quién lo emitió, la validez y los días que
  quedan, la clave; para el del propio servidor, también sus nombres y su
  huella SHA-256

![La pestaña Red](../en/images/network.png)

![El cifrado y la cadena de certificados](../en/images/network-tls.png)

Todo procede de la propia conexión de la solicitud, no de una segunda conexión
hecha para mirar: cada envío abre una conexión nueva y Apix registra lo que
acordó esa negociación. Apix pide respuestas comprimidas (`gzip`, `deflate`,
`br`) salvo que la solicitud defina su propio `Accept-Encoding`, y muestra los
encabezados `Content-Encoding` y `Content-Length` tal como los envió el
servidor.

### Ruta y mapa

**Trazar la ruta y verla en un mapa…**, bajo la dirección del servidor (o
**Ver › Ruta y mapa…**), lista los routers entre este equipo y el servidor, con
el tiempo hasta cada uno, y los dibuja en un mapa del mundo. Los saltos vienen
de la herramienta propia del sistema (`tracert` en Windows, `traceroute` o
`tracepath` en los demás), así que no hacen falta permisos de administrador; un
router que no responde aparece como `*`. Las direcciones de tu propia red se
marcan como tales.

La ubicación de cada router viene de una base de datos de ubicación IP en este
equipo; no se consulta nada en línea. Sin ella, la lista sigue funcionando y el
mapa queda vacío: **Descargar DB-IP Lite** descarga la base de datos gratuita de
DB-IP (unos 130 MB, CC BY 4.0) en `<config>/Apix/geo` (la configuración inicial
y `apix geo download` hacen lo mismo), o puedes poner ahí cualquier base de
datos de ciudades `.mmdb` (DB-IP Lite, MaxMind GeoLite2 City). Estas bases de
datos sitúan una dirección, como mucho, en una ciudad, y a veces se equivocan
con los routers troncales. En una terminal, `apix trace <url or host>` imprime
la misma lista.

![La ruta en un mapa](../en/images/route.png)

La captura usa una pequeña base de datos de demostración (`cargo run -p apix-route
--example demo_geo`), no consultas reales.

## WebSocket y eventos enviados por el servidor

**WebSocket.** Una solicitud cuya URL empieza por `ws://` o `wss://` es un
WebSocket: **Enviar** pasa a ser **Conectar**, y la pestaña Cuerpo pasa a ser
**Mensajes**. La negociación usa los parámetros, encabezados y autenticación de
la solicitud (Bearer, básica, clave de API, OAuth 2.0 …), como cualquier
solicitud.

![Una conexión WebSocket y su pestaña Mensajes](../en/images/websocket.png)

- Escribe un mensaje y pulsa **Enviar** (o Ctrl+Intro). Las `{{variables}}` se
  rellenan al enviarlo. Sin conexión, Apix conecta primero y lo envía en cuanto
  la conexión está abierta.
- **Guardar mensaje** lo guarda en el archivo de la solicitud como
  `[[message]]`; Ctrl+S guarda el archivo. Haz clic en un mensaje guardado para
  editarlo, o en ▷ para enviarlo tal cual.
- El panel de la respuesta es un registro: ↑ enviado, ↓ recibido, con el tiempo
  desde la conexión, además de cuándo se conectó y cómo se cerró (código y
  motivo). Sigue los mensajes nuevos salvo que te desplaces hacia arriba.
- **Desconectar** (o Esc) cierra la conexión; el registro se queda hasta que
  vuelvas a conectar. Una conexión sigue funcionando mientras su pestaña está en
  segundo plano.
- **Mensajes binarios:** elige **Base64** o **Hex** junto a los botones y
  escribe los bytes de esa forma (`01 02 ff`; se admiten espacios y `0x`); un
  mensaje que no se puede leer como bytes no se puede enviar. Los mensajes
  guardados conservan su formato. Los mensajes binarios aparecen en el registro
  con su tamaño y sus primeros bytes.
- Los **Subprotocolos** (arriba en la pestaña Mensajes, separados por comas) se
  ofrecen en la negociación; el registro indica el que eligió el servidor. Un
  servidor que no elige ninguno se rechaza, como hacen los navegadores.
- **Ping cada (s)**, debajo, envía un ping con ese intervalo mientras hay
  conexión (`[websocket] ping_interval` en el archivo). Cada respuesta aparece
  en el registro como `pong tras 2 ms`; un ping aún sin respuesta cuando toca el
  siguiente cierra la conexión con un error, así que una conexión que murió en
  silencio se muestra como muerta. Los pings del propio servidor siempre se
  responden. `apix send --ping 30` hace lo mismo (`--ping 0` desactiva un
  intervalo guardado) y termina con un código distinto de cero si la conexión
  está muerta.
- `wss://` sigue los ajustes de la solicitud: una CA de confianza, un
  certificado de cliente, o «Verificar el certificado TLS» desactivado.

**Eventos enviados por el servidor.** Cuando una respuesta es
`text/event-stream`, sus eventos aparecen a medida que llegan, con la hora, el
tipo y el id del evento, y los datos. **Detener** (o Esc) termina el flujo y
conserva lo recibido; un flujo que termina el servidor muestra la misma lista en
la pestaña Formateado, con Sin formato, Encabezados, Red y Hex como en cualquier
respuesta. Envía `Accept: text/event-stream` si el servidor lo necesita. Un
tiempo límite de la solicitud en Ajustes también termina un flujo, así que
déjalo vacío para flujos que permanecen abiertos.

- Con más de un tipo de evento, los tipos aparecen encima de la lista con
  cuántos llegaron de cada uno; haz clic en uno para mostrar solo esos, y en
  **Todos** para todos los eventos.
- **Reconectar** (en la vista en vivo) retoma el flujo cuando termina o se cae
  la conexión: tras el tiempo `retry:` del servidor (3 s si no dio ninguno), con
  `Last-Event-ID` igual al último id recibido, como hace un navegador. La lista
  anota cada reconexión. Una respuesta que no es un flujo de eventos lo detiene,
  y también Esc.

Desde la línea de comandos, `apix send` imprime los eventos según llegan
(`--event TYPE`, repetible, imprime solo esos; `--reconnect` sigue
reconectando hasta Ctrl+C). Para un WebSocket, conecta, envía los mensajes
guardados (o los textos de `--message`, leídos como `--format text|base64|hex`),
imprime ambas direcciones y cierra tras `--listen` segundos (3 por defecto; `0`
espera al servidor). `apix run` omite las solicitudes WebSocket.

## Socket.IO

Una solicitud Socket.IO habla con servidores de Socket.IO 3 y 4 sobre
WebSocket. Abre la pestaña **Mensajes** de una solicitud (dale una URL `ws://`
para que aparezca la pestaña) y elige **Socket.IO** arriba; la URL puede ser
entonces `http://`, `https://`, `ws://` o `wss://`, como en un cliente
Socket.IO. El archivo de la solicitud recibe una tabla `[socketio]`, y una
solicitud Socket.IO se abre en su pestaña Mensajes.

![Una conexión Socket.IO: eventos, acks y un adjunto binario](../en/images/socketio.png)

- **Espacio de nombres:** al que unirse, como `/chat`. Si se deja vacío, es la
  ruta de la URL (`http://localhost:3000/chat` se une a `/chat`), o `/`.
  **Ruta** es donde escucha el servidor, `/socket.io/` salvo que el servidor la
  haya cambiado. Una dirección copiada de la pestaña de red del navegador
  (`…/socket.io/?EIO=4&transport=websocket`) funciona tal cual.
- **Autenticación:** un objeto JSON que se envía al unirse (la opción `auth`
  del cliente), con las `{{variables}}` rellenadas. Cuando el servidor rechaza,
  el motivo que dio se muestra como error.
- **Evento** indica como qué se emite un mensaje (`message` si está vacío). El
  texto es un argumento: el JSON tal cual, cualquier otro texto como cadena, y
  nada en absoluto si está vacío. **Base64** y **Hex** envían los bytes como un
  adjunto binario. **Pedir confirmación (ack)** pide al servidor que confirme.
  Los mensajes guardados conservan su evento y su ack.
- El registro muestra cada evento con su nombre: ↑ emitido (con `ack 0`,
  `ack 1` … cuando se pidió un ack), ↓ recibido (con `espera ack 9` cuando el
  servidor pide uno), y ✓ para los acks del servidor con lo que respondió. Los
  adjuntos binarios se muestran con su tamaño y sus primeros bytes. Los
  encabezados, parámetros y autenticación de la negociación y los ajustes de TLS
  funcionan como en un WebSocket.
- Apix responde a los pings del servidor. Cuando el servidor hace que el
  cliente salga del espacio de nombres, el registro lo dice y la conexión se
  cierra; **Desconectar** (o Esc) sale del espacio de nombres y cierra.

`apix send` se une, emite los mensajes guardados e imprime los eventos, acks y
adjuntos binarios según llegan; `--message TEXT` (repetible) emite textos en su
lugar, como el evento que indica `--event NAME` (`message` por defecto).
`apix run` omite las solicitudes Socket.IO. Los adjuntos recibidos se escriben
como `{"$binary": "<base64>"}`. Todavía no: servidores Socket.IO 2 (Engine.IO
3), el transporte long-polling y responder a un ack que pide el servidor.

## MQTT

Una solicitud cuya URL empieza por `mqtt://` (o `mqtts://` para TLS) se conecta
a un broker MQTT 3.1.1: la barra de URL y la barra lateral muestran **MQTT**,
**Enviar** pasa a ser **Conectar**, y la pestaña Cuerpo pasa a ser
**Mensajes**. El puerto es 1883, u 8883 con TLS, salvo que la URL indique uno.

![Una conexión MQTT: suscripciones, mensajes publicados y recibidos](../en/images/mqtt.png)

- **Inicio de sesión:** el nombre de usuario y la contraseña del tipo
  Autenticación básica de la pestaña Autenticación, o de la URL
  (`mqtt://user:password@host`). Los demás tipos de autenticación no se aplican
  a MQTT. Cuando el broker rechaza, dice por qué. Una contraseña en la URL se
  omite de lo que imprime Apix.
- **Id. de cliente** es nuevo en cada conexión si se deja vacío. **Mantener
  sesión** pide al broker que conserve la sesión entre conexiones, lo que
  requiere un id de cliente; el registro indica cuándo el broker aún tenía una.
  **Keep alive** son los segundos entre pings en reposo (60 por defecto, 0 para
  ninguno).
- Las **Suscripciones** se hacen al conectar, cada una con su calidad de
  servicio; `+` y `#` funcionan en los temas. Con la conexión abierta, ▷
  suscribe una fila ahora y ✕ la abandona. El registro anota lo que el broker
  aceptó o rechazó.
- Para publicar, rellena el **Tema**, elige la calidad de servicio, marca
  **Retener** si el broker debe guardar el mensaje para suscriptores
  posteriores, y pulsa **Enviar** (o Ctrl+Intro). Las `{{variables}}` se
  rellenan. **Base64** y **Hex** envían bytes. Se admite un mensaje vacío: con
  Retener, borra lo que guardaba el broker. **Guardar mensaje** lo guarda en el
  archivo de la solicitud.
- El registro muestra cada mensaje bajo su tema: ↑ publicado, ↓ recibido, con
  su calidad de servicio y su indicador de retención, y ✓ cuando el broker
  confirmó un mensaje de QoS 1 o 2. Las cargas que no son texto se muestran con
  su tamaño y sus primeros bytes.
- `mqtts://` sigue los ajustes de la solicitud: una CA de confianza, un
  certificado de cliente, o «Verificar el certificado TLS» desactivado. El
  tiempo límite de la solicitud limita la conexión (10 s si no hay ninguno).

`apix send` conecta, se suscribe, publica los mensajes guardados e imprime lo
que va en ambos sentidos, y cierra tras `--listen` segundos (3 por defecto; `0`
espera al broker). `--subscribe TOPIC` (repetible) añade suscripciones;
`--message TEXT` con `--topic TOPIC` publica en lugar de los mensajes guardados;
`--qos` y `--retain` se aplican a estos. El código de salida es 1 cuando el
broker rechazó una suscripción, o colgó sin responder a una o sin confirmar un
mensaje. `apix run` omite las solicitudes MQTT. Todavía no: MQTT 5, el último
mensaje (last will) y MQTT sobre WebSocket.

## SOAP

Una llamada SOAP es un sobre XML enviado con POST, así que una solicitud SOAP es
una solicitud normal: los scripts, las pruebas, las ejecuciones y el historial
funcionan con ella como con cualquier otra.

**Desde un WSDL.** Importa un archivo WSDL 1.1 con **Archivo › Importar…**, o
su dirección con **Archivo › Importar desde URL…**
(`http://host/service?wsdl`). Obtienes una colección con una solicitud por
operación, y una carpeta por puerto cuando hay varios, normalmente uno para
SOAP 1.1 y otro para 1.2. La dirección del puerto es la variable `baseUrl` (una
variable por puerto cuando son distintas), y también se leen los documentos que
importa el WSDL. Lo que no se pudo leer, y los puertos que no son SOAP, se
listan al terminar la importación.

![Una solicitud SOAP a partir de un WSDL, con el fault que respondió el servidor](../en/images/soap.png)

- Los **encabezados** siguen la versión: SOAP 1.1 envía
  `Content-Type: text/xml` y `SOAPAction`; SOAP 1.2 envía
  `application/soap+xml` con la acción dentro.
- **El cuerpo** es un sobre con valores de ejemplo construidos a partir del
  esquema: `1` para los números, `string` para el texto, el primer valor de una
  enumeración, la primera opción de un choice. Los comentarios dicen qué es
  opcional o se repite y cuáles son los demás valores. Los atributos opcionales
  se omiten. Sustituye los valores, o pon `{{variables}}` en ellos, y borra lo
  que no necesites.
- Un tipo que se contiene a sí mismo se detiene en el segundo nivel, con un
  comentario.

**Sin un WSDL.** Elige **XML** como cuerpo; mientras está vacío, **SOAP 1.1** y
**SOAP 1.2**, a su lado, inician un sobre y definen los encabezados. Rellena la
operación y la acción (`SOAPAction`, o `action=` en el `Content-Type`).

**Faults.** Cuando una respuesta contiene un fault SOAP, una línea roja encima
de la respuesta muestra su código y su mensaje, y `apix send` lo imprime. Una
solicitud sin pruebas falla ante un fault aunque el estado sea 200, como lo
envían algunos servidores, en `apix send`, `apix run` y el Runner. Los scripts
de prueba deciden por sí mismos.

Todavía no: WSDL 2.0, que se rechaza indicando ese motivo, WS-Security, los
adjuntos MTOM, y `xsi:type` en los ejemplos para la codificación SOAP, sobre lo
que avisa la importación.

## gRPC

Una solicitud cuya URL empieza por `grpc://` (o `grpcs://` para TLS) llama a un
método gRPC: la barra de URL muestra **gRPC**, y la pestaña Cuerpo pasa a ser
**Mensaje**.

![Una llamada gRPC](../en/images/grpc.png)

1. Escribe la dirección, como `grpc://localhost:50051`.
2. **Cargar métodos** pregunta al servidor qué ofrece (reflexión de servidor
   gRPC). Si el servidor no tiene reflexión, **Añadir archivo** elige los
   archivos `.proto` o descriptor sets del servicio (`protoc
   --descriptor_set_out`, `buf build`, binarios o JSON); los imports se buscan
   junto a ellos, y los tipos conocidos de Google vienen incluidos. Nada
   necesita `protoc`. Si no tienes ese archivo, consulta
   [Sin archivo .proto](#sin-archivo-proto).
3. Elige el método. Su tipo se muestra al lado: unario, flujo del servidor,
   flujo del cliente o bidireccional.
4. Escribe el mensaje en JSON, o pulsa **Mensaje de ejemplo** para obtener uno
   con todos los campos. Para un método que recibe un flujo, una lista JSON
   envía un mensaje por elemento.
5. **Enviar** (Ctrl+Intro). El panel de la respuesta lista lo que salió (↑) y lo
   que volvió (↓) con sus tiempos, y luego el estado (`0 OK`, o un código como
   `5 NOT_FOUND` con su mensaje), los metadatos de la respuesta y los trailers.
   Un flujo muestra sus mensajes según llegan; Esc lo detiene.

Los encabezados se envían como metadatos, y la autenticación funciona como en
HTTP con Bearer, básica, una clave de API en un encabezado y OAuth 2.0. Se
aplican el tiempo límite, el certificado de cliente, la CA de confianza y
«Verificar el certificado TLS» de los ajustes de la solicitud (si se desactiva,
no se comprueba el certificado de un servidor `grpcs://`). Un encabezado `-bin`
se lee como base64.

Desde la línea de comandos, `apix send` llama al método (los flujos se imprimen
con un mensaje por línea según llegan; el código de salida es distinto de cero
salvo que el estado sea OK), y `apix schema` imprime el servicio y sus mensajes.

**En ejecuciones y pruebas.** `apix run` y el Runner también llaman a las
solicitudes gRPC, y sus scripts de prueba y comprobaciones ven la llamada como
una respuesta HTTP: el estado es el estado HTTP que corresponde al código gRPC
(`OK` es 200, `INVALID_ARGUMENT` 400, `NOT_FOUND` 404, `UNAUTHENTICATED` 401,
…), el código y el mensaje gRPC están en los encabezados `grpc-status` y
`grpc-message` junto a los metadatos y los trailers, y el cuerpo es la
respuesta, o una lista JSON de los mensajes para un método que responde con un
flujo. Así, `pm.response.json().message` o una comprobación sobre
`body.message` funcionan como en cualquier solicitud. Un **Enviar** suelto
muestra la llamada sin ejecutar sus scripts.

Cada llamada se guarda en el **Historial** (Ctrl+H) con su registro, su estado y
sus metadatos; al elegirla se muestra de nuevo la llamada tal como terminó.

### Sin archivo .proto

En la red, gRPC envía números de campo, no nombres, y llama a
`package.Service/Method`. Así que, partas de lo que partas, eso tiene que acabar
siendo correcto. La pestaña **Definición**, junto a **Mensaje**, crea una
definición a partir de lo que tengas. Pégalo (o usa **Leer un archivo** o
**Usar el mensaje**), indica el **Método a llamar** si la fuente no lo dice, y
pulsa **Convertir**. El resultado es texto `.proto` que puedes editar, guardado
en la solicitud; **Guardar como archivo** lo escribe en disco.

![Una definición creada a partir de un ejemplo JSON, y el aviso cuando no coincide](../en/images/grpc-definition.png)

| Lo que tienes | Lo que se obtiene |
|---|---|
| Texto `.proto`, un descriptor set (binario o JSON) | Exacta. |
| Código generado que contiene la descripción: `_pb2.py`, la clase externa de Java, Kotlin a través de ella, `.cs`, `.pb.cc`, `.pb.go`, `_pb.rb`, `GPBMetadata` de PHP, `_pb.ts` de protobuf-es | Exacta: se lee la descripción que contiene, sea cual sea el lenguaje. |
| Código con números pero sin descripción: etiquetas `protobuf:"…"` de Go, `#[prost(…)]` de Rust, ts-proto, betterproto, `[ProtoMember(n)]`, `@ProtoNumber(n)` | Los números y los tipos son correctos; comprueba las llamadas cuando la fuente no las lista. |
| Tipos escritos a mano: TypeScript, Go, Rust, C#, Java, Kotlin, Python, Swift, Dart, C++ | **Una estimación.** Los campos se numeran por orden de aparición. |
| Un mensaje de ejemplo en JSON (el segundo cuadro, opcional, recibe la respuesta) | **Una estimación.** Los tipos salen de los valores; una lista JSON crea un flujo. |

Una estimación solo es tan buena como sus números, y **los números incorrectos
no fallan: rellenan los campos equivocados.** Por eso Apix nunca lo oculta:

- Si el servidor ofrece reflexión, se usa su descripción en lugar de una
  estimación, y se lista cada diferencia (`number 1 here, 7 on the server`).
- Si no la ofrece, cada llamada dice que los números no están comprobados. Los
  campos de la respuesta que la definición no conoce se muestran con sus
  números y valores (`1:"o-1"`), que es también la forma de descifrar una
  respuesta para la que no tienes tipo. Una respuesta que no encaja en absoluto
  falla con una línea que dice que la causa probable es la definición.

Otros archivos que genera `protoc` solo usan la descripción (`.pb.h`, los
builders de Kotlin, `.pyi`), y los stubs de gRPC solo contienen las llamadas;
Apix dice qué archivo usar en su lugar. No se leen: las ediciones de Protobuf
como texto `.proto` (siguen siendo un descriptor set), los oneofs a partir de
etiquetas o tipos (sus campos pasan a ser campos normales, que viajan igual por
la red) y los genéricos.

Desde la línea de comandos:

```bash
apix proto service_pb2.py
```

```bash
apix proto order.json --method shop.v1.Orders/Create --response answer.json --out orders.proto
```

```bash
apix proto types.ts --method shop.v1.Orders/Create --into create.apix
```

El `.proto` va a la salida estándar o a `--out`, `--into` lo guarda en una
solicitud, y lo que hubo que elegir va a la salida de error estándar.
`apix send`, `apix run` y `apix schema` usan una definición de la misma manera,
y `apix send` imprime los mismos avisos; el informe de una ejecución no los
incluye.

## Scripts y pruebas

La pestaña **Scripts** contiene dos scripts de JavaScript para la solicitud:

- **Antes de la petición** se ejecuta antes de enviar la solicitud. Puede
  cambiar la solicitud (`pm.request.headers.upsert(…)`,
  `pm.request.url.query.add(…)`, `pm.request.body.raw = …`) y definir variables
  (`pm.environment.set("token", …)`).
- **Pruebas** se ejecuta cuando llega la respuesta y la comprueba:

```js
pm.test("status is 200", () => pm.response.to.have.status(200));
pm.test("has users", () => {
  const body = pm.response.json();
  pm.expect(body.users).to.be.an("array").that.is.not.empty;
});
```

La API es la `pm.*` de Postman (también disponible como `apix.*`), así que los
scripts importados de Postman funcionan tal cual: `pm.environment`,
`pm.collectionVariables`, `pm.globals` (las variables del proyecto),
`pm.variables`, `pm.request`, `pm.response`, `pm.test`, `pm.expect` con
aserciones al estilo de Chai, `pm.info`, `console.log`, y los antiguos
`tests["…"] = …` y `postman.setEnvironmentVariable(…)`.

`pm.sendRequest` envía otra solicitud desde un script, por ejemplo para obtener
antes un token; funcionan tanto `pm.sendRequest(req, (err, res) => …)` como
`const res = await pm.sendRequest(req)`. `require(…)` ofrece las partes de
`crypto-js` (MD5, SHA-1, SHA-2 y sus HMAC, codificadores Base64/Hex/UTF-8),
`uuid`, `lodash` (también `_`), `moment`, `ajv`, `tv4` y `chai` que usan los
scripts de API; vienen incluidas, así que no se descarga nada. Una función que
una biblioteca no tiene falla con un mensaje claro.

La tercera parte de la pestaña Scripts, **Comprobaciones**, prueba la respuesta
sin código: cada fila compara un objetivo con un valor. Los objetivos son
`status`, `time` (ms), `size` (bytes), `body`, una ruta en un cuerpo JSON como
`body.users[0].id`, o `header.Content-Type`; las comparaciones son `==`, `!=`,
`>`, `>=`, `<`, `<=`, `contains`, `not contains`, `exists`, `not exists`,
`matches` (una expresión regular como `/^ok/i`) e `is` (un tipo JSON). Los
valores pueden usar `{{variables}}`. Las comprobaciones se guardan en el archivo
de la solicitud como `[[assert]]` y aparecen como pruebas, en la aplicación y en
`apix run`.

Para guardar un valor de una respuesta, usa el botón **{x}** junto al estado:
indica el valor (los mismos objetivos que en Comprobaciones) y un nombre de
variable, y se guarda en el entorno activo (o, si no hay ninguno, en las
variables de la colección). Marca **Actualizarla en cada envío** para añadir
además la línea `pm.environment.set(…)` correspondiente al script de Pruebas.

Los scripts son archivos junto a aquello a lo que pertenecen:
`<request>.prerequest.js` y `<request>.test.js` junto a la solicitud,
`_folder.prerequest.js` / `_folder.test.js` en una carpeta,
`_collection.prerequest.js` / `_collection.test.js` en la raíz de la colección.
Se ejecutan de fuera hacia dentro: colección, carpetas y luego la solicitud.
Ctrl+S guarda los scripts de la solicitud junto con ella; el texto del editor se
usa incluso antes de guardar.

Después de enviar, el panel de la respuesta recibe una pestaña **Pruebas** (✓
correcta, ✗ fallida con el motivo) y una pestaña **Consola** con la salida de
`console.*`. Si un script previo lanza un error, la solicitud no se envía y el
error dice qué archivo falló. Los scripts se detienen tras 10 segundos.

Los valores que define un script no se escriben en ningún archivo. Duran
mientras Apix está abierto (cambiar de entorno descarta lo que definió
`pm.environment.set`) y aparecen en las tarjetas de variables como «Definida
por un script». `pm.variables.set` dura un envío.

## Runner

El **Runner** envía muchas solicitudes de una vez y las comprueba todas. Ábrelo
con **Ver › Runner…** o Ctrl+Mayús+R para la colección en la que estás, o con
**Ejecutar…** sobre una colección o carpeta de la barra lateral para solo esa
parte.

![El Runner tras una ejecución basada en datos](../en/images/runner.png)

- **Peticiones**: todas las solicitudes en el orden de la barra lateral,
  marcadas. Desmarca las que quieras dejar fuera, y mueve una con ↑ ↓ para
  cambiar el orden en esta ejecución.
- **Entorno**: las variables de qué entorno usar.
- **Iteraciones**: cuántas veces recorrer las solicitudes. Si se deja vacío, es
  una vez, o una vez por fila cuando hay un archivo de datos.
- **Espera (ms)**: una pausa entre dos solicitudes, para API con límite de
  frecuencia.
- **A la vez**: cuántas iteraciones se ejecutan en paralelo, como varios
  usuarios de la API al mismo tiempo. Cada una envía sus solicitudes en orden
  (así el token de un inicio de sesión sigue llegando a la siguiente solicitud)
  y conserva lo que definen sus propios scripts; los resultados se listan por
  iteración. Si se deja vacío, una tras otra.
- **Archivo de datos**: un archivo CSV o JSON para una ejecución basada en
  datos. Cada fila es una iteración y sus columnas son variables: `{{email}}` en
  la solicitud y `pm.iterationData.get("email")` en los scripts. CSV necesita
  una línea de encabezado; se entienden las comillas y los archivos separados
  por punto y coma (Excel en muchos idiomas). JSON es un array de objetos, como
  `[{"id": 1, "email": "a@x.test"}, …]`.
- **Parar en el primer fallo**: termina la ejecución en la primera solicitud que
  falla.

Una solicitud es correcta cuando pasan sus pruebas (scripts y Comprobaciones);
una solicitud sin pruebas es correcta con un estado 2xx o 3xx. Mientras se
ejecuta puedes **Pausar**, **Continuar** y **Detener** (tras la solicitud que se
está enviando). Cerrar el diálogo no detiene la ejecución: abre de nuevo el
Runner para verla. Haz clic en un resultado para ver sus pruebas, por qué no se
envió y sus líneas de consola. `pm.execution
.setNextRequest("name")` salta a otra solicitud de la ejecución y
`setNextRequest(null)` termina la iteración, como en Postman.

El Runner envía la versión guardada de cada solicitud. Se usan los valores que
los scripts definieron antes de la ejecución; lo que definen los scripts de la
ejecución se queda dentro de ella.

Al terminar la ejecución, **Guardar informe** la escribe como **JUnit XML**
(para las vistas de pruebas de CI), **JSON** (todo lo que ocurrió) o **HTML**
(una página que se abre en cualquier navegador, clara u oscura). `apix run` hace
lo mismo desde una terminal; consulta
[Ejecutar una colección en CI](workflows.md#ejecutar-una-colección-en-ci).
Una contraseña escrita en una URL (`https://user:password@host`) se omite de los
informes, la salida de la línea de comandos, los registros y los mensajes de
error. El archivo de la solicitud y la barra de URL la siguen conteniendo, así
que para contraseñas reales es mejor una variable o la pestaña Autenticación.

## Flujos

Un **flujo** une solicitudes en un grafo: iniciar sesión, guardar el token,
consultar hasta que un trabajo termine, ramificar según lo que volvió. Cada
flujo es un archivo `.flow` en la colección, junto a las solicitudes, así que va
a git con ellas. Abre **Ver › Flujos…**; escribe un nombre y pulsa **Crear**
para uno nuevo.

![La ventana Flujos](../en/images/flows.png)

Hay cuatro tipos de nodo:

- **Petición** envía una solicitud de la colección, con sus scripts y pruebas.
  Su resultado es **ok** cuando pasó (un estado correcto, o todas las pruebas
  superadas) y **fail** en caso contrario. **Guardar de la respuesta** convierte
  valores de la respuesta en variables para los nodos siguientes, uno por
  línea: `token = $.token`.
- **Comprobación** compara un sujeto con un valor: **yes** o **no**. El sujeto
  es `status`, `time` (ms), `body`, `header:Name`, un JSONPath (`$.items[0].id`)
  o un filtro jq (`.items | length`) sobre la última respuesta, o un texto con
  `{{variables}}`. Comparaciones: igual a, distinto de, contiene, mayor que,
  menor que (como números cuando ambos lados lo son), existe, falta.
- **Asignar variables** define variables (líneas `name = value`; las
  `{{variables}}` funcionan).
- **Esperar** hace una pausa, para bucles de consulta.

Arrastra un nodo para moverlo. Para conectar, haz clic en el punto de un
resultado, a la derecha de un nodo, y luego en el nodo al que lleva; hacer clic
de nuevo en el punto lo redirige, y **×** en el panel lo quita. El panel de la
derecha edita el nodo seleccionado: su id, sus ajustes, adónde lleva cada
resultado, **Empezar aquí**, **Eliminar nodo**. Una ejecución empieza en el nodo
**Inicio** (si no lo hay, en el primer nodo al que no lleva nada), sigue cada
resultado y termina en un resultado que no lleva a ninguna parte. Falla cuando
una solicitud falla sin un enlace **fail**, o cuando no está un valor que había
que guardar. Se permiten bucles; una ejecución se detiene tras 1000 pasos.

**Ejecutar** envía el flujo con las variables de la colección y el entorno
elegido; cada nodo se pone verde, amarillo o rojo según su resultado, y la lista
bajo el lienzo muestra cada paso con lo que se guardó o comparó. Los cambios se
guardan con **Guardar** y al cerrar la ventana.

Desde una terminal, el mismo flujo se ejecuta en CI:

```sh
apix flow list my-api
apix flow run my-api/new-user.flow --env staging --junit flow.xml
```

`--junit`, `--html` y `--markdown` informan de las solicitudes del flujo como
`apix run`; `--json` escribe cada paso. La colección de ejemplo tiene
`new-user.flow`.

Un archivo de flujo:

```toml
name = "New user"

[[node]]
id = "first-user"
kind = "request"
request = "users/list-users.apix"
x = 40.0
y = 40.0
extract = { user_id = "$[0].id" }

[[node]]
id = "found"
kind = "check"
subject = "{{user_id}}"
op = "exists"
x = 300.0
y = 40.0

[[edge]]
from = "first-user"
port = "ok"
to = "found"
```

## Pruebas de carga

**Solicitud › Prueba de carga…** (o la paleta de comandos) envía la solicitud
abierta desde muchos trabajadores a la vez y muestra cómo aguanta el servidor:
las solicitudes y las solicitudes por segundo, los fallos, los percentiles 50,
95 y 99 y el tiempo más lento, y un gráfico segundo a segundo de las
solicitudes (las fallidas en rojo) con la latencia media como una marca
amarilla.

![Una prueba de carga](../en/images/load-test.png)

- **A la vez**: solicitudes en curso al mismo tiempo (de 1 a 10 000).
- **Solicitudes**: parar tras este número; vacío para sin límite.
- **Durante**: parar tras este tiempo (`30s`, `2m`, `1500ms`); sin ninguno de
  los dos, 10 s.
- **Por segundo**: como mucho este número de solicitudes por segundo,
  repartidas por igual; vacío para tan rápido como responda el servidor.

Las conexiones se mantienen abiertas y se reutilizan, como hacen los clientes
reales bajo carga (un envío suelto abre una conexión nueva para que su pestaña
Red muestre la negociación). Las variables se rellenan una vez; los scripts no
se ejecutan, OAuth 2.0 obtiene su token una vez y la autenticación Digest no se
admite. Una solicitud cuenta como fallida cuando no llegó respuesta o el estado
es 400 o superior. **Detener** termina la prueba, y cerrar la ventana también la
detiene. Haz pruebas de carga solo contra servidores para los que tengas
permiso.

En una terminal, `apix bench` hace lo mismo y puede hacer fallar un trabajo de
CI:

```bash
apix bench my-api/users/list.apix -c 50 -d 30s --max-p95 300 --max-failed 1 --json bench.json
```

Termina con 1 cuando el percentil 95 supera `--max-p95` milisegundos, cuando
falló más del `--max-failed` por ciento de las solicitudes, o cuando ninguna
tuvo éxito.

## Monitores

Un **monitor** ejecuta la colección, o una carpeta o solicitud de ella, según un
horario, y avisa cuando empieza a fallar o vuelve a pasar. Abre
**Ver › Monitores…**.

![Monitores](../en/images/monitors.png)

- **Añadir monitor** pide un nombre, cada cuánto (`30s`, `5m`, `1h`, `1d`, o
  juntos como en `1h30m`; al menos `10s`), una carpeta o solicitud (vacío para
  toda la colección), un entorno y un webhook. El botón ⚙ edita un monitor y el
  botón ▷ lo ejecuta ahora.
- Cada monitor muestra su último resultado (correcto, o cuántas fallaron y los
  primeros fallos) y, mientras la programación está activada, cuándo se ejecuta
  la próxima vez.
- **Ejecutar los monitores según su horario mientras Apix esté abierto** activa
  la programación (está desactivada hasta que lo elijas). Apix ejecuta entonces
  los monitores pendientes en segundo plano, de uno en uno, y muestra una
  notificación cuando uno empieza a fallar o vuelve a pasar.
- Un **webhook** recibe el mismo aviso: Apix envía un JSON con una línea `text`
  (y `content`) que Slack, Discord, Mattermost y Google Chat muestran como
  mensaje, más los detalles (`monitor`, `status`, `passed`, `failed`,
  `failures`). Las `{{variables}}` se rellenan, así que la dirección puede
  quedarse en los secretos de un entorno.

Los monitores se guardan en el `apix.toml` de la colección, así que se comparten
a través de git:

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "users"            # una carpeta o solicitud; omítelo para toda la colección
environment = "staging"
webhook = "{{alerts_url}}"
# enabled = false        # en la lista, pero no se ejecuta según su horario
```

Para vigilar una API con la aplicación cerrada, ejecuta `apix monitor` en una
terminal, desde cron o el Programador de tareas, o con un horario de CI;
consulta
[Vigilar una API con monitores](workflows.md#vigilar-una-api-con-monitores). La
aplicación y `apix monitor` llevan el mismo registro de resultados, así que
cualquiera de los dos muestra lo que encontró el otro.

## Servidor simulado

El **servidor simulado** responde con los ejemplos guardados en las solicitudes
de la colección (consulta Ejemplos), así que se puede construir un front end o
ejecutar una prueba antes de que exista la API real. Abre
**Ver › Servidor simulado…**, elige un puerto y pulsa **Iniciar**.

![El servidor simulado](../en/images/mock.png)

- Una solicitud se empareja por método y ruta: la URL de la solicitud después
  de su host o después de una `{{variable}}` inicial, así que
  `{{base_url}}/users/:id` sirve `/users/7`. Los segmentos `:name` y `{{name}}`
  coinciden con cualquier segmento, y gana la ruta más literal. Un ejemplo cuya
  solicitud guardada tiene otra URL sirve esa URL.
- Con varios ejemplos, responde el primero con un estado 2xx. Una solicitud
  puede elegir otro con el encabezado `x-mock-response-name: <example>` o
  `x-mock-response-code: 404` (los que leen los servidores simulados de
  Postman).
- Cada respuesta permite cualquier origen (CORS) e indica de qué ejemplo vino en
  `X-Apix-Mock`; también se responde a una petición previa `OPTIONS`.
- Si no hay coincidencia, se recibe `404` con una lista JSON de lo que se sirve.
- La ventana lista lo que se sirve y las últimas solicitudes. Los ejemplos
  añadidos o editados se sirven al momento; el servidor sigue funcionando al
  cerrar la ventana, hasta **Detener** o hasta que se cierra la aplicación.

Para usarlo, añade un entorno (por ejemplo `mock`) cuya `base_url` sea
`http://127.0.0.1:4010`. Desde una terminal: `apix mock my-api --port 4010`
(`--host 0.0.0.0` para que se pueda acceder desde otros equipos, `--delay 300`
para un servidor lento).

## Documentación de la API

**Archivo › Exportar documentación…** (o **Exportar documentación…** sobre una
colección en la barra lateral) escribe la colección como documentación de la
API: una página HTML que se abre en cualquier navegador, clara u oscura, con un
índice; o Markdown para un repositorio cuando el nombre del archivo termina en
`.md`.

![Documentación generada](../en/images/api-docs.jpg)

Muestra la descripción de la colección (Markdown), sus variables y las
variables incluidas en el repositorio de cada entorno, y luego cada solicitud en
el orden de la barra lateral bajo sus carpetas: método, URL, descripción,
variables de ruta, consulta, encabezados, tipo de autenticación, cuerpo,
comprobaciones y ejemplos guardados. No se imprime nada secreto: no se leen los
archivos `.local.toml` ni el llavero, la autenticación muestra solo su tipo, y
los encabezados `Authorization`, `Cookie` y de clave de API muestran su valor
solo cuando es una `{{variable}}`. El HTML sin procesar de las descripciones se
omite.

Desde una terminal: `apix docs my-api --out api.html` (o `--out API.md`; sin
`--out`, Markdown en la salida estándar).

## Proxy de captura

El **proxy de captura** convierte tráfico real en solicitudes. Abre
**Ver › Proxy de captura…**, elige un puerto (8888) y pulsa **Iniciar**, y luego
configura `http://127.0.0.1:8888` como proxy de un navegador, una aplicación o
un script (`HTTP_PROXY` y `HTTPS_PROXY`). Cada solicitud que pasa se guarda en la
carpeta `captured/` de la colección como un archivo de solicitud, con su
respuesta como el ejemplo `Captured`, y se lista en la ventana.

![El proxy de captura](../en/images/capture.png)

- **Solo** limita la grabación a algunos hosts (`example.com` también incluye
  `api.example.com`); el resto pasa sin grabarse.
- No se escribe nada secreto: `Cookie` se descarta, y los valores de
  `Authorization` y de los encabezados de clave de API se convierten en
  variables (`{{authorization}}`).
- HTTPS viaja por un túnel que el proxy no puede leer, así que por defecto se
  deja pasar y solo se lista su host. Con **Abrir HTTPS** marcado, Apix responde
  a la negociación TLS con un certificado para el host firmado por una autoridad
  local (`<config>/Apix/capture-ca.pem`) y graba también las solicitudes. Esto
  solo funciona donde se confía en esa autoridad: la ventana muestra el comando
  que la hace de confianza (en Windows `certutil -user -addstore Root …`); Apix
  nunca la instala, y Firefox tiene su propia lista.

Desde una terminal: `apix capture my-api/captured --port 8888 --only example.com`
(`--https` para abrir HTTPS, `--no-save` para solo observar).

## Pestañas

Cada pestaña conserva sus cambios sin guardar, su última respuesta, e incluso
una solicitud que todavía está en curso. Ctrl+Tab y Ctrl+Mayús+Tab cambian de
pestaña, y Ctrl+W o un clic con el botón central cierra una; Apix pregunta antes
de cerrar cambios sin guardar. Las pestañas que estaban abiertas vuelven la
próxima vez que abres la misma colección o proyecto (las pestañas de las
colecciones cifradas no se recuerdan, porque sus nombres de archivo también son
secretos).

## Organizar las solicitudes

**Buscar**, en la parte superior de la barra lateral, la reduce a las
solicitudes cuyo nombre, método o URL contienen todas las palabras escritas
(`get users` encuentra una solicitud GET a `/users`), con las carpetas que
llevan a ellas abiertas. Una carpeta cuyo nombre coincide muestra todo lo que
contiene. Esc o ✕ borran la búsqueda, y las carpetas quedan como estaban.

Haz clic derecho en la barra lateral para crear, cambiar el nombre, duplicar o
eliminar solicitudes y carpetas; los archivos eliminados van a la papelera del
sistema. Arrastra una solicitud o carpeta sobre el centro de otra carpeta para
moverla allí. Suéltala en el borde superior o inferior de una fila para ponerla
antes o después de esa fila; una línea muestra dónde quedará. El orden se guarda
en `apix.toml` (para el nivel superior de la colección) y `folder.toml` (para
una carpeta) como una lista `order`, así que se comparte a través de git; los
elementos que no lista van a continuación por nombre. Las importaciones de
Postman conservan el orden de Postman, y las exportaciones a Postman siguen el
tuyo.

## Proyectos

Un **proyecto** agrupa varias colecciones que van juntas y comparte entre ellas
entornos y autenticación. Crea uno con **Archivo › Nuevo proyecto…**. En un
proyecto, **Entorno › Agrupar la barra lateral por** puede mostrar la barra
lateral como Proyecto › Entorno › Colecciones.

- Arrastra la cabecera de una colección por encima o por debajo de otra para
  cambiar su orden.
- Clic derecho en una colección › **Color** le da un color; un punto lo muestra
  en la barra lateral y en las pestañas de sus solicitudes. Los colores se
  guardan en `apix-project.toml`.
- **Archivo › Mover a un proyecto…** (con una sola colección abierta) mueve la
  carpeta de esa colección a un proyecto que elijas. Con un proyecto abierto,
  soltar la carpeta de una colección desde el explorador de archivos sobre la
  ventana ofrece lo mismo.

## Git

Una colección son archivos normales, así que vive en git como el código. Cuando
la colección (o el proyecto) abierta está dentro de un repositorio git, la
barra de estado muestra la rama y cuántos de sus archivos cambiaron desde el
último commit, y la barra lateral marca cada solicitud modificada: **M**
modificada (su archivo o sus propios scripts), **A** nueva. Las marcas siguen
cada guardado y cada cambio en disco, y se vuelven a leer cuando la ventana
vuelve al primer plano, así que un commit hecho en una terminal las borra.

![La rama y las solicitudes modificadas](../en/images/git-status.png)

Haz clic en la rama en la barra de estado (o **Ver › Git…**) para abrir la
ventana Git. A la izquierda están los archivos modificados desde el último
commit y los commits más recientes; elige un archivo para ver qué cambió en él,
igual que lo muestra `apix diff`: una solicitud campo a campo
(`query._limit: 5 → 10`, un encabezado añadido), los scripts y el resto del
texto línea a línea, y nada en absoluto cuando solo cambió el formato. Abajo,
escribe un mensaje y pulsa **Commit**: incluye todos los cambios de la carpeta
abierta y nada de fuera de ella. El menú de ramas cambia de rama, **Nueva rama**
crea una con el nombre escrito a su lado y cambia a ella, y **Pull** (solo
fast-forward) y **Push** (definiendo el upstream la primera vez) ejecutan git
con tu propia configuración, hooks y credenciales. La barra lateral sigue al
momento un cambio de rama.

![La ventana Git](../en/images/git-window.png)

En la línea de comandos, `apix run --changed` envía solo las solicitudes a las
que afecta un cambio y `apix diff` muestra qué cambió en ellas campo a campo; en
un pull request, la GitHub Action puede comentar el resultado. Consulta
[Solo lo que cambió](workflows.md#solo-lo-que-cambió-ejecuciones-que-conocen-git).

## Colecciones cifradas

Una colección cifrada guarda sellados en disco todas las solicitudes, los
entornos e incluso los nombres de archivo (XChaCha20-Poly1305 con una clave de
contraseña Argon2id). Git solo ve qué blob cifrado cambió.

- **Archivo › Cifrar esta colección…** o clic derecho en una colección ›
  **Cifrar…** cifra una colección existente en su sitio.
- **Archivo › Nueva colección cifrada…** crea una dentro de un proyecto.
- Recibes una **clave de recuperación** una sola vez. Guárdala en un lugar
  seguro: abre la colección si se pierde la contraseña.
- Al abrir la colección se pide la contraseña. Se vuelve a bloquear cuando
  eliges **Archivo › Bloquear colecciones cifradas**, sales de Apix o la dejas
  inactiva durante el tiempo definido en Configuración (15 minutos por defecto).
- Marca **Recordar en este equipo** en la ventana de desbloqueo para guardar la
  contraseña en el almacén de credenciales del sistema (el Administrador de
  credenciales en Windows, el Llavero en macOS, el Secret Service en Linux). La
  colección se abre entonces sin preguntar mientras tengas la sesión iniciada, y
  la línea de comandos `apix` también usa la contraseña guardada. **Archivo ›
  Olvidar la contraseña guardada** (o clic derecho en la colección) la quita. Si
  la contraseña se cambió en otro lugar, la guardada se descarta y Apix vuelve a
  preguntar.

## Copiar como y pegar curl

El botón de copiar junto a **Enviar** (o **Solicitud › Copiar como**) copia la
solicitud como `curl` para bash o fish, `curl.exe` para el símbolo del sistema
de Windows, una llamada `Invoke-RestMethod` de PowerShell, un comando `http` de
HTTPie (HTTPie 3.0 o posterior) o un comando `wget` de GNU, con las variables
rellenadas. wget no puede enviar formularios multipart; el comando copiado lo
indica. Las solicitudes firmadas en cada envío (OAuth 1.0, AWS para PowerShell
y código) se firman en el momento de copiar.

El mismo menú copia la solicitud como un programa: `fetch` de JavaScript, axios
de Node.js, requests de Python, `net/http` de Go, reqwest de Rust,
`HttpClient` de C#, `HttpClient` de Java, cURL de PHP, `URLSession` de Swift u
OkHttp de Kotlin. Cada uno envía lo que enviaría Apix (URL, encabezados,
autenticación y todos los tipos de cuerpo) y los ajustes de la solicitud que el
lenguaje puede expresar (tiempo límite, redirecciones, verificación TLS, proxy,
certificado de cliente). Un ajuste para el que un lenguaje no tiene una opción
sencilla se lista en un comentario `Not in this snippet` al principio, en lugar
de descartarse en silencio. En la barra lateral, haz clic derecho en una
solicitud y usa **Copiar como código**.

La CLI escribe lo mismo: `apix code request.apix --as python` (`apix shell` es
el mismo comando; `--as` acepta `bash`, `cmd`, `powershell`, `fetch`, `axios`,
`python`, `go`, `rust`, `csharp`, `java`, `php`, `swift` o `kotlin`).

Pega un comando `curl` (por ejemplo, del «Copiar como cURL» de tu navegador) en
el campo de la URL y Apix rellena toda la solicitud: método, URL, consulta,
encabezados, autenticación y cuerpo.

## Importar desde Postman

**Archivo › Importar…**, o suelta una exportación de Postman en la ventana. Se
admiten colecciones (v2.0 y v2.1) y entornos. Se trasladan las carpetas, las
solicitudes, los cuerpos, la autenticación y las variables; lo que Apix aún no
puede trasladar se lista en un informe. Los scripts previos y de prueba se
guardan como archivos `.js` junto a sus solicitudes, y los secretos de los
entornos van al `.local.toml` ignorado por git.

**Muchos archivos a la vez.** Abre o suelta una carpeta que contenga
exportaciones de Postman (o el zip que produce *Export data* de Postman) y Apix
reconoce los archivos por su contenido, no por su nombre, y se ofrece a
importarlo todo como un único proyecto: una colección por cada colección de
Postman, los entornos compartidos por todas ellas, y las variables globales como
variables del proyecto (las globales secretas se omiten, porque las variables
del proyecto se incluyen en el repositorio). Dos entornos con el mismo nombre se
guardan como `name` y `name-2`.

**Exportar a Postman.** Haz clic derecho en una colección y elige **Exportar a
Postman…** (o **Archivo › Exportar a Postman…**) para escribir un archivo de
colección Postman v2.1 y un archivo de entorno por cada entorno, para que los
compañeros que usan Postman puedan seguir trabajando con ella. Se trasladan las
carpetas, la autenticación, los cuerpos, los scripts, las variables y los
ajustes de redirección, TLS y cookies. Postman no tiene tiempo límite, proxy ni
certificado de cliente por solicitud, y los valores secretos de los entornos se
exportan solo con su nombre; la notificación lista lo que se omitió.

En la línea de comandos: `apix import <file, folder or zip>` y
`apix export <collection> --out <folder>`.

## Importar desde Insomnia, OpenAPI y HAR

**Archivo › Importar…** (o soltar el archivo en la ventana) también lee estos
formatos; el formato se reconoce por el contenido, y una carpeta o un zip pueden
mezclarlos:

| Formato | Lo que obtienes |
|---|---|
| Insomnia v4 (exportación JSON) | Una colección por espacio de trabajo, carpetas con su autenticación, el entorno base como variables de la colección y cada subentorno como un entorno. `{{ _.name }}` se convierte en `{{name}}`. |
| OpenAPI 3 / Swagger 2 (JSON o YAML) | Una solicitud por operación, una carpeta por etiqueta. El servidor pasa a ser la variable `baseUrl` (con varios servidores, un entorno para cada uno), los parámetros de ruta pasan a ser variables con sus valores de ejemplo, los parámetros de consulta y de encabezado obligatorios se activan, los cuerpos salen de los ejemplos o se construyen a partir del esquema, y los esquemas de seguridad pasan a ser autenticación con variables de credenciales vacías (`bearerToken`, `apiKey`, …) que se rellenan en un entorno. |
| WSDL 1.1 | Una solicitud por operación con un sobre de ejemplo; consulta [SOAP](#soap). |
| HAR («Save all as HAR» de las DevTools del navegador) | Solo las llamadas a API, una carpeta por host. Se omiten los scripts, estilos, imágenes y fuentes, las URL `data:` y las repeticiones, y también los encabezados `Cookie`: llevan la sesión del navegador. |

Una sola colección de un solo archivo se importa como colección; varias se
convierten en un proyecto. El informe lista todo lo que no se pudo trasladar.

## Historial

Cada solicitud que envías se guarda con su respuesta. **Ver › Historial…**
(Ctrl+H) las lista, de la más reciente a la más antigua, con el método, el
nombre, la URL, el estado y cuándo se envió; escribe para buscar por cualquiera
de ellos. Elige una entrada y la solicitud vuelve a la pestaña desde la que se
envió, como cambios sin guardar, junto con la respuesta que obtuvo. Pulsa
Ctrl+Intro para enviarla de nuevo, o Ctrl+S para quedarte con esa versión. Si el
archivo de la solicitud ya no existe, la entrada se abre en el editor sin
pestaña.

El historial se queda en este equipo (la carpeta `history` junto a la
configuración), nunca dentro de una colección. Las solicitudes se guardan tal
como están escritas, con las `{{variables}}` sin resolver, así que los secretos
de los entornos no se copian en él. Se guardan las últimas 300 solicitudes, con
cuerpos de respuesta de hasta 256 KB cada uno. Las solicitudes de colecciones
cifradas nunca se escriben en el historial. **Borrar historial**, en la ventana,
lo vacía; **Archivo › Configuración… › Historial** lo desactiva.

## Comparar respuestas

**⇄** junto a la respuesta (o **Solicitud › Comparar respuestas…**) pone la
respuesta que tienes ahora junto a otra de la misma solicitud:

- **Respuestas anteriores**: las respuestas de la solicitud en el historial, de
  la más reciente a la más antigua. Elige una para ver qué cambió desde
  entonces; así se viaja atrás en el tiempo hasta la respuesta de ayer.
- **Otros entornos**: la solicitud enviada de nuevo en otro entorno (staging
  frente a producción, un servidor simulado frente a la API real).

Muestra cuántas cosas difieren, y luego el estado, los encabezados por nombre y
el cuerpo: el JSON campo a campo (los registros de las listas se emparejan por
su `id`, `_id`, `uuid` o `key`, así que una lista en otro orden no es un
cambio, y `[id=1].email` nombra el campo), y el resto del texto línea a línea.
Los encabezados que cambian en cada respuesta (Date, ids de solicitud, caché,
cómo se envió el cuerpo) se omiten y se listan al final. **Copiar como texto**
copia el resultado.

![Comparación con una respuesta anterior](../en/images/compare.png)

En una terminal, `apix compare <request> --env staging --against production`
envía la solicitud en ambos entornos e imprime las diferencias; termina con 1
cuando difieren, así que un pipeline puede comprobar que dos entornos responden
igual. El historial guarda cuerpos de hasta 256 KB, así que los más largos se
comparan hasta donde se guardaron.

## Asistente de IA

El asistente de IA está desactivado hasta que lo activas en **Archivo ›
Configuración… › Asistente de IA**. Apix es de código abierto y no tiene cuenta
de IA propia: eliges un proveedor y das tu propia clave de API.

- **Proveedor de IA**: Claude (Anthropic), ChatGPT (OpenAI) o Gemini (Google).
- **Modelo**: el predeterminado del proveedor (Claude Opus 5.5, `gpt-5.5`,
  `gemini-2.5-flash`), o **↻** carga los modelos que puede usar tu clave y
  eliges uno.
- **Clave de API**: para el proveedor de arriba, guardada en el llavero del
  sistema, nunca en un archivo; guardar el campo vacío la quita. Si no hay
  ninguna, se usa `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` o `GEMINI_API_KEY`.

Entonces la respuesta recibe un botón más:

- **Pruebas IA** en una respuesta correcta: la IA escribe un script de Pruebas
  que comprueba el estado, el tipo de contenido y la forma del cuerpo, y Apix lo
  añade a las Pruebas de la solicitud (marcado como escrito por ella, sin
  guardar), para que lo leas antes de conservarlo.
- **Explicar** en una respuesta fallida (400 o superior): qué significa la
  respuesta, las causas probables y qué comprobar, en el idioma de la
  aplicación.

![Explicación de una respuesta fallida](../en/images/ai-explain.png)

Lo que se envía al proveedor: el método, la URL, los encabezados y el cuerpo de
la solicitud, y el estado, los encabezados y el cuerpo de la respuesta
(recortado a 12 KB). Authorization, las cookies y los encabezados con key,
token, secret o signature en su nombre se ocultan como •••. La IA puede
equivocarse; no se cambia ni se envía nada a ningún otro sitio sin ti. La
captura se hizo contra un servidor de prueba local, no es una respuesta real.

## Complementos

Los complementos añaden lo que Apix no trae incluido: un tipo de autenticación,
un importador para otro formato de archivo o una vista de una respuesta. Un
complemento es un único archivo WebAssembly (`.wasm`) y se ejecuta aislado:
recibe JSON y responde JSON, sin red, sin archivos y sin reloj, y se detiene si
tarda demasiado o usa demasiada memoria. Un complemento de otra persona puede
cambiar la solicitud que firma o la colección que importa, y nada más.

**Ver › Complementos…** lista lo que está instalado y lo que ofrece cada
complemento. **Instalar desde un archivo…** comprueba un archivo `.wasm` y lo
copia en la carpeta de complementos (`<config>/Apix/plugins`), **Quitar** lo
elimina, **Abrir carpeta** muestra la carpeta y **Recargar** la vuelve a leer
después de que copies archivos allí tú mismo.

![La ventana Complementos](../en/images/plugins.png)

- Los **tipos de autenticación** aparecen en la lista de la pestaña
  Autenticación como «… (complemento)», con los campos que pide el complemento
  (las `{{variables}}` funcionan en ellos). Al enviar la solicitud, el
  complemento recibe la solicitud final (método, URL, encabezados, cuerpo) y
  añade encabezados o parámetros de consulta. Una solicitud cuyo complemento no
  está instalado lo indica y no se envía.

  ![Una solicitud firmada por un complemento](../en/images/plugin-auth.png)

- Las **vistas** aparecen junto a Texto, Tabla y Gráfico encima de una respuesta
  JSON; el complemento lee el estado, los encabezados y el cuerpo y responde con
  una tabla o un texto.

  ![Una vista de un complemento](../en/images/plugin-view.png)

- Los **importadores** tienen un botón **Importar un archivo…** en la ventana
  Complementos: el archivo se convierte en una nueva colección a su lado, que
  luego se abre.

En un archivo de solicitud, una autenticación de complemento es:

```toml
[auth.plugin]
plugin = "hmac-signature"

[auth.plugin.config]
key_id = "k1"
secret = "{{signing_secret}}"
```

Desde una terminal:

```sh
apix plugins list                     # complementos instalados y lo que ofrecen
apix plugins install my-plugin.wasm
apix plugins remove my-plugin
apix import endpoints.csv --plugin csv-endpoints --out shop
apix send posts/list-posts.apix --view json-stats
```

La carpeta `plugins/` del repositorio tiene tres ejemplos, compilados a partir
de `crates/apix-plugin-sdk/examples`: **hmac-signature** (firma las solicitudes
con HMAC-SHA256 sobre el método, la ruta y el hash del cuerpo),
**csv-endpoints** (una lista CSV de `method,url,name,folder` se convierte en
solicitudes) y **json-stats** (estadísticas de los campos de una lista JSON:
tipos, presencia, valores distintos, rangos).

### Escribir un complemento

En Rust, añade la dependencia `apix-plugin-sdk`, implementa su trait `Plugin` y
llama a `export!`:

```rust
use apix_plugin_sdk::{Plugin, export, serde_json::{Value, json}};

struct Hello;

impl Plugin for Hello {
    fn manifest() -> Value {
        json!({ "name": "hello", "version": "1.0.0",
                "visualize": { "label": "Hello" } })
    }
    fn call(_hook: &str, input: Value) -> Result<Value, String> {
        let size = input["body"].as_str().unwrap_or("").len();
        Ok(json!({ "text": format!("{size} bytes") }))
    }
}

export!(Hello);
```

Compila el crate como `cdylib` con `cargo build --release --target
wasm32-unknown-unknown` e instala el `.wasm`. El `name` del manifiesto (letras
minúsculas, dígitos y `-`) es como las solicitudes se refieren al complemento;
ofrece cualquiera de estos:

| Clave del manifiesto | El hook recibe | Responde |
|---|---|---|
| `auth: {label, fields: [{name, label, secret}]}` | `{method, url, headers, body, config}` | `{headers: [[name, value]], query: [[name, value]]}` |
| `import: {label, extensions}` | `{file_name, content}` | `{name, requests: [{folder, name, method, url, headers, body}]}` |
| `visualize: {label}` | `{status, headers, body}` | `{title, text}` o `{title, columns, rows}` |

Sirve cualquier lenguaje que compile a WebAssembly. El módulo exporta `memory`,
`apix_alloc(len) -> ptr`, `apix_manifest() -> i64` y `apix_call(hook_ptr,
hook_len, input_ptr, input_len) -> i64`, donde el `i64` es `ptr << 32 | len` de
una respuesta JSON en UTF-8, `{"ok": …}` o `{"error": "…"}`. No puede importar
nada, y cada llamada se ejecuta en una instancia nueva.

## Paleta de comandos

Ctrl+K (o Ctrl+P) abre la paleta de comandos: escribe unas letras del nombre de
una solicitud o de un comando y pulsa Intro. Los comandos se pueden encontrar
por su nombre en inglés en todos los idiomas.

![La paleta de comandos](../en/images/palette.png)

## Configuración inicial

En el primer inicio, Apix abre la **Configuración inicial**, con los extras
opcionales en un solo lugar; **Ayuda › Configuración inicial…** la vuelve a
abrir. Apix funciona sin ninguno de ellos.

- **Base de datos de ubicación IP**: sitúa cada router en el mapa de la ruta y
  en `apix trace`. **Descargar** descarga la base de datos gratuita de ciudades
  de DB-IP (unos 130 MB, CC BY 4.0) en `<config>/Apix/geo`; **Actualizar**
  descarga el mes más reciente. No se consulta nada en línea. En una terminal:
  `apix geo download`, `apix geo status`, `apix geo remove`.
- **Asistente de IA**: muestra si está activado y con qué proveedor; **Abrir
  Ajustes** lleva a sus filas.
- **Claude (MCP)**: **Conectar con Claude** (también **Ayuda › Conectar con
  Claude (MCP)…**) para la colección abierta; se explica a continuación.

![Configuración inicial](../en/images/setup.png)

### Conectar Claude

**Conectar con Claude** convierte la colección abierta en un servidor MCP para
Claude, de modo que Claude puede listar, enviar y ejecutar sus solicitudes con
sus pruebas sin ver los valores de las variables (consulta
[MCP](workflows.md#dejar-que-un-agente-de-ia-use-tu-colección-mcp)).

- **Añadir a Claude Desktop** escribe la entrada `apix-<collection>` en el
  `claude_desktop_config.json` de Claude Desktop. Se conservan sus demás
  servidores y ajustes, y el archivo tal como estaba se copia a `.bak` la
  primera vez; reinicia Claude Desktop para usarlo. **Quitar** vuelve a sacar la
  entrada.
- **Añadir a Claude Code** ejecuta `claude mcp add --scope user …`, así que la
  colección está disponible en todos los proyectos.
- **Solo listar** da a Claude solo las herramientas de listado (`--list-only`).
- Tanto la entrada como el comando se muestran con **Copiar**, para añadirlos a
  mano.

![Conectar con Claude](../en/images/claude-connect.png)

Lo mismo desde una terminal: `apix mcp my-api --add-to claude-desktop`,
`apix mcp my-api --add-to claude-code`, `apix mcp my-api
--remove-from-claude-desktop`.

## Configuración e idioma

**Archivo › Configuración…** (Ctrl+,):

- **Idioma:** English, Türkçe, العربية, Deutsch, Français, Русский, 日本語,
  简体中文 o Español. El cambio se aplica al momento. En el primer inicio, Apix
  sigue el idioma del sistema. En árabe la ventana se refleja: la barra lateral
  está a la derecha y los menús, las pestañas y los formularios van de derecha a
  izquierda, mientras que las URL, el código y las tablas clave–valor siguen de
  izquierda a derecha.
- **Bloquear colecciones cifradas:** tras 5, 15 o 30 minutos, 1 hora, o nunca.
- **Valores secretos de los entornos:** en un archivo `.local.toml` o en el
  llavero del sistema (consulta
  [Variables y entornos](#variables-y-entornos)).
- **Confirmar en producción:** preguntar antes de DELETE, PUT o PATCH mientras
  está activo un entorno «prod» (activado por defecto).
- **Buscar actualizaciones:** al iniciar, preguntar a GitHub si se ha publicado
  un Apix más nuevo (activado por defecto). Solo se envía esa solicitud, sin
  nada que te identifique, y no se descarga nada: una versión más nueva muestra
  una notificación con un botón **Descargar** que abre su página. **Ayuda ›
  Buscar actualizaciones…** pregunta en cualquier momento, y `apix update` hace
  lo mismo en una terminal.

![Anuncio de una versión más nueva](../en/images/update.png)

- **Historial:** activado o desactivado (consulta [Historial](#historial)).

La parte **Tema** de la configuración cambia el aspecto de Apix, al momento, sin
reiniciar:

- **Apariencia:** oscura, clara, o **Seguir el sistema**, que cambia cuando
  Windows o macOS pasan del modo claro al oscuro.
- **Tema oscuro:** Obsidian (el predeterminado), Cyber glass, Dracula, Nord o
  Solarized Dark. **Tema claro:** Luz de día o Solarized Light. Elegir un tema
  del otro tipo cambia la apariencia a ese tipo.
- **Color de acento:** el del tema, o cian, azul, morado, rosa, rojo, naranja,
  amarillo o verde, para los botones, las selecciones, los marcos de foco y los
  chips de variables. Los temas claros reciben un tono más intenso, y el texto
  sobre el acento es oscuro o blanco, lo que se lea mejor.
- **Transparencia:** un control deslizante de 0 (una ventana opaca) a 90%:
  cuánto del escritorio se ve a través, desenfocado como vidrio esmerilado
  (acrylic en Windows, vibrancy en macOS). El fondo de la ventana se aclara con
  él, los paneles a un ritmo más lento para que el texto siga teniendo un fondo,
  y los bordes se iluminan como el canto de un vidrio; los menús y los diálogos
  siguen opacos. Los temas claros se aclaran con más suavidad, porque el texto
  oscuro necesita un fondo claro. El valor por defecto es 20%.
- **Tamaño del texto:** 90%, 100%, 110%, 125% o 150% del tamaño normal, para
  todo el texto de la ventana, al momento.
- **Tamaño de fuente del código:** de 11 a 18 px para los editores de código
  (cuerpos, scripts, la vista Formateado de la respuesta); el visor de cuerpos
  muy grandes mantiene 13 px.
- **Densidad:** filas compactas, normales u holgadas en la barra lateral.
- Los editores de código toman sus colores del tema: las cadenas, los números y
  las palabras clave como los da el tema, los tipos y las funciones en el color
  de acento, y los comentarios y la puntuación atenuados.

![Tamaño del texto, tamaño de fuente del código y densidad](../en/images/look.png)

- **Tus temas:** **Abrir la carpeta** abre `<config>/Apix/themes` y, la
  primera vez, escribe allí `my-theme.toml` como punto de partida. Cada archivo
  `.toml` de esa carpeta es un tema en las listas; un archivo con el nombre de un
  tema integrado (`nord.toml`) lo sustituye. Un archivo que no se puede leer
  aparece en rojo con el motivo.

![Configuración](../en/images/settings.png)

Un archivo de tema define un nombre, si es claro, y cualquiera de estos colores
(`#RRGGBB`, o `#RRGGBBAA` con opacidad); los que omite vienen de Obsidian, o de
Luz de día para un tema claro:

```toml
name = "Deep sea"
light = false

[colors]
background = "#03141FEE"      # la ventana, un degradado hacia background_end
background_end = "#062A3AEE"
surface = "#05202DB3"         # paneles
raised = "#0A2B3BF7"          # menús y diálogos
border = "#FFFFFF14"
accent = "#FF9F43"
text = "#E8F6FF"
muted = "#7FA3B8"
ok = "#34D399"                # 2xx, pruebas superadas
warning = "#FBBF24"
error = "#F87171"
string = "#86EFAC"            # JSON en el visor de cuerpos grandes
number = "#FDBA74"
keyword = "#C4B5FD"
```

Los archivos de tema son texto plano, así que un equipo puede compartirlos; Apix
los lee al iniciarse y al abrir la configuración.

La configuración se guarda en `settings.toml`, en la carpeta Apix del directorio
de configuración de tu sistema (`%APPDATA%\Apix` en Windows,
`~/Library/Application Support/Apix` en macOS, `~/.config/Apix` en Linux).

## Atajos de teclado

| Teclas | Acción |
|---|---|
| Ctrl+Intro | Enviar |
| Esc | Cancelar la solicitud |
| Ctrl+S | Guardar |
| Ctrl+N | Nueva solicitud GET |
| Ctrl+O | Abrir una carpeta, un proyecto o una colección |
| Ctrl+K o Ctrl+P | Paleta de comandos |
| Ctrl+E | Variables y entornos |
| Ctrl+, | Configuración |
| Ctrl+H | Historial |
| Ctrl+Mayús+R | Runner |
| Alt+1…7 | Cambiar el método |
| Ctrl+W | Cerrar la pestaña |
| Ctrl+Tab / Ctrl+Mayús+Tab | Pestaña siguiente / anterior |
| Ctrl+Espacio | Sugerencias en la consulta GraphQL (tras obtener el esquema) |

En macOS, usa ⌘ en lugar de Ctrl (Ctrl+Tab y Ctrl+Espacio siguen siendo Ctrl).
