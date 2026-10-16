# Apix İş Planı

> Bu belge Apix'in ayrıntılı ve Türkçe iş planıdır. Özellikler tek tek yazılmıştır.
> Özet İngilizce yol haritası [ROADMAP.md](ROADMAP.md), teknik mimari [ARCHITECTURE.md](ARCHITECTURE.md) dosyasındadır.
> Tasarım referansı: [design/mockup-v1.webp](design/mockup-v1.webp).

**İşaretler:** ✅ yapıldı · 🔜 sıradaki · ⬜ planlandı
**Boyut:** S = birkaç saat · M = 1–3 gün · L = 1 hafta ve üstü

---

## 0. Vizyon ve kurallar

Apix, Postman'in yaptığı her şeyi yapan bir API istemcisi olacak. Postman'den daha hızlı, daha küçük ve daha sade olacak.

| Kural | Ne anlama geliyor |
|---|---|
| **Minimalist** | Ekranda sadece o an gereken şey görünür. Gerisi Ctrl/⌘+K komut paletinde durur. |
| **Tiny** | Kurulum dosyası 15 MB'ın altında kalır. Açılış 150 ms'nin, boşta bellek kullanımı 60 MB'ın altında kalır. |
| **Hızlı** | Arayüz hiç donmaz. 100 MB'lık bir yanıt bile kare kaybetmeden kaydırılır. |
| **Local-first** | Hesap ve bulut zorunlu değildir. Koleksiyonlar düz dosya olarak durur ve git ile versiyonlanır. |
| **Postman'den kolay geçiş** | Postman koleksiyonları, ortamları ve scriptleri içe aktarılınca çalışır. |
| **Dürüst arayüz** | Ekrandaki her etiket gerçeği söyler. Desteklenmeyen bir özellik "hazır" diye gösterilmez. |

---

## 1. Tasarım dili (Gemini mockup'ı temel alınarak)

### 1.1 Mockup'tan alınanlar
- **Cam görünüm.** Koyu lacivertten petrol mavisine geçen gradyan bir arka plan var, üstündeki paneller yarı saydam.
- **Çok ince ayraçlar.** Paneller kalın kutular yerine 1 px'lik yarı saydam çizgilerle ayrılır.
- **Renkli metot rozetleri.** POST, GET, DELETE, PUT, gRPC ve GQL için her protokolün kendi rengi var.
- **Yapı.** Üstte komut çubuğu, solda koleksiyon ağacı, ortada istek editörü, sağda yanıt paneli, altta durum çubuğu.
- **Sekmeler.** İstek tarafında Params · Headers · Auth · Body · Pre-request & Tests. Yanıt tarafında Pretty · Timing · Tests · Code.
- **Sayı rozetleri.** Sekme başlıklarında `Params 2`, `Headers 3` gibi sayılar görünür.
- **Değişken vurgusu.** URL içindeki `{{base_url}}` ayrı bir çip olarak çizilir. Fareyle üstüne gelince çözülmüş değeri gösterir (`→ https://api.apix.dev`).
- **Durum çubuğu.** Git branch'i, dosya sayısı, ortam ve kısayol ipuçları burada görünür.
- **Cam ayarı.** Saydamlık slider'ı ve tema seçici (Obsidian / Cyber).

### 1.2 Mockup'ta değiştireceklerimiz
| Mockup'ta | Apix'te | Neden |
|---|---|---|
| `SIMD Validated`, `HTTP/3 QUIC Ready`, `zero_copy_parser` gibi etiketler | Kaldırılacak. Yalnızca gerçekten çalışan şeyler gösterilecek. | Dürüst arayüz kuralı. HTTP/3 henüz deneysel. |
| `120 FPS` sayacı her zaman açık | Sadece geliştirici modunda görünecek (Ctrl+Shift+P). | Minimalizm |
| Her yerde rozet | Rozet sadece bilgi taşıyorsa kullanılacak. | Görsel gürültüyü azaltmak |
| macOS trafik ışıkları | Windows ve Linux'ta sağda küçült/büyüt/kapat düğmeleri (✅ yapıldı) | Her platform kendi düzenini kullanır. |
| `.apix` uzantısı | **Karar gerekiyor**, bkz. §6 | |

