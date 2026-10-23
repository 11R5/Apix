<!-- translated from en/architecture.md sha256:b09ada03e09160cb -->

# Apix mimarisi

Apix, Rust ile yazılmış native bir API istemcisidir. Arayüz,
[GPUI](https://github.com/zed-industries/zed) (Zed'in arayüz motoru) ve
[gpui-kit](https://github.com/longbridge/gpui-kit) bileşen kütüphanesiyle GPU
üzerinde çizilir. WebView ya da Electron kullanılmaz.

## İlkeler

1. **Önce yerel, git dostu.** Bir koleksiyon, küçük TOML dosyalarından oluşan bir
   klasördür. Hesap ya da bulut zorunluluğu yoktur. Koleksiyonlar kod gibi
   commit'lenir, diff'lenir ve gözden geçirilir.
2. **Arayüz asla bloklanmaz.** Ağ, disk, ayrıştırma ve betik çalıştırma UI
   thread'inin dışında yürür. UI thread'i yalnızca yerleşim (layout) ve çizim yapar.
3. **Tek motor, çok önyüz.** Masaüstü uygulaması, `apix` CLI'ı ve gelecekteki
   önyüzler (bir TUI, bir MCP sunucusu) aynı crate'leri paylaşır; böylece bir istek
   CI dahil her yerde aynı davranır.
4. **Devasa yanıtlara ölçeklenme.** 100 MB'lık bir JSON yanıtı akıcı biçimde
   kaydırılabilmelidir. Yalnızca görünen satırlar yerleştirilir (sanal listeler) ve
   gövde tembel (lazy) ayrıştırılır.
5. **Hafif ve hızlı açılış.** Hız ve özellikler boyuttan önce gelir. Release
   binary'si 100 MB'ın altında kalır (bugün 33.1 MB; Electron uygulamaları 300 MB+)
   ve pencereyi 150 ms'den kısa sürede açar.

## Crate'ler

```
crates/
├── apix-core     istek modeli, {{variables}} ve {{$dynamic}} değerleri, disk formatı,
│                 projeler, ortamlar ve gizli değerler, ağaç ve dosya işlemleri
├── apix-http     HTTP motoru: reqwest + rustls, zamanlamalar, yanıt yakalama, ağ izi
│                 (DNS, bağlantı kurulumu, yönlendirmeler, sertifika, yerel ağ arayüzü)
├── apix-import   Postman v2.0/v2.1 koleksiyonları ve ortamları → Apix koleksiyonları
├── apix-codegen  istekler ↔ kabuk komutları: curl (bash), curl.exe (cmd), PowerShell; yapıştırılan curl'ü ayrıştırır; 10 dilde kod
├── apix-vault    şifreli koleksiyonlar: dosya başına XChaCha20-Poly1305, Argon2id, kurtarma anahtarı
├── apix-script   istek öncesi ve test scriptleri: pm.* uyumlu API ile QuickJS
├── apix-cli      `apix` binary'si: terminalden ve CI'dan send / run / import / shell
└── apix-desktop  GPUI masaüstü uygulaması (tek crate'ten Windows, macOS ve Linux)
tools/
└── icongen       tüm uygulama ikonlarını (ico, icns, hicolor PNG'ler) Logo/LogoFirst.jpg'den üretir
```

`apix-desktop/src` içinde: `workspace.rs` (istek düzenleyici, yanıt paneli,
eylemler ve tuş bağlamaları), `workspace/collections.rs` (kenar çubuğu, projeler,
dosya işlemleri, içe aktarma, dosya izleme), `workspace/env_editor.rs`
(değişkenler penceresi), `workspace/auth_editor.rs` (Yetkilendirme sekmesi ve
klasör/koleksiyon/proje yetkilendirme penceresi), `workspace/palette.rs` (komut
paleti ve bulanık (fuzzy) eşleştiricisi), `workspace/network.rs` (yanıtın Ağ
sekmesi), `workspace/vault_ui.rs` (kilit açma, kilitleme ve otomatik kilit,
şifreleme, şifrelemeyi kaldırma, parola değiştirme, kurtarma anahtarı),
`assets.rs` (gpui-kit ikonları ve iki kilit ikonu), `workspace/menu_bar.rs`
(başlık çubuğundaki Dosya/İstek/Ortam/Görünüm/Yardım; düğmeler başlık çubuğunun
sürükleme alanını `occlude()` ile örter, böylece Windows tıklamayı onlara
iletir), `workspace/tabs.rs` (açık istekler; öndeki sekme düzenleyici
widget'larında yaşar, diğerleri kendi taslağını, yanıtını ve süren görevini
tutar), `workspace/drag.rs` (istekleri ve klasörleri taşımak için sürükleme),
`workspace/shell_copy.rs` (farklı kopyala / curl yapıştır), `workspace/var_chips.rs`
(URL çubuğundaki `{{variable}}` çipleri ve üzerine gelince açılan kartları),
`large_view.rs` (devasa gövdeler için satır tabanlı görüntüleyici), `net.rs`
(tokio köprüsü), `bidi.rs` (sağdan sola yerleşim yardımcıları, bkz. Diller), `state.rs` (kullanıcının yapılandırma klasöründe son kullanılan
öğeler ve klasör başına ortam), `theme.rs` ve `fonts.rs`.

Planlanan crate'ler:

| Crate          | Amaç                                                               |
|----------------|--------------------------------------------------------------------|
| `apix-script`  | QuickJS üzerinde `rquickjs` ile istek öncesi ve test betikleri (`pm.*` uyumlu API) |
| `apix-proto`   | GraphQL, WebSocket (`tokio-tungstenite`), gRPC (`tonic` + reflection), SSE |
| `apix-mock`    | Kaydedilmiş örnekleri sunan yerel bir mock sunucu (`hyper`)        |

### Üç işletim sistemi için tek masaüstü crate'i

GPUI ve gpui-kit aynı kodla Windows, macOS ve Linux'ta çalışır; Zed de bu şekilde
dağıtılır. Bu yüzden işletim sistemine özel arayüz klasörleri yoktur. Gerçekten
işletim sistemine göre değişen kod,
`crates/apix-desktop/src/platform/{windows,macos,linux}.rs` içinde `cfg` arkasına
konur: başlık çubuğu ve pencere kontrolleri, blur/vibrancy, macOS menü çubuğu,
`.apix` için dosya ilişkilendirmeleri ve anahtar zinciri (keychain) erişimi.
Kurulum paketleri ve ikonlar `packaging/{windows,macos,linux}/` altındadır. Ayrı
bir arayüz ancak bir işletim sistemi bir gün gerçekten farklı bir arayüz
gerektirirse ayrılır.

Bağımlılık yönü: `core` ← `http`/`script`/`proto` ← `cli`/`desktop`.
Hiçbir şey `desktop`'a bağımlı değildir.

## Bir isteğin veri akışı

```
.apix file ──load──▶ Request ──resolve(Variables)──▶ Request (resolved)
                                                        │
                     istek öncesi betik (ileride) ◀─────┤
                                                        ▼
                                         apix-http::HttpClient::send
                                                        │  (tokio runtime, arka plan thread'i)
                                                        ▼
                    Response {status, headers, body: Bytes, timing}
                                                        │
                         test betiği (ileride) ◀────────┤
                                                        ▼
                Arayüz: durum satırı, sanallaştırılmış gövde görüntüleyici
```

`{{variables}}` katmanlar hâlinde çözümlenir. Sonraki katmanlar kazanır:
**global değişkenler → koleksiyon (`apix.toml`) → ortam → geçersiz kılmalar (CLI `--var`, betik `pm.variables.set`)**.
Tanımsız adlar olduğu gibi bırakılır ve raporlanır; böylece arayüz bunların altını
çizebilir, CLI ise göndermeyi reddedebilir.

## Disk formatı

```
my-api/
├── apix.toml               ad + koleksiyon düzeyinde [vars] ve [auth]
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── folder.toml         isteğe bağlı: bu klasördeki istekler için [auth]
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

**Yetkilendirme devralma.** `auth` içermeyen bir istek, yetkilendirmeyi
`folder.toml` dosyasında bir yetkilendirme ayarlayan en yakın klasörden, sonra
koleksiyondan (`apix.toml`), sonra projeden (`apix-project.toml`) devralır;
hiçbiri yoksa hiçbir şey gönderilmez. Bir istekte (veya klasörde)
`auth = "none"` devralmayı durdurur. Bu aramayı masaüstü uygulaması, CLI ve
"Komut olarak kopyala" için aynı şekilde `apix_core::project::inherited_auth` yapar.

**İstek ayarları.** Aynı dört düzeyde bir `[settings]` tablosu bulunur (zaman
aşımı, yönlendirmeler, TLS doğrulama, proxy, istemci sertifikası, çerezler).
Her alan bir `Option`'dır: `None`, aynı zincir üzerinden alan alan devralır
(`apix_core::project::inherited_settings`); yine boş kalanlar
`RequestSettings::effective` içindeki varsayılanlara düşer — zaman aşımı yok,
en çok 10 yönlendirme izlenir, TLS doğrulanır, proxy ortamdan, çerezler açık.
Motor sonucu her gönderimin `reqwest::Client`'ını kurarken uygular; paylaşılan
çerez kavanozu (çerez yöneticisinin listeleyip düzenleyebildiği bir
`cookie_store`) istek çerezleri kapatmadıkça o istemciye verilir.

**İmzalı yetkilendirme.** AWS Signature V4, OAuth 1.0a ve Digest,
`apix_core::sign` içinde saf fonksiyonlardır; AWS test paketine, RFC 7616'ya,
RFC 7636'ya ve klasik OAuth 1.0 örneğine karşı doğrulanır, böylece motor ve
"Komut olarak kopyala" aynı şekilde imzalar. Motor önce `reqwest::Request`'i oluşturur,
sonra onu imzalar (AWS payload hash'i son gövdeye ihtiyaç duyar; multipart akışı
`UNSIGNED-PAYLOAD` olarak gönderilir). Digest bir kez gönderir, 401 yanıtındaki
sınamayı (challenge) okur ve aynı bağlantı üzerinden yanıtlar. OAuth 2.0
`apix_http::oauth2` içindedir: token göndermeden önce alınır (istemci kimlik
bilgileri, parola veya PKCE ile yetkilendirme kodu; bu sonuncusunda tarayıcı
127.0.0.1 üzerindeki tek seferlik bir dinleyiciye geri döner), sunucu, istemci,
grant, kapsam ve kullanıcı başına bellekte tutulur, refresh token'ı ile yenilenir
ve asla diske yazılmaz.

Bir istek dosyası:

```toml
name = "Create user"
method = "POST"
url = "{{base_url}}/users"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false          # dosyada tutulur, gönderilmez

[body]
json = '''
{ "name": "Ada" }
'''

[auth.bearer]
token = "{{token}}"
```

**Gövdeler.** `[body]` şunlardan birini içerir: `json`, `text`, `xml`, `form`
(URL-encoded satırlar), `multipart` (dosya alanları için `file = true` içeren
satırlar ve isteğe bağlı bir `content_type`), `binary = { file = "…" }` ya da
`graphql` (`query` ve `variables`). Dosya yolları değişken içerebilir ve istek
dosyasına göre görelidir; `Request::with_files_relative_to`, değişkenler
çözümlendikten sonra hem uygulamada hem CLI'da bunları mutlak yola çevirir. Motor
dosyaları yalnızca gönderirken okur; bu yüzden eksik bir dosya yükleme hatası
değil, gönderim hatasıdır. GraphQL gövdesi `{"query", "variables"}` JSON'u olarak
gönderilir; geçersiz değişkenler gönderimi durdurur. Multipart gövdede, motorun
boundary'sini içermeyen bir `Content-Type: multipart/…` başlığı (Postman dışa
aktarımlarında yaygındır) atılır, böylece gövde okunabilir kalır.

Açık sorular:
- **Gizli değerler.** Token gibi ortam değerleri commit'lenmemelidir.
  `environments/<name>.local.toml`, `<name>.toml` üzerine katmanlanır. Böyle bir
  dosya yazıldığında, eksikse `environments/` yanındaki `.gitignore` dosyasına
  `*.local.toml` eklenir. Bir gizli değer paylaşılan dosyada bir yer tutucuyla
  kalabilir; böylece ekip arkadaşları adını görür. İstenirse (Ayarlar › Ortam
  gizli değerleri) gizli değerler işletim sistemi anahtarlığına gider:
  `<name>.local.toml` yalnızca `keychain = "<kimlik>"` satırını tutar, değerler
  `Apix` servisindeki `env:<kimlik>` kaydında durur
  (`apix_vault::keychain::save_text`; Windows'un 2560 baytlık sınırı için
  birden çok kayda bölünür). Yeni gizli değerlerin nereye yazılacağına
  `apix_core::secrets::use_keychain` karar verir; okuma dosyayı izler, bu yüzden
  CLI'nin bir ayara ihtiyacı yoktur. Kimlik dosyayla birlikte taşındığından
  ortamı yeniden adlandırmak ya da klasörü taşımak bağlantıyı bozmaz. Şifreli
  koleksiyonlarda gizli değerler kasada kalır.
- **Dinamik değişkenler.** `{{$name}}`, bu adla tanımlı bir değişken yoksa
  `apix_core::dynamic` tarafından doldurulur; her geçişte yeni bir değer üretilir
  (`$guid`, `$timestamp`, `$isoTimestamp`, `$randomEmail`, …). Ek crate yok:
  küçük bir xorshift üreteci ve elle yazılmış bir UTC tarih biçimlendiricisi.
- **Kenar çubuğunda sıralama.** Kaydedilmiş bir şey yoksa önce klasörler, sonra
  istekler gelir; her biri ada göre. Sürüklemek, klasörün `folder.toml`
  dosyasına (ya da koleksiyonun `apix.toml` dosyasına) alt öğe adlarından oluşan
  bir `order` listesi kaydeder; `store::place` bir öğeyi diğerinin önüne ya da
  arkasına koyar, yeniden adlandırma ve taşıma listeyi günceller. Listenin
  bilmediği adlar varsayılan sırayla arkadan gelir, böylece yeni bir dosya hiç
  kaybolmaz. Projedeki koleksiyonlar `apix-project.toml` içindeki `order`
  alanını kullanır (`Project::place_collection`). Postman içe aktarıcısı
  `order` alanını yalnızca Postman'ın sırası varsayılandan farklıysa yazar.

### Projeler

Bir proje koleksiyonları gruplar ve hepsinin paylaştığı ortamlara sahiptir:

```
shop/
├── apix-project.toml       ad, paylaşılan [vars], koleksiyon sırası, group_by, pins, colors
├── environments/           test.toml, prod.toml (+ *.local.toml gizli değerleri)
├── users/                  bir koleksiyon (apix.toml + .apix dosyaları)
└── payments/               bir koleksiyon; bazı ortamlara sabitlenebilir
```

`[colors]` bir koleksiyon klasörünü bir renk adıyla eşler (`red`, `orange`,
`yellow`, `green`, `teal`, `blue`, `purple`, `pink`); uygulama bunu kenar
çubuğunda ve sekmelerde nokta olarak çizer. `project::move_collection_into` bir
koleksiyon klasörünü projeye taşır (farklı sürücüde kopyalayıp siler).

Değişken katmanları, sonraki kazanır: proje değişkenleri → koleksiyon değişkenleri
→ projenin ortamı → koleksiyonun aynı adlı ortamı (her biri kendi `.local.toml`
gizli değerleriyle). CLI (`apix run --env`) ve masaüstü uygulaması aynı fonksiyonu
kullanır: `apix_core::project::variables`. `group_by = "environment"` kenar
çubuğunu Proje → Ortam → Koleksiyonlar şeklinde düzenler; `[pins]` ise bir
koleksiyonu bazı ortamlarla sınırlar.

### Şifreli koleksiyonlar (M4)

Format ve motor `crates/apix-vault` içindedir. Apix'in geri kalanı bunlara,
`std::fs` ile aynı çağrılara sahip ince bir katman olan `apix_core::vfs`
üzerinden erişir: kilidi açık bir vault'un kökü altındaki bir yol, bellekte şifresi
çözülerek vault'tan sunulur; diğer her yol diske gider. Böylece store, kenar
çubuğu, sekmeler ve CLI sıradan yolları kullanmaya devam eder ve şifresi
çözülmüş hiçbir şey diske yazılmaz. Kilidi açık vault'lar süreç genelindeki bir
registry'de tutulur; kilitleme (elle, 15 dakika boşta kalındıktan sonra veya
çıkışta) vault'u bırakır ve anahtarını siler. Şifreli bir koleksiyonun içinde
silme kalıcıdır: sistem çöp kutusu şifresi çözülmüş dosyaları tutardı.

```
my-api/
├── apix-vault.toml   düz metin üst bilgi: format, Argon2id maliyeti ve salt'lar,
│                     parola ve kurtarma anahtarıyla sarılmış veri anahtarı
├── .gitignore        objects/local/
└── objects/
    ├── index.apixe   her dosyanın gerçek yolu, şifreli
    ├── 3f9c…e1.apixe her biri tek bir şifreli dosya; rastgele adlar, düz (klasör yapısı yok)
    └── local/        *.local.toml gizli değerleri, aynı format, asla commit'lenmez
```

- Rastgele 256 bitlik bir **veri anahtarı** her dosyayı XChaCha20-Poly1305 ile ve
  her yazımda yeni bir nonce ile şifreler. Nesne kimliği ilişkili veri (associated
  data) olarak kullanılır; böylece blob'lar fark edilmeden yer değiştirilemez
  veya yeniden adlandırılamaz.
- Veri anahtarı iki kez saklanır: **paroladan** türetilen bir anahtarla
  (Argon2id, 64 MiB, 3 geçiş) ve **kurtarma anahtarından** türetilen bir anahtarla
  (160 rastgele bit, bir kez `XXXX-XXXX-…` biçiminde Crockford base32 olarak
  gösterilir) sarılmış olarak. Parolayı değiştirmek anahtarı yeniden sarar;
  dosyalar yeniden şifrelenmez.
- Yeniden adlandırma ve taşıma yalnızca şifreli dizine dokunur.
- Kilidi açılmış anahtar yalnızca bellekte yaşar (`zeroize` kilitlemede onu siler).
- Git yine de hangi nesnenin değiştiğini görür, ama içinde ne olduğunu asla
  görmez. Güvenlik formatın gizli olmasından değil, şifreleme algoritmasından ve
  paroladan gelir; Apix açık kaynaktır.
- Parola işletim sisteminin kimlik bilgisi deposunda hatırlanabilir
  (`apix_vault::keychain`, `keyring` crate'i üzerinden: Credential Manager,
  macOS Anahtar Zinciri, Secret Service). Her kasa için `Apix` servisi altında
  bir kayıt tutulur; adı `vault:<mutlak klasör yolu>` (Windows'ta küçük harf).
  Kilit açma penceresinden isteğe bağlı açılır; uygulama ve CLI sormadan önce
  onu dener, kasayı artık açmayan bir parola silinip yeniden sorulur, parola
  değişince kayıt güncellenir, şifre kaldırılınca silinir. Depo yoksa Apix
  eskisi gibi parolayı sorar.

## Scriptler (M5)

`apix_core::scripts` bir isteğin scriptlerini bulur (`_collection.<olay>.js`,
her klasörün `_folder.<olay>.js` dosyası, sonra `<istek>.<olay>.js`; olaylar
`prerequest` ve `test`) ve scriptlerin kapsam kapsam atadığı ya da sildiği
değerleri `ScriptVars` içinde tutar. `ScriptVars::apply` bunları isteğin
`Variables` katmanlarının üstüne koyar, böylece çözümleme eskisi gibi çalışır.
Scriptlerin atadığı hiçbir şey diske yazılmaz.

`apix-script` bir olayın scriptlerini yeni bir QuickJS çalışma zamanında
çalıştırır (`rquickjs`; 10 sn ve 256 MB sınırı). `pm` nesnesi JavaScript ile
yazılmıştır (`prelude.js`): Rust bağlamı JSON olarak verir (çözülmemiş istek,
testler için yanıt, `project::scope_values` ile kapsam değerleri, önceki
`ScriptVars`, `pm.info`), scriptler birer fonksiyon içine sarılıp sırayla
çalışır ve `__apix_output()` değişen isteği ve değişkenleri, test sonuçlarını,
konsol satırlarını, `setNextRequest` ve `skipRequest` bilgisini JSON olarak
döndürür. API'nin JavaScript'te durması Rust tarafını birkaç çağrıya indirir ve
Postman davranışını yakalamayı kolaylaştırır.

Scriptli bir gönderim: istek öncesi scriptler, sonra `ScriptVars::apply` ve
çözümleme, HTTP isteği, ardından `ResponseView` ile test scriptleri. CLI bunu
`exchange` içinde yapar (durum `apix run` boyunca taşınır); masaüstü uygulaması
`script_run.rs` içinde, scriptleri arka plan thread'inde çalıştırarak ve
isteğin kendi scriptlerini kaydedilmiş olsun olmasın editörden alarak yapar.

## Diller (M4)

Masaüstü arayüzündeki her metin bir anahtardan geçer: `tr!("menu.file")`
(`rust_i18n::t!` üzerine kurulu, gösterilecek bir `SharedString` döndüren ince
bir sarmalayıcı; kaydedilen metinler, örneğin yeni bir isteğin varsayılan adı,
için aynı metni `tr_plain!` verir).
rust-i18n, gpui-kit'in kullandığı sistemin aynısıdır; bu yüzden ikisi de süreç
genelinde tek bir locale'i izler. Çince için gpui-kit'in kendi metinlerinin
kullandığı kod olan `zh-CN` kullanılır. Her dil tek bir düz dosyadır:
`crates/apix-desktop/locales/<code>.yml` (satır başına bir `key: value`, sıralı,
`%{name}` yer tutucuları); binary'ye derlenir ve bir dosya değiştiğinde
`build.rs` yeniden derler. Eksik bir metin İngilizceye düşer; bir dilde bir
anahtar eksikse, bilinmeyen bir anahtar varsa ya da İngilizceden farklı yer
tutucular kullanılıyorsa bir birim testi başarısız olur. Cümleler asla
parçalardan yapıştırılmaz, yer tutuculu bütün anahtarlardır; çünkü sözcük sırası
diller arasında farklıdır.

Dil Ayarlar'da seçilir (veya ilk açılışta sistem dilini izler) ve anında değişir:
metinler her render'da okunur, widget'ların içinde tutulan birkaç metin (input
yer tutucuları, seçici öğeleri) ise `Workspace::retranslate` tarafından yeniden
ayarlanır. Japonca, Çince ve Arapça glifleri, gömülü Inter'in yanında platformun
font fallback mekanizmasından (DirectWrite, Core Text, fontconfig) gelir. Arapça
metin sağdan sola şekillendirilir ve yerleştirilir; Windows'ta bunun için glyph
run yerleşimi düzeltilmiş gpui 0.3.9 DirectWrite backend'i olan
`vendor/gpui-pre-windows` gerekir (workspace `Cargo.toml` üzerinden patch'lenir;
upstream düzelttiğinde kaldırılacak). gpui sağdan sola metni satıra
kaydıramaz: bir paragrafı tek satır olarak şekillendirir ve soldan sağa arttığını
varsaydığı glif x konumlarından keser. Bu yüzden satıra kayabilecek metinler
(notlar, ipuçları, açıklamalar, iletiler) `bidi::para` üzerinden geçer; o da
RTL'de metni okuma sırasıyla boşluklardan böler, her aday satırı ölçer ve her
satırı kaydırmadan ayrı çizer (`src/bidi/paragraph.rs`). Diğer dillerde düz gpui
metnidir. gpui her satırı ayrıca soldan sağa temel yönle yerleştirir; bu yüzden
RTL bir dilde `tr!` her metni sağdan sola gömmeyle (U+202B…U+202C; DirectWrite
yeni yalıtım karakterlerini yok sayar) sarar. Böylece Latin bir sözcükle
("URL", "OAuth 2.0", "Ctrl+S") başlayan ya da biten bir cümle de sağdan sola
okunur; durum çubuğu gibi birleştirilmiş metinler için aynısını `i18n::display`
yapar. Bu işaretler dosyalara asla ulaşmaz: kaydedilen metinler `tr_plain!`
kullanır. Motor crate'lerinden gelen hata iletileri İngilizce kalır.

Sağdan sola bir dilde (`Language::rtl`, Arapça) pencere aynalanır: kenar çubuğu
sağa, yanıt paneli istek panelinin soluna, pencere düğmeleri sola geçer; menüler,
sekme şeritleri, form satırları ve durum çubuğu sağdan başlar, metin sağa
hizalanır. GPUI'de yerleşim yönü yoktur, bu yüzden bunu `src/bidi.rs` yapar:
stil alan her öğe için bir `Bidi` trait'i (`dir_row` bir satırı ters çevirir;
`text_start` / `text_end`, `ps` / `pe` dolguları ve `border_e_1` tarafı yöne göre
seçer; `ltr` metni sola sabitler), gpui-kit'in `TabBar`'ı için `tab_bar` /
`fill_tabs` (sekmeler ters sırayla verilir, indeksler geri eşlenir) ve diyaloglar
için `bidi_title` (RTL'de kapatma düğmesi başlık satırının soluna taşınır).
Yardımcılar dili görünüm render edilirken okur, böylece dil değişince pencere
anında aynalanır. Kod ve yanıt görüntüleyicileri, URL çubuğu, anahtar–değer
tabloları (parametreler, başlıklar, form alanları, değişkenler) ve yanıt
başlıkları her dilde soldan sağa kalır. gpui-kit'in açılır menüleri ve düğmeleri
kendi soldan sağa iç yerleşimlerini korur (kısayol sağda, ikon etiketin önünde);
menüler düğmelerinden sola doğru açılır.

Dil eklemek: `locales/en.yml` dosyasını `locales/<code>.yml` olarak kopyalayın,
değerleri çevirin ve `src/i18n.rs` içindeki `LANGUAGES` listesine bir satır ekleyin.

Kullanıcı belgeleri plandan ayrı, her dil için bir klasörde durur
(`Docs/<code>/`: kullanıcı kılavuzu, mimari, teknik, iş akışları, görseller) ve
`Docs/languages.toml` içinde listelenir. Kaynak İngilizcedir; her çeviri hangi
İngilizce sürümden yapıldığını kaydeder, böylece eskimiş çeviriler ortaya çıkar.

## Thread modeli (masaüstü)

- Ana thread GPUI'ye aittir; olay döngüsünü ve render'ı o çalıştırır.
- Tek bir çok thread'li **tokio runtime**, bir arka plan thread'inde bir kez
  başlatılır. reqwest, tonic ve tungstenite buna ihtiyaç duyar.
- Arayüz işi tokio'ya gönderir ve sonucu bir GPUI görevinde (`cx.spawn_in`)
  alır. Bu görev bırakıldığında (drop) istek iptal edilir; böylece Esc ve yeniden
  gönderme temiz biçimde iptal eder.
- Yanıt geldikten sonra gövde, arayüz onu görmeden önce GPUI'nin arka plan
  executor'ında biçimlendirilir ve indekslenir. Biçimlendirme, ağaç kurmayan ve
  sayıları tam gönderildiği gibi koruyan tek geçişli bir yeniden girintileyici
  olan `apix_core::text::pretty_json` ile yapılır.

## Büyük yanıt görüntüleyici

512 KB ve üzeri gövdeler kod düzenleyici yerine `large_view.rs` kullanır:

- `apix_core::text::RowIndex`, her görüntü satırının nerede başladığını satır
  başına 8 bayt ile kaydeder. 2 KB'tan uzun satırlar (minify edilmiş JSON)
  birkaç satıra bölünür.
- Kaydırma konumu piksel ofseti değil, bir **satır numarasıdır**. Satır başına
  20 px ile on milyon satır yaklaşık 200 milyon px eder; bu, `f32` piksellerin
  tam olduğu aralığın çok ötesindedir. Görüntüleyici yalnızca ekrana sığan ~120
  satırı yerleştirir. Fare tekerleğini, kendi kaydırma çubuğunu ve klavyeyi
  (↑ ↓ PgUp PgDn, Ctrl+Home/End) kendisi yönetir.
- Büyük bir JSON gövdesinin Ham sekmesi yalnızca açıldığında oluşturulur.

Windows'ta release derlemede, yerel bir sunucudan gelen 100 MB minify edilmiş
JSON için ölçülmüştür (`cargo run -p apix-http --release --example big_json_server`):
yanıt, Enter'dan yaklaşık 0.33 s sonra ekrandadır. Bu süre aktarımı, 9.6 milyon
satıra biçimlendirmeyi, indekslemeyi ve ilk kareyi kapsar. Bellek, gövde
yüklüyken yaklaşık 407 MB'tır; boştayken 69 MB.

## Performans bütçesi

| Metrik                              | Hedef             | Ölçülen (Windows, release)  |
|-------------------------------------|-------------------|-----------------------------|
| Soğuk başlangıçtan ilk kareye       | < 150 ms          | 274 ms (`main`'den ilk kareye) |
| Boştaki RAM                         | < 60 MB           | 83 MB working set, örnek koleksiyon açık |
| Release binary'si                   | < 100 MB          | 33.1 MB |
| Girdiden çizime gecikme             | 1 kare (120 Hz'de ≤ 8 ms) | ölçülmedi |
| 100 MB JSON gövdesini açma ve kaydırma | kare düşmesi yok | gönderdikten 325 ms sonra ekranda (yerel sunucu); kare süreleri ölçülmedi |

Başlangıç süresi ve boştaki bellek bugün bütçenin üzerinde; bunların profilinin
çıkarılması planlanıyor. Rakamlar `scripts/measure.ps1`'den gelir (bkz.
[teknik notlar](technical.md#performans-ölçümü)).

Release profili zaten fat LTO, tek codegen unit ve strip edilmiş semboller
kullanır (bkz. kök `Cargo.toml`).
