# ADR-0002: Tek platform, iki modül
Karar: Fund Copilot ve Trading EMS ayrı uygulama değil, Vantix altında iki modül; ortak identity/tenant/broker/exec-core/audit.
Gerekçe: aynı hesap ve broker bağlantısı; fon emirleri de exec-core'dan geçer; admin ve uyum tek yerde.
Sonuç: satış kanalları ayrı kalır (B2C/B2B2C vs B2B white-label); fund execution yalnızca lisanslı tenant'ta açılır.