### 1.3 Renk paleti (tasarım token'ları)
| Token | Değer | Kullanım |
|---|---|---|
| `bg.gradient` | `#0A1626` → `#0F2A3D` (sol üst) → `#161E3A` (sağ alt) | Pencere zemini, blur'un üstünde |
| `surface` | `#0E1726`, %70 opak | Panel, editör, yanıt alanı |
| `surface.raised` | `#132033`, %80 opak | URL çubuğu, açılır menüler |
| `hairline` | beyaz, %8 opak | Panel ayraçları |
| `border.active` | `#22D3EE`, %60 opak | Seçili öğe, odaklanmış alan |
| `accent` | `#22D3EE` (elektrik camgöbeği) | SEND düğmesi, linkler, seçili sekme |
| `text` | `#E6EDF3` | Ana metin |
| `text.muted` | `#8B9BB0` | Yardımcı metin, dosya adları |
| `status.ok` | `#34D399` | 2xx yanıtlar |
| `status.warn` | `#FBBF24` | 3xx yanıtlar, uyarılar |
| `status.err` | `#F87171` | 4xx ve 5xx yanıtlar, hatalar |

**Metot renkleri:**
| Metot | Renk |
|---|---|
| GET | `#34D399` yeşil |
| POST | `#22D3EE` camgöbeği |
| PUT | `#FBBF24` sarı |
| PATCH | `#FB923C` turuncu |
| DELETE | `#F87171` kırmızı |
| HEAD, OPTIONS | `#A78BFA` mor |
| gRPC | `#818CF8` çivit |
| GraphQL | `#F472B6` pembe |
| WebSocket | `#2DD4BF` turkuaz |

**Tipografi:**
- Arayüz: **Inter** veya **Geist Sans**, 13 px
- Kod, URL ve JSON: **JetBrains Mono** veya **Geist Mono**, 12.5 px
- Fontlar uygulamanın içine gömülür, böylece her işletim sisteminde aynı görünür.

**Temalar:**
- **Obsidian:** koyu ve sade. Varsayılan tema.
- **Cyber:** mockup'taki camgöbeği gradyan.
- **Light:** açık tema, M7'de gelecek.
- **Cam slider'ı:** saydamlığı %0 (tamamen opak) ile %40 arasında ayarlar.

### 1.4 Ekran anatomisi
```
┌───────────────────────────────────────────────────────────────────────────┐
│ [≡] apix   [ 🔍 Uç noktaları ara, test çalıştır, ortam değiştir   ⌘K ]   [Prod ▾]  – □ ✕ │  ← Başlık + komut çubuğu
├──────────────┬────────────────────────────────────────────────────────────┤
│ 🔍 Filtre     │ workspace / auth / verify.apix                     ⋯     │  ← Breadcrumb
│ ▾ Auth        │ [POST ▾] [{{base_url}}]/v2/auth/session        [ SEND ] │  ← URL çubuğu
│   POST Verify │ ─────────────────────────────┬──────────────────────────│
│   DEL  Revoke │ Params Headers Auth Body Tests│ ● 200 OK  11 ms  1.3 KB  │
│ ▾ Users       │                              │ Pretty Raw Headers Timing│
│   GET  List   │   (istek editörü)            │   (yanıt görüntüleyici)  │
│   PUT  Update │                              │                          │
├──────────────┴──────────────────────────────┴──────────────────────────┤
│ ⎇ main*  6 dosya  ✓ git       ·  Production       ⌘B Kenar  ⌘K Palet  │  ← Durum çubuğu
└───────────────────────────────────────────────────────────────────────────┘
```
- Kenar çubuğu Ctrl/⌘+B ile tamamen gizlenir.
- Editör ile yanıt paneli arasındaki ayraç sürüklenebilir. Pencere daralınca paneller alt alta dizilir.
- Kullanılacak gpui-kit bileşenleri: `TitleBar`, `Tree`, `Tabs`, `Table`, `Input` (tree-sitter destekli kod editörü), `VirtualList`, `Resizable`, `Command`, `Select`, `Slider`, `StatusBar`, `Notification`, `Chart`, `Diff`.

---

## 2. Özellik kataloğu

### A. İstek oluşturma

