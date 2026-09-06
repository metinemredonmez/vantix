# ADR-0004: Grafik/chart için TradingView (hazır kütüphane)

Durum: Kabul edildi (2026-09-05). İlgili: ARCHITECTURE §5 (Trade workspace), SCREENS.md.

## Karar
Web ve mobilde chart, DOM/derinlik görselleştirmesi ve teknik çizim için **TradingView'in hazır
kütüphanesi** kullanılır (Charting Library / Advanced Charts; mobilde aynı ürünün RN entegrasyonu).
Kendi chart motorumuzu YAZMIYORUZ.

## Gerekçe
- Chart, ana rekabet farkımız değil (fark: exec-core + sentetik emir orkestrasyonu). TradingView olgun,
  hızlı, trader'ların tanıdığı bir UX sağlar; sıfırdan yazmak yüksek maliyet + düşük getiri.
- Emir tetikleme/çizim üstü etkileşim (chart'tan SL/TP) TradingView'in çizim API'siyle beslenir;
  **emir mantığı yine exec-core'da** kalır (chart yalnız görsel + niyet girişi).

## Kapsam ve sınır
- Veri: `md.tick.*` feed → API/WS → TradingView datafeed adapter'ı (UDF/streaming).
- Chart'tan gelen aksiyonlar API'ye HTTP/WS ile gider, oradan `exec.order.submit`'e; chart karar vermez.
- Lisans: TradingView kütüphanesi lisans/erişim gerektirir; ticari kullanım öncesi teyit edilecek.

## Sonuç
- `apps/web` ve `apps/mobile` chart bileşeni TradingView sarmalayıcısıdır; SCREENS.md buna göre.
- Alternatif (lightweight-charts, kendi canvas motoru) reddedildi: özellik derinliği/UX yetersiz.
