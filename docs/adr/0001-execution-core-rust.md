# ADR-0001: Execution Core Rust'ta, tek servis
Karar: Emir durum makinesi, trigger engine ve algo slicer Rust (tokio + NATS) ile tek serviste yazılır; hem Fund hem Trade modülü bu servisi kullanır. API/BFF NestJS, quant Python, frontend/admin/mobile TS'de kalır.
Gerekçe: deterministik ve düşük gecikmeli emir orkestrasyonu tek yerde; GC duraksaması yok; kapsam dar tutulduğu için geliştirme maliyeti sınırlı.
Reddedilenler: tüm backend Rust (ürün API'si için gereksiz maliyet); NautilusTrader'ı gömmek (BIST adapter'ı yok, Python-first, kapsam fazla); Nest'te kalıcı engine (tick hacmi ve determinizm garantisi zayıf).
