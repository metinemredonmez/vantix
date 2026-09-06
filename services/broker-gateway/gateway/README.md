# @vantix/broker-gateway — mock gateway servisi

exec-core ile broker arasındaki NATS köprüsü. Bu paket **mock** implementasyondur; gerçek
aracı kurum adapter'ı (FIX/REST/WS) aynı subject'leri konuşarak bunun yerine geçer.

| Yön | Subject | Payload ($def) |
|---|---|---|
| tüketir | `exec.broker.send` | `SendToBroker` |
| tüketir | `exec.broker.cancel` | `CancelAtBroker` |
| tüketir | `md.tick.*` | `Tick` |
| yayınlar | `exec.broker.ack` | `{ ems_order_id, broker_order_id }` |
| yayınlar | `exec.broker.fill` | `Fill` |
| yayınlar | `exec.broker.cancelled` | `{ ems_order_id }` |

## Mock davranış
- **MARKET**: son fiyattan (`md.tick`) hemen tam fill; fiyat yoksa `MOCK_DEFAULT_PRICE` (vars. 100).
- **LIMIT**: borsada bekler; `md.tick` fiyatı çaprazlayınca (BUY: last≤price, SELL: last≥price) tam fill.
- **cancel**: bekleyen emir düşürülür, `exec.broker.cancelled` yayınlanır.
- Para değerleri wire'da **string**; yalnız eşleştirme karşılaştırması number'a çevrilir.

## Çalıştırma
```bash
NATS_URL=nats://localhost:4222 pnpm --filter @vantix/broker-gateway start
pnpm --filter @vantix/broker-gateway test   # saf eşleştirme testi (NATS gerekmez)
```
Uçtan uca döngü için: [../../../scripts/smoke-exec-loop.mjs](../../../scripts/smoke-exec-loop.mjs).
