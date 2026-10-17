# Apix İş Planı

> Bu belge Apix'in ayrıntılı ve Türkçe iş planıdır. Özellikler tek tek yazılmıştır.
> Özet İngilizce yol haritası [ROADMAP.md](ROADMAP.md), teknik mimari [ARCHITECTURE.md](ARCHITECTURE.md) dosyasındadır.
> Tasarım referansı: [design/mockup-v1.webp](design/mockup-v1.webp).

**İşaretler:** ✅ yapıldı · 🔜 sıradaki · ⬜ planlandı · 💭 sonra karar verilecek
**Boyut:** S = birkaç saat · M = 1–3 gün · L = 1 hafta ve üstü

### Kullanıcı notları (17 Ekim 2026) ve plana nasıl işlendikleri
| # | Not | Nereye işlendi |
|---|---|---|
| 1 | Postman koleksiyonlarını tanıyabilmeli | §2-D. İçe aktarma ✅ yapıldı. Otomatik tanıma ve toplu dışa aktarma desteği eklendi. |
| 2 | Şifreli koleksiyonlar oluşturulup açılabilmeli, dışarıdan açılamamalı | §2-C2 (yeni), M4 |
| 3 | Tailscale ile ekip modu | §7 💭. Tüm geliştirme bitince hatırlatılacak. |
| 4 | Logo: `Logo/` klasöründeki görselden Windows, macOS ve Linux simgeleri | §2-M, M3 (ilk iş), M9 |
| 5 | Her işletim sistemi için ayrı klasör mü, tek klasör mü | §6, karar 6: tek masaüstü crate'i, işletim sistemine özel kısımlar `platform/` altında |
| 6 | Aç, yeni koleksiyon, yeni proje; Proje → Ortam → Koleksiyonlar gruplaması | §2-C1 (yeni), M3 |
| 7 | Koleksiyona metot seçerek istek eklemek gibi temel özellikler | §2-C |
| 8 | İsteği bash/sh, terminal, PowerShell ve cURL olarak kopyalamak | §2-I, M3'e öne alındı |
| 9 | Hem yerel tarafın hem hedefin ayrıntılı ağ bilgisi ve haritası | §2-B2 (yeni), M4 ve M7 |
| 10 | Program boyutu 15 MB'ı geçebilir | §0 ve M9 güncellendi: üst sınır yaklaşık 40 MB |

---

## 0. Vizyon ve kurallar

Apix, Postman'in yaptığı her şeyi yapan bir API istemcisi olacak. Postman'den daha hızlı, daha küçük ve daha sade olacak.