#### A1. HTTP metotları
| Özellik | Açıklama | Durum |
|---|---|---|
| **GET** | Veri okuma. Gövde (body) gönderilmez. Query parametreleri URL ile iki yönlü senkron çalışır. | ✅ motor · 🔜 UI |
| **POST** | Yeni kayıt oluşturma. Bütün gövde tiplerini destekler (§A4). | ✅ motor · 🔜 UI |
| **PUT** | Kaydın tamamını değiştirme. Gövde tipleri POST ile aynıdır. | ✅ motor · 🔜 UI |
| **PATCH** | Kaydın bir kısmını güncelleme. JSON Patch ve Merge Patch şablonları gelir. | ✅ motor · 🔜 UI |
| **DELETE** | Kayıt silme. İsteğe bağlı gövde gönderilebilir. İstenirse "prod ortamında DELETE göndermeden önce onay iste" koruması açılabilir. | ✅ motor · 🔜 UI |
| **HEAD** | Sadece başlıkları getirir. Yanıt panelinde gövde sekmesi gizlenir. | ✅ motor |
| **OPTIONS** | CORS preflight yardımcısı. İzin verilen metotları ve başlıkları tablo olarak gösterir. | ✅ motor · ⬜ yardımcı |
| **Özel metot** | `PROPFIND`, `PURGE` gibi elle yazılan metotlar. | ⬜ S |
| **Metot seçici** | Açılır liste. Ayrıca URL çubuğunda `Alt+1…7` ile hızlı seçim. | 🔜 S |

#### A2. URL çubuğu
- ⬜ Düzenlenebilir, tek satırlık bir kod editörü olacak. `{{değişken}}` çip olarak çizilir. Tanımsız değişkenin altı kırmızı çizilir. (🔜 M)
- ⬜ Fareyle değişkenin üstüne gelince çözülmüş değer ve hangi kapsamdan geldiği görünür.
- ⬜ **Path değişkenleri:** `/users/:id` yazılınca Params sekmesinde `id` alanı kendiliğinden açılır.
- ⬜ **Query senkronu:** `?a=1&b=2` yazılınca tabloya, tabloya yazılınca URL'ye eklenir.
- ⬜ **cURL yapıştırma:** URL çubuğuna bir cURL komutu yapıştırılırsa istek otomatik oluşturulur.
- 🔜 **Ctrl/⌘+Enter** isteği gönderir. **Esc** devam eden isteği iptal eder.

#### A3. Params ve Headers
- 🔜 Anahtar–değer tablosu. Her satırda aç/kapa kutucuğu, açıklama alanı ve sürükleyerek sıralama olur. Kayıt modelinde bu yapı hazır (✅).
- ⬜ **Toplu düzenleme:** tablo ile `key: value` metin modu arasında geçiş.
- ⬜ **Header otomatik tamamlama:** standart header adları ve yaygın değerleri önerilir (`Content-Type`, `Accept` ...).
- ⬜ **Otomatik header'lar:** `Content-Type`, `User-Agent`, `Accept-Encoding` gibi otomatik eklenen başlıklar soluk renkte gösterilir. Kullanıcı bunları ezebilir. Motorda kullanıcının header'ını öne alma kuralı hazır (✅).
- ⬜ **Header ön ayarları:** sık kullanılan header gruplarını kaydedip tek tıkla ekleme.

#### A4. Body (gövde) tipleri
| Tip | Ayrıntı | Durum |
|---|---|---|
| none | | ✅ |
| raw / JSON | Renklendirme, biçimlendirme (Ctrl+Shift+F), anlık doğrulama, yorum satırı desteği (gönderirken silinir) | ✅ motor · 🔜 editör |
| raw / Text, XML, HTML, JavaScript | Her biri kendi renklendirmesiyle | ✅ Text, XML · ⬜ diğerleri |
| x-www-form-urlencoded | Anahtar–değer tablosu | ✅ motor · ⬜ UI |
| multipart/form-data | Metin ve dosya alanları, alan başına içerik tipi | ⬜ M |
| binary | Dosya seçip ham olarak gönderme | ⬜ S |
| GraphQL | Sorgu ve değişkenler editörü, şemadan otomatik tamamlama (§H1) | ⬜ L |

