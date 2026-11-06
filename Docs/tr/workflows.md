<!-- translated from en/workflows.md sha256:ced46a4d6c27e0a9 -->

# İş akışları

Sık yapılan işler için adım adım tarifler. [Kullanım kılavuzu](user-guide.md)
uygulamanın her bölümünü anlatır; [teknik notlar](technical.md) Apix'in derlenmesini
ve ölçülmesini ele alır.

- [Başlarken: ilk isteği gönderme ve kaydetme](#başlarken-ilk-isteği-gönderme-ve-kaydetme)
- [Koleksiyonu git'te tutma](#koleksiyonu-gitte-tutma)
- [Dev, staging ve production ile çalışma](#dev-staging-ve-production-ile-çalışma)
- [Gizli bilgiler içeren bir koleksiyonu paylaşma](#gizli-bilgiler-içeren-bir-koleksiyonu-paylaşma)
- [Postman'dan geçiş](#postmandan-geçiş)
- [Koleksiyonu CI'da çalıştırma](#koleksiyonu-cida-çalıştırma)
- [Bir API'yi monitörlerle izleme](#bir-apiyi-monitörlerle-izleme)
- [OAuth 2.0 ile oturum açma](#oauth-20-ile-oturum-açma)
- [Bir AWS API'sini çağırma](#bir-aws-apisini-çağırma)
- [Dosya yükleme, GraphQL gönderme](#dosya-yükleme-graphql-gönderme)
- [Bir gRPC servisini çağırma](#bir-grpc-servisini-çağırma)
- [Tarayıcıdaki bir isteği yeniden oluşturma](#tarayıcıdaki-bir-isteği-yeniden-oluşturma)
- [Bir istek neden yavaş ya da başarısız?](#bir-istek-neden-yavaş-ya-da-başarısız)
- [Bir yapay zekâ ajanına koleksiyonu kullandırma (MCP)](#bir-yapay-zekâ-ajanına-koleksiyonu-kullandırma-mcp)

## Başlarken: ilk isteği gönderme ve kaydetme

1. **Dosya › Yeni koleksiyon…** seçin, boş bir klasör seçip ona bir ad verin.
2. Ctrl+N yeni bir GET isteği açar. `https://jsonplaceholder.typicode.com/users` yazın.
3. Ctrl+Enter isteği gönderir; yanıt sağda görünür.
4. Ctrl+S isteği koleksiyon klasöründe yeni bir dosya olarak kaydeder. Ona
   "List users" gibi bir ad vermek için kenar çubuğunda sağ tıklayıp ›
   **Yeniden adlandır…** seçin.

Dosya, okuyup düzenleyebileceğiniz düz TOML'dur:

```toml
name = "List users"
url = "https://jsonplaceholder.typicode.com/users"
```

## Koleksiyonu git'te tutma

Koleksiyon bir klasördür:

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

`*.local.toml` dışındaki her şeyi commit'leyin; Apix, ilk kez bir gizli değer
yazdığında bu kalıbı `.gitignore`'a kendisi ekler. Her istek kendi küçük
dosyasıdır; böylece incelemeler ve birleştirmeler okunaklı kalır.

## Dev, staging ve production ile çalışma

1. **Ortam › Değişkenler ve ortamlar…** (Ctrl+E) menüsünü açın.
2. Her birinde bir `base_url` bulunan `dev`, `staging` ve `prod` adlı ortamlar
   ekleyin.
3. İsteklerinizde `{{base_url}}/users` kullanın.
4. Ortamı kenar çubuğunda, koleksiyon adının altından seçin.

Commit'lenmemesi gereken değerler (token'lar, parolalar) **Gizli** işaretini
alır. Her ekip arkadaşı kendi `*.local.toml` dosyasını tutar. Koleksiyon
düzeyinde tanımlanan bir değişken, ortamların geçersiz kılabileceği bir
varsayılandır.

## Gizli bilgiler içeren bir koleksiyonu paylaşma

Koleksiyonun tamamı hassas olduğunda (dahili URL'ler, örnek müşteri verileri,
uzun ömürlü anahtarlar) onu şifreleyin:

1. **Dosya › Bu koleksiyonu şifrele…** seçin ve bir parola belirleyin.
2. Size gösterilen **kurtarma anahtarını** parola yöneticinizde saklayın.
3. Klasörü commit'leyin. Diskte bir dizi şifreli nesneden oluşur; dosya adları
   bile gizlidir.

Ekip arkadaşları klasörü Apix'te açıp parolayı yazar. Bir terminalde veya CI'da:

```sh
APIX_PASSWORD='…' apix run my-api --env staging
```

`apix decrypt my-api --out my-api-plain` düz bir kopya yazar; parola yerine
kurtarma anahtarını kullanmak için `--recovery` ekleyin.

## Postman'dan geçiş

1. Postman'da koleksiyonu (Collection v2.1) ve her ortamı dışa aktarın.
2. Apix'te **Dosya › İçe aktar…** seçin ya da dosyaları pencereye
   bırakın. Veya bir terminalde:

   ```sh
   apix import "My API.postman_collection.json"            # creates ./my-api
   apix import "Staging.postman_environment.json" --into my-api
   ```

3. İçe aktarma raporunu okuyun: aktarılamayan her şey orada listelenir
   (örneğin NTLM yetkilendirmesi veya kaydedilmiş örnek yanıtlar).
4. Pre-request ve test betikleri isteklerinin yanında `.js` dosyaları olarak
   saklanır; Apix bunları henüz çalıştırmaz.
5. Yetkilendirme kodu akışlı OAuth 2.0 için kimlik sağlayıcınıza
   `http://127.0.0.1:8765/callback` gibi bir loopback yönlendirme URL'si
   kaydedin ve bunu Yetkilendirme sekmesinde ayarlayın.

Tüm çalışma alanını mı taşıyorsunuz? Postman'da **Settings › Data › Export
data** kullanın, sonra klasörü (veya zip'i) Apix'te açın ya da pencereye
bırakın: her koleksiyon, ortam ve global tek bir proje olarak gelir
(`apix import postman-export.zip` de aynısını yapar). Postman'da kalan ekip
arkadaşları koleksiyonunuzu **Postman'a dışa aktar…** ile geri alır.

## Koleksiyonu CI'da çalıştırma

`apix run` bir koleksiyonun tüm isteklerini kenar çubuğu sırasıyla gönderir ve
herhangi biri başarısız olursa sıfırdan farklı bir kodla çıkar; bu yüzden bir
CI adımı olarak çalışır:

```sh
apix run my-api --env staging --var token="$API_TOKEN"
```

Testi (script ya da Doğrulama) olan istekler testlerine, olmayanlar durum
koduna göre geçer ya da kalır. Çıktı her testi isteğinin altında listeler:

```text
create.apix                              201 Created  466.7 ms · 42 B
  │ new id 101
  ✓ created
get.apix                                 200 OK  174.7 ms · 292 B
  ✓ status 200
  ✗ has the right id expected 1 to equal 2

1 passed, 1 failed (1 failed tests)
```

Bir scriptin atadıkları (`pm.environment.set`, `pm.variables.set`) koşunun
sonraki isteklerine taşınır; `pm.execution.setNextRequest("ad")` başka bir
isteğe atlar (`null` o tekrarı bitirir).

Seçenekler:

| Seçenek | Ne yapar |
|---|---|
| `apix run my-api/users` | Yalnızca bir klasördeki istekler (ya da tek bir istek dosyası) |
| `-d, --data users.csv` | Veri odaklı koşu: CSV ya da JSON'daki her satır bir tekrardır, sütunları değişken olur (`{{email}}`, `pm.iterationData.get("email")`) |
| `-n, --iterations 5` | İstekleri 5 kez dolaşır (varsayılan: veri satırı başına bir kez, yoksa bir kez) |
| `--delay 200` | İstekler arasında 200 ms bekler |
| `--bail` | Başarısız olan ilk istekte durur |
| `-p, --parallel 4` | 4 tekrarı aynı anda koşar, 4 kullanıcı gibi; her biri isteklerini sırayla gönderir ve scriptlerinin atadıklarını kendine saklar |
| `--junit report.xml` | CI servislerinin test listesi olarak gösterdiği JUnit XML raporu |
| `--json report.json` / `--html report.html` | Her şey JSON olarak ya da her tarayıcıda açılan bir sayfa |
| `--markdown report.md` | Pull request yorumu ya da CI iş özeti için bir özet (aşağıya bakın) |
| `--changed` / `--changed=origin/main` | Yalnızca bir değişikliğin dokunduğu istekler (aşağıya bakın) |
| `--timeout 10`, `-k`, `--proxy URL` | İsteklerin bağlantı ayarlarını geçersiz kılar |
| `--env-file .env` | Değişkenleri bir dotenv dosyasından okur (aşağıya bakın) |

Bir veri dosyası şöyle görünür; koleksiyon dışından gelen değerler (`--var` ve
aşağıdakiler) sütunlarından üstündür:

```csv
id,name
1,Leanne Graham
2,Ervin Howell
```

### CI'da gizli değerler

Gizli değerler git'e girmez (`.local.toml` dosyaları yok sayılır, anahtarlık
senin bilgisayarındadır); bu yüzden bir pipeline onları üç yoldan biriyle
verir, her biri öncekinden üstündür:

1. `--env-file .env`: `AD=değer` satırlarından oluşan bir dotenv dosyası
   (`#` yorumları, tırnaklar ve `export` kullanılabilir).
2. `APIX_VAR_<AD>` ortam değişkenleri: `APIX_VAR_TOKEN`, `{{token}}`'ı
   doldurur. CI gizli değerleri genellikle büyük harfle yazıldığı için ad,
   koleksiyonun tanımladığı ya da isteklerinin kullandığı bir değişkenle büyük
   küçük harf farkı gözetmeden eşleşir.
3. `--var ad=değer`.

Üçü de ortamın değerlerinden (ve veri dosyasının sütunlarından) üstündür.
`apix` hangi adları nereden aldığını stderr'e yazar, değerleri asla yazmaz.

```sh
APIX_VAR_TOKEN="$API_TOKEN" apix run api --env staging
```

### GitHub Actions

Depo aynı zamanda bir GitHub Action'dır. Koleksiyonu çalıştırır ve JUnit ile
HTML raporlarını yazar. Bir sürüm etiketinde kullanılırsa o sürümün `apix`'ini
indirir (saniyeler); bir dal ya da commit'te `apix`'i ilk seferde derleyip
önbelleğe alır, `apix-version: latest` (ya da `0.1.1`) ise bir sürümü indirir:

```yaml
- uses: 11R5/Apix@v0.1.1        # bir sürüm: apix indirilir, derlenmez
  with:
    path: api
    environment: staging        # ayrıca: data, iterations, delay, bail, env-file, junit, html, json, markdown, changed, diff, comment, args, apix-version
  env:
    APIX_VAR_TOKEN: ${{ secrets.API_TOKEN }}
```

[examples/ci/github-actions.yml](../../examples/ci/github-actions.yml) eksiksiz
bir iş akışıdır: koleksiyonu her ortam için bir kez çalıştırır (`test` ve
`staging` matrisi), JUnit sonuçlarını koşunun sayfasında gösterir ve HTML
raporları saklar. Action, Markdown özetini işin sayfasına da ekler.

## Yalnızca değişenler: git'e duyarlı koşular

Koleksiyon git'teki bir klasör olduğu için `apix`, bir değişikliğin neye
dokunduğunu git'e sorabilir. `--changed` yalnızca o istekleri gönderir:

```sh
apix run api --changed                 # henüz commit'lenmemiş olanlar
apix run api --changed=origin/main     # bu dalın main'den ayrıldığından beri değiştirdikleri
```

Bir istek şu durumlarda yeniden koşar: kendi dosyası ya da scriptleri
değiştiyse; üstündeki bir `folder.toml` ya da klasör scripti değiştiyse (o
klasördeki tüm istekler); `apix.toml`, bir koleksiyon scripti, bir ortam ya da
proje dosyası değiştiyse (tüm istekler); ya da değişen başka bir dosyayı
adıyla anıyorsa (gövde dosyası, `.proto`). Yeni dosyalar commit'lenmeden de
sayılır; silinen istekler listelenir. Değer verilmezse `--changed`, GitHub
Actions'ta pull request'in hedef dalıyla (`origin/$GITHUB_BASE_REF`), başka
yerde `HEAD` ile karşılaştırır. Değeri `=` ile yazın; böylece koşulacak
klasörle karışmaz.

```text
2 of 3 requests changed since origin/main
  users/create-user.apix  ← users/create-user.test.js
  users/list-users.apix
```

Karşılaştırma, pull request'teki gibi dalın hedeften ayrıldığı noktadan
başlar; arada main'e gelenler sayılmaz. Bunun için geçmiş gerekir: sığ bir
klonda (`actions/checkout`'un varsayılanı) `apix` uyarır ve doğrudan hedefle
karşılaştırır; `fetch-depth: 0` ile checkout edin. Şifreli koleksiyonlar
karşılaştırılamaz, çünkü git yalnızca şifreli dosyaları görür.

### Ne değişti: `apix diff`

`apix diff` bir değişikliği gözden geçiren birinin okuyacağı gibi gösterir:
TOML satırları yerine isteğin alanları, header ve sorgu parametreleri adıyla,
gövde ve scriptler satır satır, ortamlar değişken değişken. Dosyayı yeniden
biçimlendirmek değişiklik sayılmaz; başka bir şeyi değişmeden taşınan ya da
yeniden adlandırılan istek tek bir taşıma olarak görünür.

```sh
apix diff                       # HEAD ile dosyaların şimdiki hâli
apix diff main                  # main ile şimdi
apix diff v1.2 v1.3 -C api      # iki etiket arası, api/ içindeki koleksiyon için
```

```text
Between main and the working tree: 1 script, 1 request

A  script  users/create-user.test.js
     + pm.test("ok", () => pm.response.to.have.status(200));
M  GET List users  users/list-users.apix
     query._limit: 3 → 5
     headers.Accept: (none) → application/json
```

`--markdown` pull request için, `--json` araçlar için yazar; `--exit-code` bir
şey değiştiyse 1 ile çıkar.

### Pull request'e özet

`--markdown report.md` koşuyu GitHub Markdown'ı olarak yazar: toplamlar,
istek başına bir satır ve altında açık hâlde neyin neden başarısız olduğu. İlk
satırı koleksiyonun adını taşıyan gizli bir işarettir; böylece bir CI adımı
geçen sefer yazdığı yorumu bulup yenisini eklemek yerine onu düzenler.

Action bunların hepsini yapar. Bir pull request'te yalnızca onun dokunduğu
istekleri gönderir, özetin altına `apix diff` ekler ve pull request'teki tek
bir yorumu güncel tutar:

```yaml
on: pull_request
permissions:
  contents: read
  pull-requests: write          # yorum için
jobs:
  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0        # dalın başladığı yeri bulmak için geçmiş
      - uses: 11R5/Apix@main
        with:
          path: api
          changed: "true"       # ya da bir commit, dal veya etiket
          diff: "true"
          comment: "true"
```

Bir push'ta `changed: "true"` push'tan önceki commit'le karşılaştırır. Özet
ayrıca `apix-report.md` dosyasına (girdi `markdown`) yazılır ve işin
sayfasında görünür.

Masaüstü uygulamasında durum çubuğu dalı ve son commit'ten beri kaç dosyanın
değiştiğini gösterir; kenar çubuğu değişen istekleri **M** (değişti) ya da
**A** (yeni) ile işaretler; bkz. [kullanım kılavuzu](user-guide.md#git).

### Docker

Diğer CI sistemleri için yalnızca `apix` komutunu içeren bir konteyner imajı
vardır (bir sürüm etiketlendiğinde `ghcr.io/11r5/apix` olarak yayımlanır;
kendin `docker build -t apix .` ile de derleyebilirsin):

```sh
docker run --rm -v "$PWD:/work" -e APIX_VAR_TOKEN ghcr.io/11r5/apix run api --env staging --junit reports/apix.xml
```

[examples/ci/gitlab-ci.yml](../../examples/ci/gitlab-ci.yml) ve
[examples/ci/azure-pipelines.yml](../../examples/ci/azure-pipelines.yml) aynısını
GitLab CI'da (imajla) ve Azure Pipelines'ta (`apix`'i bir kez derleyip
önbelleğe alarak) yapar; ikisi de JUnit sonuçlarını yayımlar.

Masaüstü uygulamasının **Runner**'ı (Ctrl+Shift+R) aynı koşuları bir
pencereyle yapar; bkz. [kullanım kılavuzu](user-guide.md#runner).

`apix send <file>` tek bir istek gönderir ve yanıtı yazdırır; başlıklar için
`-i`, bağlantı ayrıntıları için `--network` ekleyin.

## Bir API'yi monitörlerle izleme

Monitör, bir koleksiyonu (ya da onun bir klasörünü veya isteğini) zamanında
çalıştırır ve başarısız olmaya başladığında ve yeniden geçtiğinde bir
webhook'a haber verir. Monitörler koleksiyonun `apix.toml` dosyasına elle ya da
uygulamada **Görünüm › Monitörler…** ile yazılır (bkz.
[kullanım kılavuzu](user-guide.md#monitörler)):

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "health.apix"
environment = "production"
webhook = "{{alerts_url}}"
```

`apix monitor` onları açık kaldığı sürece, her birini kendi zamanında çalıştırır
ve `apix.toml`'u ara ara yeniden okur; yapılan değişiklikler geçerli olur:

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

| Seçenek | Ne yapar |
|---|---|
| `--once` | Her monitörü bir kez çalıştırıp çıkar, biri başarısızsa 1 döner: cron, Görev Zamanlayıcı ya da CI zamanlaması için |
| `--list` | Monitörler ve son sonuçları |
| `--only AD` | Yalnızca bu monitör (tekrarlanabilir); `--once` ile kapalıyken de çalışır |
| `--var`, `--env-file` | Monitörlerin ortamlarının üstüne değişkenler, `apix run`'daki gibi (`APIX_VAR_*` de okunur) |
| `--log-dir KLASÖR` | Günlüğü başka yerde tutar (CI'da önbelleğe alınan bir klasör) |
| `--timeout`, `-k`, `--proxy`, `--cacert` | Bağlantı ayarları, `apix run`'daki gibi |

Günlük, her monitörün son 500 sonucunu `<config>/Apix/monitors` içinde
tutar; masaüstü uygulaması aynı dosyaları okur, böylece aynı bilgisayarda
`apix monitor`'ün bulduğunu gösterir. Webhook'a yalnızca monitörün durumu
döndüğünde haber verilir (geçerken başarısız olunca ya da ilk koşusunda;
başarısızken geçince); gövde, Slack, Discord, Mattermost ve Google Chat'in
mesaj olarak gösterdiği bir JSON'dur.

Bir sunucuda `apix monitor api`'yi bir servis olarak başlatın ya da
`apix monitor api --once`'ı cron'a koyun (`*/5 * * * *`). GitHub'da bunu
zamanlanmış bir iş akışı yapar:
[examples/ci/monitors.yml](../../examples/ci/monitors.yml) monitörleri action
üzerinden (`monitor: "true"`) her 15 dakikada bir çalıştırır ve düzelmelerin
fark edilmesi için günlüğü önbellekte tutar.

## OAuth 2.0 ile oturum açma

**İstemci kimlik bilgileri** (servisten servise):

1. Yetkilendirme sekmesi › **OAuth 2.0**, grant türü **İstemci kimlik bilgileri**.
2. **Token URL'si**, **İstemci kimliği**, **İstemci gizli anahtarı** ve
   **Kapsam** alanlarını doldurun. Gizli değerleri doğrudan yazmak yerine
   değişken kullanın (`{{client_secret}}`, gizli olarak işaretli).
3. Gönderin. Apix bir token alır, ardından onu yeniden kullanır ve yeniler.

**PKCE ile yetkilendirme kodu** (bir kullanıcı oturum açar):

1. İstemciniz için yönlendirme URL'si olarak `http://127.0.0.1:<port>/callback`
   kaydedin (ya da sağlayıcı herhangi bir loopback portunu kabul ediyorsa
   **Yönlendirme URL'si** alanını boş bırakın).
2. Grant türü **Yetkilendirme kodu**; **Yetkilendirme URL'si**, **Token URL'si**,
   **İstemci kimliği** ve **Kapsam** alanlarını doldurun; **PKCE kullan**
   işaretli kalsın.
3. **Yeni token al**'a basın veya isteği gönderin. Tarayıcınız oturum açma
   sayfasını açar; oturum açtıktan sonra token Apix'te görünür.

OAuth 2.0'ı koleksiyonda bir kez ayarlayın (sağ tık › **Yetkilendirme…**);
yetkilendirmeyi devralan her istek aynı token'ı kullanır. Token'lar bellekte
kalır; Apix yeniden başlatıldığında yeniden oturum açılır.

## Bir AWS API'sini çağırma

1. Yetkilendirme sekmesi › **AWS Signature**.
2. **Erişim anahtarı** ve **Gizli anahtar** değişken olarak
   (`{{aws_access_key}}`, `{{aws_secret_key}}` gizli olarak işaretli), geçici
   kimlik bilgileri için **Oturum token'ı**, **Bölge** (örneğin `eu-central-1`)
   ve **Servis** (örneğin `execute-api`, `s3`, `lambda`).
3. Gönderin. Her istek, çıkmadan hemen önce Signature Version 4 ile imzalanır.

**Komut olarak kopyala › cURL** `curl --aws-sigv4` yazar; böylece curl onu
çalıştırdığınızda yeniden imzalar. PowerShell kopyası ise kopyaladığınız anda
oluşturulmuş, birkaç dakika geçerli bir imza içerir.

## Dosya yükleme, GraphQL gönderme

**Multipart:** Gövde › **Multipart**, bir alan ekleyin, onu **Dosya** olarak
değiştirin ve dosyayı seçin. Metin alanları **Metin** olarak kalır. Test
dosyalarını koleksiyonun içinde tutun: yol isteğe göre göreli kaydedilir, böylece
her makinede çalışır.

**İkili:** Gövde › **İkili** tek bir dosyayı gövdenin tamamı olarak gönderir.

**GraphQL:** Gövde › **GraphQL**, sorguyu yazın, değişkenleri altına JSON olarak
girin. Apix `{"query": …, "variables": …}` içeriğini
`Content-Type: application/json` ile gönderir. Önce yetkilendirmeyi ayarlayın,
sonra **Şemayı getir**'e basın: sorgu düzenleyicisi bundan sonra alanları,
argümanları ve enum değerlerini önerir, üzerine gelince açıklamalarını
gösterir. Şema adres başına saklanır; isteği bir dahaki açışınızda hazırdır.
Bir pull request'te şema değişikliğini görmek için
`apix schema api/graphql.apix > schema.graphql` çıktısını depoda tutun, farkı
diff göstersin.

## Bir gRPC servisini çağırma

1. **İstek › Yeni istek**, sonra adresi `grpc://` (TLS arkasındaysa
   `grpcs://`) ile yazın, örneğin `grpc://localhost:50051`.
2. **Mesaj** sekmesinde **Metotları yükle**'ye basın. Sunucu reflection'ı
   kapalıysa önce `.proto` dosyalarını ekleyin (**Dosya ekle**); yolların her
   makinede çalışması için onları koleksiyonun içinde tutun.
3. Metodu seçin, **Örnek mesaj**'a basın, değerleri doldurun.
4. Token'ı **Yetkilendirme**'de (Bearer) verin ya da **Başlıklar**'a metadata
   ekleyin, sonra **Gönder**.

Bir pipeline'da aynı dosya komut satırıyla çalışır:

```sh
apix send api/say-hello.apix --env staging   # durum OK değilse çıkış kodu 1
apix schema api/say-hello.apix > greeter.txt   # servisin sundukları, incelemede diff için
```

## Tarayıcıdaki bir isteği yeniden oluşturma

1. Tarayıcının geliştirici araçlarında bir isteğe sağ tıklayıp › **Copy as cURL**
   seçin (bash veya cmd, ikisi de çalışır).
2. Apix'te URL alanına tıklayıp yapıştırın. Metot, URL, sorgu, başlıklar,
   çerezler, yetkilendirme ve gövde doldurulur.
3. Gönderin, değiştirin, kaydedin.

Tersi yönde, **Gönder** düğmesinin yanındaki kopyala düğmesi isteği bir
terminale veya hata raporuna yapıştırmak üzere `curl`, `curl.exe` ya da
PowerShell olarak verir.

## Bir istek neden yavaş ya da başarısız?

1. İsteği gönderin ve yanıtın **Ağ** sekmesini açın.
2. Zamanlama çubuğu sürenin DNS'e mi, TCP bağlantısını açmaya mı, TLS el
   sıkışmasına mı, sunucuyu beklemeye mi yoksa indirmeye mi gittiğini
   gösterir. Yavaş bir TCP bağlantısı ağa işaret eder; yavaş bir el sıkışma
   (bir gidiş-dönüş süren TCP ile karşılaştırıldığında) sunucuya ya da TLS
   1.2'nin fazladan gidiş-dönüşüne.
3. Uzak adresi ve yerel arayüzü kontrol edin: bir VPN veya proxy orada görünür.
4. Yönlendirmeler sırayla listelenir. Şifreleme altında TLS sürümü, şifre
   takımı ve HTTP/2'de anlaşılıp anlaşılmadığı görünür; sertifika zinciri her
   sertifikanın vereni ve kalan gününü gösterir, böylece eksik bir ara
   sertifika ya da süresi dolmak üzere olan biri göze çarpar. Sıkıştırılmış
   bir gövde ne kadar kazandırdığını gösterir.

Aynı rapor bir terminalde de alınabilir:

```sh
apix send my-api/health.apix --network
```

## Bir yapay zekâ ajanına koleksiyonu kullandırma (MCP)

`apix mcp` bir koleksiyonu Model Context Protocol üzerinden sunar; böylece
bir yapay zekâ ajanı (Claude, bir editörün asistanı, herhangi bir MCP
istemcisi) API'nizle sizin Apix'te çalıştığınız gibi çalışabilir. MCP'yi
standart girdi ve çıktı üzerinden konuşur; onu ajan başlatır. Claude Code ile:

```bash
claude mcp add my-api -- apix mcp /path/to/my-api
```

Öteki istemciler aynı komutu kendi ayarlarında alır, örneğin:

```json
{ "mcpServers": { "my-api": { "command": "apix", "args": ["mcp", "/path/to/my-api"] } } }
```

Ajan dört araç alır:

| Araç | Ne yapar |
|---|---|
| `list_requests` | istekler (yol, yöntem, ad, `{{değişkenler}}`'iyle URL); bütün koleksiyonun ya da bir klasörün |
| `list_environments` | ortamlar ve değişkenlerinin adları; değerleri asla |
| `send_request` | bir isteği scriptleri ve testleriyle, bir ortamda, ajanın verdiği değişkenlerle gönderir; durumu, süreyi, başlıkları, gövdeyi (64 KB'a kadar) ve test sonuçlarını döndürür |
| `run_requests` | bir klasörü ya da bütün koleksiyonu testleriyle çalıştırır ve neyin geçtiğini bildirir |

Yalnızca o koleksiyonun içindeki isteklere ulaşılabilir. `--list-only` iki
listeleme aracını bırakır, gönderenleri kaldırır. Gizli değerler yerinde
kalır: ajan bir isteğin token'la gönderilmesini isteyebilir ama token'ın
değeri ona asla gösterilmez (onu geri yansıtan bir yanıt başka konu; ajana
neyi gönderttiğinize dikkat edin). Şifreli bir koleksiyon için parolayı
`APIX_PASSWORD` ile verin.