| Kural | Ne anlama geliyor |
|---|---|
| **Minimalist** | Ekranda sadece o an gereken şey görünür. Gerisi Ctrl/⌘+K komut paletinde durur. |
| **Hafif** | Hız ve özellik boyuttan önce gelir. Program dosyası için üst sınır yaklaşık 40 MB (şu an 27.7 MB; Postman ve Electron uygulamaları 300 MB'ı aşıyor). Açılış 150 ms'nin, boşta bellek kullanımı 60 MB'ın altında kalır (şu an 69 MB ölçüldü). |
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
| `.apix` uzantısı | ✅ Kullanılıyor. İçerik TOML. | |

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
| **Metot seçici** | Açılır liste. Ayrıca URL çubuğunda `Alt+1…7` ile hızlı seçim. | ✅ |

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

### B2. Ağ denetçisi ve ağ haritası (not 9)
Her istek için "Network" sekmesi: istek gerçekte nasıl ve nereden geçti?

| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Yerel taraf** | Kullanılan ağ arayüzü, yerel IP (IPv4/IPv6), varsayılan ağ geçidi, DNS sunucuları, kullanılan proxy, VPN/tünel olup olmadığı | ⬜ M |
| **DNS** | Çözülen tüm A/AAAA kayıtları, bağlanılan IP, DNS süresi | ⬜ M |
| **Bağlantı** | TCP bağlantı süresi, bağlantının yeniden kullanılıp kullanılmadığı, HTTP sürümü ve ALPN | ⬜ M |
| **TLS** | Sürüm, şifre takımı, sertifika zinciri (veren, geçerlilik ve kalan gün, SAN listesi). Süresi yaklaşan sertifika için uyarı. | ⬜ M |
| **Zaman şelalesi** | DNS → TCP → TLS → ilk bayt → indirme, çubuk grafik olarak (§B "Timing" ile aynı veri) | ⬜ M |
| **Yönlendirmeler** | 301/302 zinciri, her adımın süresi ve adresi | ⬜ S |
| **Boyut** | Sıkıştırılmış ve açılmış boyut, kullanılan sıkıştırma (gzip, br, zstd) | ⬜ S |
| **Rota (traceroute)** | Hedefe giden atlamalar ve gecikmeleri. ICMP yönetici izni istediğinde TCP traceroute'a geçilir. | ⬜ L |
| **Harita** | Atlamalar ve hedef, dünya haritası üzerinde çizgi olarak gösterilir. Konumlar çevrimdışı bir IP konum veritabanından gelir (ör. DB-IP Lite, CC-BY). Veritabanı isteğe bağlı indirilir. Çevrimiçi sorgu varsayılan olarak kapalıdır, gizlilik için. | ⬜ L |

**Teknik not:** reqwest DNS, TCP ve TLS sürelerini ayrı ayrı vermiyor. Bunun için zamanlayan bir connector yazılacak (hyper-util ile). TLS sertifika bilgisi reqwest'in `tls_info` özelliğinden alınacak.

### C. Koleksiyonlar
| Özellik | Açıklama | Durum |
|---|---|---|
| **Klasör = koleksiyon** | Bir klasör içindeki `apix.toml` ve istek dosyaları bir koleksiyon oluşturur. | ✅ |
| **Ağaç görünümü** | Klasörler açılıp kapanır, her istekte metot rozeti görünür. | ✅ |
| **Oluştur / yeniden adlandır / çoğalt / sil** | Sağ tık menüsünden ve kısayollardan. Silinen dosyalar çöp kutusuna gider, hemen kalıcı silinmez. | ✅ |
| **Metot seçerek yeni istek** (not 7) | "+" menüsünde GET, POST, PUT, PATCH, DELETE … seçilir; istek o metotla ve uygun gövde tipiyle açılır (POST/PUT/PATCH için JSON). | ✅ |
| **Sürükle-bırak** | İstekleri ve klasörleri taşıma ve sıralama | ⬜ M |
| **Miras alma** | Klasör ve koleksiyon seviyesinde auth, header, değişken ve script tanımlanabilir. Alt öğeler bunları devralır. | ⬜ M |
| **Dokümantasyon** | Her koleksiyon, klasör ve istek için Markdown açıklama | ⬜ S |
| **Örnekler (examples)** | Kaydedilmiş istek ve yanıt çiftleri | ⬜ M |
| **Filtre ve arama** | Kenar çubuğunda anlık filtre, komut paletinde bulanık (fuzzy) arama | ⬜ S |
| **Çoklu koleksiyon** | Bir çalışma alanında birden fazla koleksiyon açılabilir. | ⬜ M |
| **Dosya izleme** | Dosya diskte değişirse (git pull veya başka bir editör) uygulama anında günceller. | ✅ |
| **Git durumu** | Kenar çubuğunda değişen dosyalar işaretlenir. Durum çubuğunda branch adı görünür. | ⬜ M |
| **Sekmeler** | Birden çok istek sekmede açık durur. Kaydedilmemiş değişiklik işareti olur. Ctrl+Tab ile sekmeler arası geçilir. | ⬜ M |
| **Geçmiş** | Gönderilen her istek yanıtıyla birlikte kaydedilir. Aranabilir ve tekrar gönderilebilir. | ⬜ M |

### C1. Projeler: Proje → Ortam → Koleksiyonlar (not 6)
Koleksiyonlar projeler altında toplanır, ortamlar projeye aittir.

```
A Projesi/                      apix-project.toml  (ad, renk, koleksiyon sırası)
├── environments/
│   ├── test.toml               ← projedeki tüm koleksiyonlarda geçerli
│   └── prod.toml
├── kullanicilar/               ← koleksiyon (apix.toml)
├── odemeler/                   ← koleksiyon
└── raporlar/                   ← koleksiyon; "sadece Test'te göster" ile işaretlenebilir
```

| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Aç / Yeni koleksiyon / Yeni proje** | Başlangıç ekranı ve komut paletinde üç seçenek. "Yeni proje" sihirbazı ad, klasör ve ilk ortamları (Test, Prod) sorar. | ✅ Kenar çubuğundaki proje/koleksiyon menüsünden (yeni proje Test ve Prod ortamlarıyla oluşur). Komut paleti M3'te. |
| **Son açılanlar** | Projeler ve koleksiyonlar, son açılma sırasıyla. Koleksiyonlar için ✅ (boş ekranda "RECENT" listesi, kullanıcı ayarlarında `state.toml`); projeler C1 ile gelecek. | ✅ |
| **Proje seçici** | Kenar çubuğunun üstündeki "Shop ▾" menüsü: son açılanlar, Aç, Yeni proje. Windows'ta başlık çubuğu sürükleme alanı olduğu için oraya konmadı. | ✅ |
| **Proje ortamları** | Projenin `environments/` klasörü, içindeki tüm koleksiyonlara uygulanır. Koleksiyonun kendi ortamı varsa onun üstüne eklenir. Katman sırası: proje → koleksiyon → ortam → yerel. | ✅ (CLI `--env` de aynı katmanları kullanıyor) |
| **Özelleştirilebilir gruplama** | Kenar çubuğu iki şekilde gösterilebilir: "Proje → Ortam → Koleksiyonlar" ya da "Proje → Koleksiyonlar". Bir koleksiyon belirli ortamlara bağlanabilir, örneğin "Raporlar sadece Test'te". Gruplara renk ve sıra verilebilir. | ✅ gruplama ve "Show in environment" sabitleme · ⬜ renk ve sürükleyerek sıra |
| **Var olan koleksiyonu projeye ekleme** | Sürükle-bırak ile ya da "Projeye taşı" menüsüyle | ⬜ S |
| **Prod koruması** | Prod ortamı seçiliyken pencere kenarında kırmızı bir çizgi görünür. DELETE, PUT ve PATCH göndermeden önce onay istenir (açılıp kapatılabilir). | Kırmızı çizgi ✅ (adında "prod" geçen ortamlar) · onay ⬜ S |

### C2. Şifreli koleksiyonlar 🔐 (not 2)
Bir koleksiyon (veya proje) parola ile şifrelenir. Parolayı bilmeyen hiçbir araç, dosyalar elinde olsa bile içeriği okuyamaz.

| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Şifreli koleksiyon oluşturma** | "Yeni koleksiyon" ekranında "Şifrele" seçeneği ve parola. Parola gücü göstergesi. | ⬜ M |
| **Var olanı şifreleme / şifreyi kaldırma** | Sağ tık → "Şifrele…" / "Şifreyi kaldır…" (parola sorulur) | ⬜ S |
| **Kilit açma** | Açarken parola sorulur. Anahtar sadece bellekte tutulur. İsteğe bağlı olarak işletim sistemi anahtarlığında hatırlanır. Belirli bir süre işlem yapılmazsa otomatik kilitlenir. | ⬜ M |
| **Ne şifrelenir** | İstek dosyaları, ortamlar, scriptler ve klasör/istek adları. Dosya adları anlamsız kimliklere dönüşür, adlar şifreli bir dizinde tutulur. Diskte sadece "bu şifreli bir Apix koleksiyonudur" bilgisi açık kalır. | ⬜ L |
| **Algoritma** | XChaCha20-Poly1305 (dosya başına rastgele nonce) ve Argon2id ile paroladan anahtar türetme (RustCrypto crate'leri) | ⬜ |
| **Kurtarma anahtarı** | Oluştururken bir kez gösterilen kurtarma anahtarı. Parola ve kurtarma anahtarı ikisi birden kaybolursa veri açılamaz; bu, arayüzde açıkça yazar. | ⬜ S |
| **CLI ve CI** | `apix run --password-env APIX_PASSWORD` | ⬜ S |
| **Git** | Her dosya ayrı şifrelendiği için git hâlâ hangi isteğin değiştiğini görür, ama içeriğini göremez. | ⬜ |

**Dürüst sınır:** Apix açık kaynak olduğu için güvenlik, formatın gizli olmasından değil algoritmadan ve parolanın gücünden gelir. Kilidi açık koleksiyonun ekran görüntüsü veya panoya kopyalanan bir değer elbette korunmaz.

### D. Postman uyumluluğu ve içe/dışa aktarma ⭐ (öncelikli)
| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Postman Collection v2.1 ve v2.0 içe aktarma** | Klasörler, istekler, header'lar, raw/urlencoded/graphql gövdeleri, bearer/basic/API key auth (miras dahil), koleksiyon değişkenleri, açıklamalar, path değişkenleri. Multipart dosyaları, OAuth 2.0 ve örnek yanıtlar raporda listelenir. | ✅ |
| **Postman Environment içe aktarma** | `*.postman_environment.json`. `secret` tipindeki değerler `*.local.toml` dosyasına yazılır, git'e girmez. | ✅ |
| **Postman Globals içe aktarma** | Tanınıyor, ortam olarak aktarılıyor. Proje genelinde değişken olarak kullanılması C1 ile gelecek. | ✅ tanıma · ⬜ S |
| **Postman scriptleri** | İsteklerin yanına `.js` olarak kaydediliyor ✅. `pm.*` uyumlu API ile çalıştırılması M5'te (§F). | ⬜ L |
| **İçe aktarma raporu** | Neyin aktarıldığı, neyin desteklenmediği ve hangi istekte olduğu listelenir. Hiçbir şey sessizce kaybolmaz. | ✅ |
| **Sürükle-bırak ile içe aktarma** | `.json` dosyasını pencereye bırakmak yeterli. | ✅ kod · ⬜ elle test |
| **Otomatik tanıma** (not 1) | Postman export'u içeren bir klasör açıldığında ya da sürüklendiğinde Apix bunu fark eder ve "Postman koleksiyonu bulundu, içe aktarılsın mı?" diye sorar. Dosya adına değil içeriğe bakarak tanır (v1, v2.0, v2.1, ortam, globals). | ⬜ S |
| **Postman toplu dışa aktarma** | Postman "Export data" ile verilen, tüm koleksiyon ve ortamları içeren klasör veya zip tek seferde bir projeye (§C1) dönüştürülür. | ⬜ M |
| **Postman v1** | Tanınıyor; kullanıcıya v2.1 olarak yeniden dışa aktarması söyleniyor. | ✅ |
| **Postman'e dışa aktarma** | Apix koleksiyonu Postman v2.1 formatına dönüştürülür, böylece ekip arkadaşı Postman kullanmaya devam edebilir. | ⬜ M |
| **CLI ile içe aktarma** | `apix import <dosya> [--out klasör] [--into koleksiyon]` | ✅ |
| Insomnia v4, Bruno, Hoppscotch | | ⬜ M her biri |
| OpenAPI 3 / Swagger 2 | İçe aktarma ve koleksiyon üretme | ⬜ L |
| HAR | Tarayıcı kayıtlarını içe aktarma | ⬜ M |
| cURL | İçe ve dışa aktarma | ⬜ S |

### E. Ortamlar ve değişkenler
| Özellik | Durum |
|---|---|
| Kapsamlar: global → koleksiyon → ortam → yerel ve script. Sonraki kapsam öncekini ezer. | ✅ motor |
| `{{değişken}}` çözümleme. İç içe değişkenler çalışır, döngüye girmez, eksikler raporlanır. | ✅ |
| Ortam seçici (kenar çubuğunda, koleksiyon başına hatırlanır) | ✅ |
| Ortam editörü (Ctrl+E, "Değişkenler" penceresi): ortak değişkenler ve ortamlar tek yerde, ortam ekleme, yeniden adlandırma, silme (çöp kutusuna), gizli değerler (••••, göz düğmesiyle görünür). Kaydet'e basılana kadar diske hiçbir şey yazılmaz. | ✅ · ⬜ başlangıç ve güncel değer ayrımı |
| **Gizli değerler:** `*.local.toml` (git'e girmez; `.gitignore` satırı kendiliğinden eklenir, ✅) ve işletim sistemi anahtarlığı (Keychain / Credential Manager / Secret Service) | ✅ `.local.toml` · ⬜ anahtarlık M |
| **Dinamik değişkenler:** `{{$guid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomEmail}}` ve 23 tane daha. Her kullanımda yeni değer; aynı adla tanımlanan değişken önce gelir. Listesi pencerede, tıklayınca kopyalanır. | ✅ |
| Prod ortamı uyarısı: prod seçiliyken pencere kenarında kırmızı ince bir çizgi | ✅ |

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
| **Kabuk komutu olarak kopyala** (not 8): sağ tık ve URL çubuğundaki "Kopyala ▾" menüsü. Değişkenler çözülmüş veya `{{değişken}}` olarak bırakılabilir. Gizli değerler maskelenip maskelenmeyeceği sorulur. Seçenekler: **bash/zsh/sh** için `curl` (tek tırnak kaçışlı) · **Windows cmd** için `curl.exe` (`^` satır devamı) · **PowerShell** için `Invoke-RestMethod` ve `Invoke-WebRequest` ile `curl.exe` sürümü · **fish** · **HTTPie** · **wget** | ✅ bash, cmd, PowerShell (gerçek kabuklarda httpbin ile doğrulandı) · ⬜ fish, HTTPie, wget |
| **Kabuk komutundan içe aktar:** URL çubuğuna yapıştırılan `curl` (bash veya cmd biçimi) ve PowerShell `Invoke-RestMethod` komutları isteğe dönüşür | ✅ curl (bash ve cmd biçimi, tarayıcıların "Copy as cURL" çıktısı dahil) · ⬜ PowerShell |
| **Kod üretici:** fetch, axios, Python requests, Go, **Rust reqwest**, C#, Java, PHP, Swift, Kotlin, Node | ⬜ M |
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
| **Uygulama simgesi** (not 4): kaynak `Logo/LogoFirst.jpg` (2048×2048). Windows için `.ico` (16–256 px, exe içine gömülü), macOS için `.icns` (16–1024 px, @2x dahil), Linux için hicolor PNG'leri ve `.desktop` dosyası. Ayrıca başlık çubuğundaki işaret, README ve GitHub sosyal önizlemesi. Ayrıntılar M3'te. | ⬜ M |
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
| Code snippets | Kabuk komutu olarak kopyala (bash, cmd, PowerShell) M3 · `apix-codegen` (diller) M4 | M3–M4 |
| Cookies / Certificates / Proxy | Yöneticiler + yakalama proxy'si | M4–M7 |
| Mock servers | `apix mock` (yerel) | M7 |
| Documentation | `apix docs` | M7 |
| Monitors | Yerel zamanlayıcı veya CI | M7 |
| Flows | Düğüm editörü | M8 |
| Visualizer | Tablo ve grafik görünümü | M7 |
| Workspaces / Team | Projeler (§C1) M3 · git tabanlı işbirliği M7 · Tailscale ekip modu 💭 | M3–M7 |
| — (Postman'da yok) | Şifreli koleksiyonlar (§C2) | M4 |
| Console / zamanlama | Ağ denetçisi ve harita (§B2) | M4–M7 |
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

### M1 — Arayüzden gerçek istek ✅ (tamamlandı)
1. [x] **Tasarım token'ları:** §1.3'teki paleti `theme.rs`'e taşımak, Obsidian ve Cyber temaları (S)
2. [x] **Fontları gömmek:** Inter 4.1 ve JetBrains Mono 2.304 (OFL 1.1), toplam 2.2 MB (S)
3. [x] **tokio ↔ GPUI köprüsü:** istekler arka planda çalışır, sonuç arayüze gelir, iptal edilebilir (M)
4. [x] **URL çubuğu:** düzenlenebilir alan, metot seçici, SEND düğmesi, Ctrl+Enter (M)
5. [x] **Yanıt paneli:** durum, süre, boyut, Pretty ve Raw görünümleri, Headers sekmesi (M)
6. [x] **JSON görüntüleyici:** renklendirme ve katlama. 512 KB üstü yanıtlar için satır tabanlı sanal görüntüleyici var. 100 MB JSON (9.6 milyon satır) release derlemede yaklaşık 0.33 saniyede ekrana geliyor. Tekerlek, kaydırma çubuğu ve klavye (↑ ↓ PgUp PgDn Ctrl+Home/End) ile geziliyor. Bellek 100 MB yanıtla yaklaşık 407 MB. (L)
7. [x] **Params ve Headers tabloları:** aç/kapa kutucuğu, sayı rozetleri (M)
8. [x] **Body editörü:** JSON, Text ve XML seçenekleri (M)

**Bitti sayılması için:** jsonplaceholder'a arayüzden GET ve POST gönderilip yanıt renkli olarak görülebilmeli.

### M2 — Koleksiyonlar ve Postman içe aktarma ⭐ 🔜 (10 maddeden 8'i bitti)
1. [x] Klasör açma (Ctrl+O). Apix olmayan bir klasör seçilirse orada yeni koleksiyon oluşturmayı önerir. Klasör pencereye sürüklenince de açılır. "En son açılanlar" listesi henüz yok. (S)
2. [x] Kenar çubuğu ağacı: gerçek dosyalardan, iç içe ve açılıp kapanan klasörler, metot rozetleri (M)
3. [x] İstek seçme, düzenleme ve kaydetme (Ctrl+S). Kaydedilmemiş değişiklik işareti var. Başka isteğe geçerken "Kaydet / Kaydetme / Vazgeç" sorusu çıkar. (M)
4. [x] Yeni istek (Ctrl+N) ve yeni klasör, yeniden adlandırma, çoğaltma, çöp kutusuna silme. Sağ tık menüsünden yapılır. Scriptler istekle birlikte taşınır. (M)
5. [ ] Sürükle-bırak ile taşıma ve sıralama (M)
6. [ ] Sekmeler (M)
7. [x] **Postman v2.1/v2.0 koleksiyon içe aktarma** ve içe aktarma raporu (L). Kenar çubuğu düğmesi, sürükle-bırak ve CLI ile yapılabilir.
8. [x] **Postman environment içe aktarma** (S). `secret` değerler git'e girmeyen `*.local.toml` dosyasına yazılır.
9. [x] `apix import <dosya> [--out klasör] [--into koleksiyon]` CLI komutu, dosya türünü kendisi algılar (S)
10. [x] Dosya izleme: diskteki değişiklikler (git pull, başka editör) yarım saniye içinde yansır. Açık istek kaydedilmemiş değişiklik içermiyorsa editör de güncellenir. (M)

**Bitti sayılması için:** gerçek bir Postman koleksiyonu içe aktarılıp arayüzden gezilebilmeli ve istekleri çalıştırılabilmeli.

### M3 — Simge, projeler, ortamlar, kopyalama, auth
1. [x] **Uygulama simgesi** (not 4) (M). `tools/icongen` aracı logodan PNG, ICO, ICNS ve Linux hicolor dosyalarını üretir. Simge exe'ye gömülü, başlık çubuğunda da görünüyor.
   - `Logo/LogoFirst.jpg` kırpılır: alttaki yansıma atılır, zemin squircle dışında saydam yapılır.
   - 16–32 px için sadeleştirilmiş bir sürüm hazırlanır (sadece kuş ve X). Küçük boyutta ince çizgiler kaybolur.
   - Windows için `.ico` exe'ye gömülür, macOS için `.icns`, Linux için hicolor PNG'leri ve `.desktop` dosyası.
   - Mümkünse saydam zeminli PNG veya vektör (SVG) bir ana dosya edinilir. JPG zemini sabit olduğu için en temiz sonuç bundan alınır.
2. [x] **Ortam seçici ve ortam editörü:** Seçici ✅ (kenar çubuğunda; Windows'ta başlık çubuğu sürükleme alanı olduğu için oraya konmadı). Koleksiyon başına hatırlanıyor, `.local.toml` gizli değerleri dahil. Ortam editörü ✅ (Ctrl+E). Dinamik değişkenler ✅ (`{{$guid}}` … 28 tane). Kalan: OS anahtarlığı. (M)
3. [x] **Projeler (§C1):** proje modeli (`apix-project.toml`), Yeni proje / Yeni koleksiyon / Aç, son açılanlar, proje menüsü, proje genelinde ortamlar (proje → koleksiyon → proje ortamı → koleksiyon ortamı), "Proje → Ortam → Koleksiyonlar" gruplaması, koleksiyonu ortamlara sabitleme, prod çizgisi. Kalanlar: renkler, koleksiyonu sürükleyerek projeye taşıma, prod onay penceresi. (L)
4. [x] **Kabuk komutu olarak kopyala** (§I): bash `curl`, cmd `curl.exe`, PowerShell `Invoke-RestMethod`; URL çubuğundaki kopyala menüsü, sağ tık ve `apix shell` CLI komutu. Yapıştırılan `curl` komutu isteği doldurur. Kalanlar: fish, HTTPie, wget. (M)
5. [x] **Metot seçerek yeni istek** (§C): kenar çubuğundaki "+" menüsü, klasör ve koleksiyon menülerinde "New request ▸". POST, PUT ve PATCH JSON gövdeyle açılır. URL çubuğunda Alt+1…7 metodu değiştirir. (S)
6. [ ] Değişken çipleri ve fareyle üstüne gelince değer gösterme (M)
7. [ ] Auth sekmesi: temel tipler ve miras alma, OAuth 2.0 akışları (L)
8. [ ] Komut paleti (Ctrl/⌘+K) (M)
9. [ ] M2'den kalanlar: sekmeler, sürükle-bırak ile taşıma ve sıralama (M)

### M4 — Günlük kullanıma hazır
- **Şifreli koleksiyonlar (§C2)** (not 2): oluşturma, şifreleme ve şifreyi kaldırma, kilit açma ve otomatik kilit, kurtarma anahtarı, CLI desteği (L)
- **Ağ denetçisi (§B2)** (not 9): yerel ağ bilgisi, DNS, TCP, TLS ve sertifika zinciri, zaman şelalesi, yönlendirmeler (L)
- multipart, binary ve GraphQL gövdeleri. Digest, AWS SigV4 ve OAuth 1.0 auth tipleri.
- İstek ayarları (timeout, SSL, proxy, sertifikalar), çerez yöneticisi
- Kod üretici (diller), geçmiş
- Postman otomatik tanıma ve toplu dışa aktarma, Insomnia, OpenAPI ve HAR içe aktarma, Postman'e dışa aktarma

### M5 — Scriptler, testler, runner
- `apix-script` (QuickJS) ve `pm.*` uyumluluğu, hazır kütüphaneler, kodsuz testler
- Test sonuçları sekmesi, konsol
- Runner arayüzü, veri odaklı testler, JUnit/HTML raporlar

### M6 — Protokoller
- WebSocket, SSE, GraphQL introspection, gRPC, Socket.IO, MQTT, SOAP

### M7 — İleri özellikler
- **Rota ve ağ haritası (§B2)** (not 9): traceroute, çevrimdışı IP konum veritabanı ile dünya haritası
- Mock sunucu, dokümantasyon üretme, monitörler, visualizer, yakalama proxy'si
- Git entegrasyonu, açık tema, i18n, ayarlar ekranı, otomatik güncelleme

### M8 — Apix'e özel
- Yük testi, yanıt karşılaştırma, zaman yolculuğu, MCP, AI, WASM eklentileri, Flows

### M9 — Yayın
- GitHub Actions: Windows, macOS ve Linux'ta test ve derleme
- Kurulum paketleri: `.msi` ve winget, `.dmg` ve Homebrew, AppImage, Flatpak ve `.deb`
- İkili boyutunu izlemek: üst sınır yaklaşık 40 MB (not 10). Şu an release 27.7 MB; içinde 2.2 MB font, simgeler ve tree-sitter var. Kolay kazançlar alınır (`panic = "abort"`, kullanılmayan özellikleri kapatmak), ama hız ve özellikten ödün verilmez.
- **İmzalı sürümler:** Windows kod imzası (Akıllı Uygulama Denetimi engellemesin diye) ve macOS notarization
- Kurulum paketlerinde M3'te hazırlanan simgelerin kullanılması, web sitesi, tanıtım (r/rust, Hacker News)

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
| 1 | ~~İstek dosyası uzantısı~~ | ✅ **Karar verildi: `.apix`** (içerik TOML). `.gitattributes` ile GitHub TOML olarak renklendiriyor. | |
| 2 | Varsayılan tema | Obsidian (sade) veya Cyber (mockup) | Obsidian varsayılan olsun, Cyber tek tıkla seçilebilsin. |
| 3 | Postman script uyumluluğu ne kadar olsun | Tam `pm.*` veya kendi API'miz | `pm.*` ile uyumlu ve `apix.*` takma adı. Geçişi kolaylaştırır. |
| 4 | Arayüz dili | Sadece İngilizce veya TR + EN | TR + EN (M7). Kod ve dokümanlar İngilizce kalır. |
| 5 | Sürüm numarası | `0.x` ile başla | M2 sonunda `0.1.0` ilk açık sürüm. |
| 6 | İşletim sistemleri için klasör yapısı (not 5) | (a) Windows, macOS ve Linux için ayrı UI klasörleri · (b) tek masaüstü crate'i | **(b) Tek crate.** GPUI ve gpui-kit üç sistemi aynı kodla destekliyor; Zed de böyle çalışıyor. Ortak kısımlar zaten ayrı crate'lerde: `apix-core`, `apix-http`, `apix-import`. Sisteme özel parçalar `crates/apix-desktop/src/platform/{windows,macos,linux}.rs` altına girer: pencere düğmeleri ve başlık çubuğu, blur/vibrancy, macOS menü çubuğu, dosya ilişkilendirme, anahtarlık. Kurulum dosyaları `packaging/{windows,macos,linux}/` altına girer. Bir sistemin gerçekten ayrı bir arayüze ihtiyacı çıkarsa o zaman ayrılır. |
| 7 | Şifreli koleksiyonun diskteki biçimi | (a) Tek bir şifreli dosya (kasa) · (b) Her dosya ayrı şifreli, dosya adları anlamsız | **(b).** Git hangi isteğin değiştiğini görmeye devam eder (içeriği değil), birleştirme çakışmaları küçük kalır. (a) daha basit ama her değişiklikte tüm dosya değişir. |
| 8 | Ortam ile koleksiyon ilişkisi (not 6) | Ortam = değişken seti (Postman gibi) · Ortam = koleksiyonları gruplayan klasör | **İkisi birden:** ortamlar değişken setidir, ama bir koleksiyon belirli ortamlara bağlanabilir. Kenar çubuğu istenirse "Proje → Ortam → Koleksiyonlar" olarak gruplanır. |

---

## 7. 💭 Sonra karar verilecek fikirler
Bu fikirler planın dışında tutuluyor. Ana geliştirme (M9'a kadar) bitince tekrar ele alınacak.

| Fikir | Not | Ne zaman hatırlatılacak |
|---|---|---|
| **Tailscale ile ekip modu** (not 3) | Bulut kullanmadan, ekibin tailnet'i üzerinden projeleri ve koleksiyonları paylaşmak. Örnek: bir makinedeki proje diğerlerine canlı açılır, değişiklikler eşlenir. Seçenekler: Tailscale yerel API'si veya `tsnet`, ya da sadece git + Tailscale üzerinden paylaşılan bir klasör. Önce git tabanlı işbirliği (§L) tamamlanacak, sonra ihtiyaç değerlendirilecek. | M9 bitince |