#### A5. Kimlik doğrulama (Auth)
| Tip | Durum |
|---|---|
| No Auth / Inherit from parent (klasörden veya koleksiyondan miras al) | ✅ none · ⬜ miras |
| Bearer Token | ✅ |
| Basic Auth | ✅ |
| API Key (header veya query) | ✅ |
| Digest Auth | ⬜ M |
| OAuth 1.0 | ⬜ M |
| OAuth 2.0: Authorization Code + PKCE, Client Credentials, Password, Implicit, token yenileme, token önbelleği | ⬜ L |
| AWS Signature v4 | ⬜ M |
| Hawk, NTLM, Akamai EdgeGrid, JWT Bearer (JWT'yi Apix imzalar) | ⬜ M her biri |

#### A6. İstek ayarları
- ⬜ Zaman aşımı, yönlendirmeleri takip et/etme, SSL doğrulamasını kapatma, istek başına proxy.
- ⬜ HTTP sürümü seçimi (1.1 / 2). HTTP/3, reqwest'te kararlı hâle gelince eklenecek.
- ⬜ İstemci sertifikaları (mTLS) ve özel CA sertifikası.

### B. Yanıt paneli
| Özellik | Açıklama | Durum |
|---|---|---|
| Durum satırı | Durum kodu (renkli), süre, boyut | ✅ CLI · 🔜 UI |
| **Pretty** | JSON, XML ve HTML biçimlendirilmiş ve renkli gösterilir. JSON'da düğümler katlanabilir. | ✅ JSON metni · 🔜 görüntüleyici |
| **Raw** | Ham metin | 🔜 |
| **Preview** | HTML'i basit hâliyle, resim, PDF ve ses dosyalarını kendi görünümüyle gösterir. | ⬜ M |
| **Hex** | İkili yanıtlar için onaltılık görünüm | ⬜ S |
| **Headers** | Tablo, kopyala ve ara | 🔜 |
| **Cookies** | Yanıtla gelen çerezler | ⬜ |
| **Timing** | DNS, TCP, TLS, TTFB ve indirme sürelerini şelale grafiği olarak gösterir. Mockup'taki "Latency Visualizer" budur. | ✅ TTFB ve toplam · ⬜ ayrıntı L |
| **Büyük yanıt** | 100 MB+ yanıtlarda sanal liste kullanılır, sadece ekrandaki satırlar çizilir. | 🔜 L |
| **Gövdede arama** | Ctrl+F, regex desteğiyle | ⬜ |
| **Filtre** | JSONPath ve jq ile yanıtı süzme | ⬜ M |
| **Örnek olarak kaydet** | Yanıtı isteğin yanına "example" olarak kaydeder. Mock sunucu ve dokümantasyon bunu kullanır. | ⬜ |
| **Dosyaya kaydet ve kopyala** | | ⬜ S |
| **Visualizer** | Yanıtı tablo veya grafik olarak gösterme (Postman Visualizer karşılığı) | ⬜ L |

### C. Koleksiyonlar
| Özellik | Açıklama | Durum |
|---|---|---|
| **Klasör = koleksiyon** | Bir klasör içindeki `apix.toml` ve istek dosyaları bir koleksiyon oluşturur. | ✅ |
| **Ağaç görünümü** | Klasörler açılıp kapanır, her istekte metot rozeti görünür. | 🔜 M |
| **Oluştur / yeniden adlandır / çoğalt / sil** | Sağ tık menüsünden ve kısayollardan. Silinen dosyalar çöp kutusuna gider, hemen kalıcı silinmez. | ⬜ M |
| **Sürükle-bırak** | İstekleri ve klasörleri taşıma ve sıralama | ⬜ M |
| **Miras alma** | Klasör ve koleksiyon seviyesinde auth, header, değişken ve script tanımlanabilir. Alt öğeler bunları devralır. | ⬜ M |
| **Dokümantasyon** | Her koleksiyon, klasör ve istek için Markdown açıklama | ⬜ S |
| **Örnekler (examples)** | Kaydedilmiş istek ve yanıt çiftleri | ⬜ M |
| **Filtre ve arama** | Kenar çubuğunda anlık filtre, komut paletinde bulanık (fuzzy) arama | ⬜ S |
| **Çoklu koleksiyon** | Bir çalışma alanında birden fazla koleksiyon açılabilir. | ⬜ M |
| **Dosya izleme** | Dosya diskte değişirse (git pull veya başka bir editör) uygulama anında günceller. | ⬜ M |
| **Git durumu** | Kenar çubuğunda değişen dosyalar işaretlenir. Durum çubuğunda branch adı görünür. | ⬜ M |
| **Sekmeler** | Birden çok istek sekmede açık durur. Kaydedilmemiş değişiklik işareti olur. Ctrl+Tab ile sekmeler arası geçilir. | ⬜ M |
| **Geçmiş** | Gönderilen her istek yanıtıyla birlikte kaydedilir. Aranabilir ve tekrar gönderilebilir. | ⬜ M |

### D. Postman uyumluluğu ve içe/dışa aktarma ⭐ (öncelikli)
| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Postman Collection v2.1 ve v2.0 içe aktarma** | Klasörler, istekler, header'lar, tüm body modları (raw, urlencoded, formdata, file, graphql), tüm auth tipleri, koleksiyon değişkenleri, açıklamalar ve örnek yanıtlar | 🔜 L |
| **Postman Environment içe aktarma** | `*.postman_environment.json`. `secret` tipindeki değerler `*.local.toml` dosyasına yazılır, git'e girmez. | 🔜 S |
| **Postman Globals içe aktarma** | | ⬜ S |
| **Postman scriptleri** | Pre-request ve test scriptleri olduğu gibi taşınır, `pm.*` uyumlu API ile çalışır (§F). | ⬜ L |
| **İçe aktarma raporu** | Neyin aktarıldığı, neyin desteklenmediği ve hangi satırda olduğu listelenir. Hiçbir şey sessizce kaybolmaz. | 🔜 S |
| **Sürükle-bırak ile içe aktarma** | `.json` dosyasını pencereye bırakmak yeterli. | ⬜ S |
| **Postman'e dışa aktarma** | Apix koleksiyonu Postman v2.1 formatına dönüştürülür, böylece ekip arkadaşı Postman kullanmaya devam edebilir. | ⬜ M |
| **CLI ile içe aktarma** | `apix import postman koleksiyon.json --out ./api` | 🔜 S |
| Insomnia v4, Bruno, Hoppscotch | | ⬜ M her biri |
| OpenAPI 3 / Swagger 2 | İçe aktarma ve koleksiyon üretme | ⬜ L |
| HAR | Tarayıcı kayıtlarını içe aktarma | ⬜ M |
| cURL | İçe ve dışa aktarma | ⬜ S |

### E. Ortamlar ve değişkenler
| Özellik | Durum |
|---|---|
| Kapsamlar: global → koleksiyon → ortam → yerel ve script. Sonraki kapsam öncekini ezer. | ✅ motor |
| `{{değişken}}` çözümleme. İç içe değişkenler çalışır, döngüye girmez, eksikler raporlanır. | ✅ |
| Ortam seçici (üst çubukta "Production ▾") | 🔜 S |
| Ortam editörü: tablo, gizli değerler (••••), başlangıç ve güncel değer | ⬜ M |
| **Gizli değerler:** `*.local.toml` (git'e girmez, ✅) ve işletim sistemi anahtarlığı (Keychain / Credential Manager / Secret Service) | ⬜ M |
| **Dinamik değişkenler:** `{{$guid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomEmail}}` ... (Postman'in tüm listesi) | ⬜ M |
| Prod ortamı uyarısı: prod seçiliyken pencere kenarında kırmızı ince bir çizgi | ⬜ S |

### F. Scriptler ve testler
| Özellik | Durum |
|---|---|
| **JS motoru:** QuickJS (`rquickjs`). Küçüktür (yaklaşık 1 MB), hızlı açılır, her istek kendi izole ortamında çalışır. | ⬜ L |
| **Pre-request script:** istek gönderilmeden önce çalışır. Değişken atayabilir, header ekleyebilir, imza hesaplayabilir. | ⬜ |
| **Test script:** `pm.test`, `pm.expect` (Chai tarzı), `pm.response.json()`, `pm.environment.set` ... | ⬜ |
| `pm.sendRequest` ile script içinden istek gönderme (örneğin token alma) | ⬜ M |
| Hazır kütüphaneler: `crypto-js`, `lodash`, `moment`, `uuid`, `ajv` (JSON Schema) | ⬜ M |
| **Kodsuz testler:** `status == 200`, `body.users.length > 0`, `time < 300ms` gibi tablo hâlinde tanımlanan testler | ⬜ M |
| Yanıttan değişken çıkarma: "bu alanı `token` değişkenine kaydet" (sağ tık ile) | ⬜ S |
| Test sonuçları sekmesi (✓ / ✗ ve hata mesajı) | ⬜ |
| Script konsolu: `console.log` çıktıları ve ağ günlüğü (Postman Console karşılığı) | ⬜ M |

### G. Collection Runner ve CLI (Newman karşılığı)
| Özellik | Durum |
|---|---|
| `apix send <dosya>` | ✅ |
| `apix run <klasör>`: hata olursa sıfırdan farklı çıkış kodu döner | ✅ |
| `--env`, `--var anahtar=değer` | ✅ |
| Runner arayüzü: sıra seçimi, tekrar sayısı, istekler arası bekleme, durdur ve devam et | ⬜ M |
| **Veri odaklı test:** CSV veya JSON dosyasındaki her satır için koleksiyonu çalıştırma | ⬜ M |
| Raporlar: JUnit XML (CI için), JSON, HTML | ⬜ M |
| `--bail` (ilk hatada dur), `--parallel N`, `--timeout` | ⬜ S |
| GitHub Actions örneği ve `apix` için resmi Docker imajı | ⬜ S |

### H. Diğer protokoller
| Protokol | Ayrıntı | Durum |
|---|---|---|
| **GraphQL** | Şema introspection, otomatik tamamlama, değişkenler, şema gezgini | ⬜ L |
| **WebSocket** | Bağlan ve bağlantıyı kes, mesaj gönder, mesajları zaman damgasıyla listele, kayıtlı mesaj şablonları | ⬜ M |
| **Server-Sent Events** | Canlı olay akışı | ⬜ S |
| **gRPC** | Unary ve streaming çağrılar, `.proto` dosyası veya sunucu yansıması (reflection), metadata | ⬜ L |
| **Socket.IO** | | ⬜ M |
| **MQTT** | Abone ol ve yayınla | ⬜ M |
| **SOAP** | WSDL'den şablon üretme, XML zarf | ⬜ M |

### I. Geliştirici araçları
| Özellik | Durum |
|---|---|
| **Kod üretici:** cURL, fetch, axios, Python requests, Go, **Rust reqwest**, C#, Java, PHP, Swift, Kotlin, Node | ⬜ M |
| Çerez yöneticisi: alan adına göre listeleme, düzenleme, silme | ⬜ M |
| **Yakalama proxy'si:** tarayıcı veya telefon trafiğini kaydedip koleksiyona dönüştürme | ⬜ L |
| Sertifika yöneticisi | ⬜ S |
| Ağ konsolu: gerçekte gönderilen ham isteği ve gelen yanıtı gösterir | ⬜ M |

### J. Simülasyon, dokümantasyon ve izleme
| Özellik | Durum |
|---|---|
| **Mock sunucu:** kayıtlı örneklerden veya OpenAPI'den yerel sahte sunucu (`apix mock`) | ⬜ L |
| **Dokümantasyon üretme:** koleksiyondan statik HTML doküman sitesi (`apix docs`) | ⬜ M |
| **Monitörler:** zamanlanmış çalıştırma ve hata bildirimi. Yerel zamanlayıcı veya CI ile, bulut gerekmez. | ⬜ M |
| **Flows:** istekleri görsel düğümlerle birbirine bağlama | ⬜ L |
| **API tasarımı:** OpenAPI dosyasını düzenleme ve doğrulama | ⬜ L |

### K. Apix'e özel özellikler (Postman'de olmayanlar)
| Özellik | Açıklama |
|---|---|
| **Yük testi** | `apix bench` ve arayüzde grafik. Binlerce eşzamanlı istek, p50/p95/p99 gecikmeleri. |
| **Yanıt karşılaştırma** | İki çalıştırmayı veya iki ortamı yan yana gösterir. Hazır `Diff` bileşeni kullanılacak. |
| **Zaman yolculuğu geçmişi** | Her istek ve yanıt saklanır. Yanıtın dün ve bugünkü hâli karşılaştırılabilir. |
| **MCP sunucusu** | `apix mcp` ile yapay zeka ajanları koleksiyonlarını listeleyip çalıştırabilir. |
| **AI yardımcısı** | Test yazma ve hata açıklama, kendi API anahtarınla. Varsayılan olarak kapalı. |
| **WASM eklentileri** | Özel auth tipleri, içe aktarıcılar ve görselleştiriciler eklenebilir. |
| **Performans göstergesi** | FPS, bellek ve kare süresi. Geliştirici modunda açılır. |

### L. İşbirliği (bulutsuz)
- ⬜ Git tabanlı çalışma: branch, diff ve commit uygulamanın içinden yapılır.
- ⬜ Uygulama içi çakışma (conflict) çözümü. TOML dosyaları satır satır birleştirilebildiği için bu kolaydır.
- ⬜ Bir isteği link olarak paylaşma: dosyadan veya panodan `apix://` bağlantısı.

### M. Uygulama geneli
| Özellik | Durum |
|---|---|
| Şeffaf ve blur'lu pencere, kendi başlık çubuğu | ✅ |
| Komut paleti (Ctrl/⌘+K) | 🔜 M |
| Kısayol tablosu ve kısayolları özelleştirme | ⬜ M |
| Ayarlar ekranı (tema, cam ayarı, font, proxy, güncelleme) | ⬜ M |
| Türkçe ve İngilizce arayüz (`rust-i18n`, gpui-kit zaten destekliyor) | ⬜ M |
| Otomatik güncelleme | ⬜ M |
| Erişilebilirlik: klavyeyle tam kullanım, yüksek kontrast | ⬜ M |
| Çökme raporu (isteğe bağlı, varsayılan kapalı) | ⬜ S |

---

## 3. Postman → Apix karşılık tablosu
| Postman | Apix | Aşama |
|---|---|---|
| Request builder | URL çubuğu + sekmeler | M1 |
| Collections | Klasör ve TOML dosyaları + ağaç görünümü | M2 |
| Import / Export | Postman v2.1, Insomnia, OpenAPI, HAR, cURL | M2–M4 |
| Environments / Variables | Katmanlı değişkenler + ortam seçici | M3 |
| Authorization | Tüm tipler | M3–M4 |
| Pre-request / Tests | QuickJS, `pm.*` uyumlu | M5 |
| Collection Runner / Newman | Runner arayüzü + `apix run` | M5 (CLI ✅) |
| Console | Ağ ve script konsolu | M5 |
| GraphQL / WebSocket / gRPC / MQTT / SOAP | `apix-proto` | M6 |
| Code snippets | `apix-codegen` | M4 |
| Cookies / Certificates / Proxy | Yöneticiler + yakalama proxy'si | M4–M7 |
| Mock servers | `apix mock` (yerel) | M7 |
| Documentation | `apix docs` | M7 |
| Monitors | Yerel zamanlayıcı veya CI | M7 |
| Flows | Düğüm editörü | M8 |
| Visualizer | Tablo ve grafik görünümü | M7 |
| Workspaces / Team | Git tabanlı işbirliği | M7 |
| Postbot (AI) | İsteğe bağlı AI yardımcısı + MCP | M8 |

---

## 4. Aşamalar ve sıralı iş listesi

Her aşamanın sonunda kullanılabilir bir sürüm çıkar.

### M0 — Temel ✅ (tamamlandı)
- [x] Cargo workspace ve hız/boyut için ayarlanmış release profili
- [x] `apix-core`: istek modeli, değişkenler, TOML formatı (9 test)
- [x] `apix-http`: reqwest + rustls, TTFB ve toplam süre (2 test)
- [x] `apix` CLI: `send` ve `run`, `--env` ve `--var` ile
- [x] Masaüstü kabuğu: blur'lu pencere, başlık çubuğu, pencere düğmeleri

### M1 — Arayüzden gerçek istek 🔜
1. [ ] **Tasarım token'ları:** §1.3'teki paleti `theme.rs`'e taşımak, Obsidian ve Cyber temaları (S)
2. [ ] **Fontları gömmek:** Inter ve JetBrains Mono (S)
3. [ ] **tokio ↔ GPUI köprüsü:** istekler arka planda çalışır, sonuç arayüze gelir, iptal edilebilir (M)
4. [ ] **URL çubuğu:** düzenlenebilir alan, metot seçici, SEND düğmesi, Ctrl+Enter (M)
5. [ ] **Yanıt paneli:** durum, süre, boyut, Pretty ve Raw görünümleri, Headers sekmesi (M)
6. [ ] **JSON görüntüleyici:** renklendirme ve büyük yanıtlar için sanal liste (L)
7. [ ] **Params ve Headers tabloları:** aç/kapa kutucuğu, sayı rozetleri (M)
8. [ ] **Body editörü:** JSON, Text ve XML seçenekleri (M)

**Bitti sayılması için:** jsonplaceholder'a arayüzden GET ve POST gönderilip yanıt renkli olarak görülebilmeli.

### M2 — Koleksiyonlar ve Postman içe aktarma ⭐
1. [ ] Klasör açma: "Open Folder" ve en son açılanlar listesi (S)
2. [ ] Kenar çubuğu ağacı: gerçek dosyalardan, metot rozetleriyle (M)
3. [ ] İstek seçme, düzenleme ve kaydetme (Ctrl+S), kaydedilmemiş değişiklik işareti (M)
4. [ ] Yeni istek ve klasör oluşturma, yeniden adlandırma, çoğaltma, silme (çöp kutusuna) (M)
5. [ ] Sürükle-bırak ile taşıma ve sıralama (M)
6. [ ] Sekmeler (M)
7. [ ] **Postman v2.1/v2.0 koleksiyon içe aktarma** ve içe aktarma raporu (L)
8. [ ] **Postman environment içe aktarma** (S)
9. [ ] `apix import postman` CLI komutu (S)
10. [ ] Dosya izleme: diskteki değişiklikleri anında yansıtma (M)

**Bitti sayılması için:** gerçek bir Postman koleksiyonu içe aktarılıp arayüzden gezilebilmeli ve istekleri çalıştırılabilmeli.

### M3 — Ortamlar, değişkenler, auth
- Ortam seçici ve ortam editörü, gizli değerler ve OS anahtarlığı, dinamik değişkenler
- Değişken çipleri ve fareyle üstüne gelince değer gösterme
- Auth sekmesi: tüm temel tipler ve miras alma, OAuth 2.0 akışları
- Komut paleti (Ctrl/⌘+K)

### M4 — Günlük kullanıma hazır
- multipart, binary ve GraphQL gövdeleri. Digest, AWS SigV4 ve OAuth 1.0 auth tipleri.
- İstek ayarları (timeout, SSL, proxy, sertifikalar), çerez yöneticisi
- Kod üretici, cURL içe ve dışa aktarma, geçmiş
- Insomnia, OpenAPI ve HAR içe aktarma, Postman'e dışa aktarma

### M5 — Scriptler, testler, runner
- `apix-script` (QuickJS) ve `pm.*` uyumluluğu, hazır kütüphaneler, kodsuz testler
- Test sonuçları sekmesi, konsol
- Runner arayüzü, veri odaklı testler, JUnit/HTML raporlar

### M6 — Protokoller
- WebSocket, SSE, GraphQL introspection, gRPC, Socket.IO, MQTT, SOAP

### M7 — İleri özellikler
- Mock sunucu, dokümantasyon üretme, monitörler, visualizer, yakalama proxy'si
- Git entegrasyonu, açık tema, i18n, ayarlar ekranı, otomatik güncelleme

### M8 — Apix'e özel
- Yük testi, yanıt karşılaştırma, zaman yolculuğu, MCP, AI, WASM eklentileri, Flows

### M9 — Yayın
- GitHub Actions: Windows, macOS ve Linux'ta test ve derleme
- Kurulum paketleri: `.msi` ve winget, `.dmg` ve Homebrew, AppImage, Flatpak ve `.deb`
- **İmzalı sürümler:** Windows kod imzası (Akıllı Uygulama Denetimi engellemesin diye) ve macOS notarization
- Logo, web sitesi, tanıtım (r/rust, Hacker News)

---

## 5. Kalite kuralları (her aşamada geçerli)
- Her özelliğin `apix-core`'da birim testi olur. Arayüz mantığı mümkün olduğunca core'da kalır.
- `cargo clippy` uyarısız olmalı, `cargo fmt` uygulanmış olmalı.
- Performans bütçesi her sürümde ölçülür: açılış süresi, boşta bellek, ikili boyutu, 100 MB JSON testi.
- Arayüz değişiklikleri ekran görüntüsüyle doğrulanır.
- CI kurulunca her PR üç işletim sisteminde test edilir.

---

## 6. Karar bekleyen konular
| # | Konu | Seçenekler | Önerim |
|---|---|---|---|
| 1 | **İstek dosyası uzantısı** | `istek.toml` veya `istek.apix` (içerik yine TOML) | **`.apix`**. Dosya ilişkilendirme mümkün olur (çift tıklayınca Apix açılır), marka görünür olur, `apix.toml` ile karışmaz. GitHub'da renklendirme için `.gitattributes`'a `*.apix linguist-language=TOML` eklenir. |
| 2 | Varsayılan tema | Obsidian (sade) veya Cyber (mockup) | Obsidian varsayılan olsun, Cyber tek tıkla seçilebilsin. |
| 3 | Postman script uyumluluğu ne kadar olsun | Tam `pm.*` veya kendi API'miz | `pm.*` ile uyumlu ve `apix.*` takma adı. Geçişi kolaylaştırır. |
| 4 | Arayüz dili | Sadece İngilizce veya TR + EN | TR + EN (M7). Kod ve dokümanlar İngilizce kalır. |
| 5 | Sürüm numarası | `0.x` ile başla | M2 sonunda `0.1.0` ilk açık sürüm. |
