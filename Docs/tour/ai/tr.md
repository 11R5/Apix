Sunucu 404 Not Found döndü: jsonplaceholder.typicode.com üzerinde /status diye bir yol yok. İsteğin kendisinde sorun yok (sunucu hızlı yanıt verdi, başlıklar olağan), yanlış olan adres.

- Yolu denetleyin: bu API'de /users, /posts ve /comments var, /status yok.
- Durum sayfası başka bir sunucudaysa o adresi bu istek için ya da ortamında base_url'e yazın.
- "status == 200" denetimi yalnızca 404 yüzünden başarısız oldu; adres düzelince geçecek.
