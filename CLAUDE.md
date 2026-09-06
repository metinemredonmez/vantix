# Vantix — Claude Code çalışma notları

Bu dosya projeye giren her ajan/geliştirici için tek giriş noktasıdır. Önce bunu, sonra `docs/ARCHITECTURE.md` ve `docs/adr/` dosyalarını oku. İki PDF rapor (`docs/*.pdf`) ürün/mimari gerekçeleri içerir; karar tartışması gerekirse oraya bak, tekrar araştırma yapma.

## Ne yapıyoruz
Vantix = tek platform, iki ürün modülü, tek çekirdek:
- **Portfolio (Fund Copilot)** — TEFAS fonları için açıklanabilir AI portföy yönetimi. Kararı deterministik quant motoru verir (HRP + CVaR, look-through, stress test); LLM yalnızca profil çıkarımı ve "neden?" açıklaması yapar, hesap yapmaz.
- **Trade (EMS)** — aracı kurumun mevcut OMS/BIST bağlantısını değiştirmeden önüne gelen gelişmiş emir katmanı: DOM/ladder, bracket, OCO, OTO, chain, trailing, TWAP/VWAP/iceberg, scale in/out, order templates. B2B white-label.
- **Execution Core (Rust)** — iki modülün de her emri, tetiği ve tick'i buradan geçer. Tek deterministik motor.

Hedef kitle: bireysel yatırımcı (Portfolio) + aktif trader ve aracı kurumlar (Trade). Türkiye pazarı, SPK uyumu (bkz. ARCHITECTURE §7).

## Stack (karar verildi, tartışma açma)
| Katman | Teknoloji |
|---|---|
| Web + Admin | Next.js (App Router) + Tailwind, TypeScript |
| Mobil | Expo React Native |
| API / BFF | NestJS (Fastify adapter), WebSocket gateway |
| Execution Core | Rust, tokio + async-nats, rust_decimal |
| Quant / AI | Python 3.12, FastAPI + worker; polars, scipy, cvxpy, riskfolio-lib, LangGraph |
| Broker Gateway | FIX 4.4 / REST / WS adapter'ları (TS veya Rust) |
| Veri | PostgreSQL + TimescaleDB, Redis, pgvector |
| Event bus | NATS JetStream — servisler arası tek iletişim yolu |
| Monorepo | pnpm workspaces + Turborepo; Cargo workspace; uv (Python) |

Servisler yalnızca NATS subject'leri ve `packages/contracts` şemalarıyla konuşur. Senkron HTTP sadece web/mobile → api arasında.

## Depo düzeni
```
apps/web, apps/admin, apps/mobile   — Next / Next / Expo (henüz iskelet, README var)
apps/api                            — NestJS: auth, tenant, orders (geçici in-memory engine), broker registry, risk, market-data
services/exec-core                  — Rust motor (v0 yazıldı, cargo test ile doğrula)
services/quant                      — Python (README, henüz kod yok)
services/broker-gateway             — adapters-ts/: BrokerAdapter interface + MockBrokerAdapter
packages/shared                     — zod şemaları (AdvancedOrderRequest, OrderStatus, NATS subjects)
packages/contracts                  — NATS subject/payload sözleşmesi (subjects.md)
packages/ui                         — ortak React bileşenleri (boş)
infra/docker-compose.yml            — TimescaleDB, Redis, NATS JetStream
docs/                               — ARCHITECTURE.md, SCREENS.md, adr/, PDF raporlar
```

## Mevcut durum (5 Eylül 2026)
- exec-core v0: `Engine::apply(Command) -> Vec<Event>` saf durum makinesi; bracket/OCO/OTO/chain, kısmi fill'de child qty küçültme, stop/trailing/conditional trigger, `RiskPolicy` kancası, NATS bin. **`cargo test -p exec-core` yeşil**: 12 birim + `bracket_readme` (README örneği uçtan uca) + `contracts_conformance` (golden ↔ serde).
- contracts (tek kaynak) kuruldu: `packages/contracts/schema/exec.schema.json` → `codegen/generate.mjs` → zod (`packages/shared/src/generated/contracts.ts`, `contracts` namespace) + pydantic (`services/quant/vantix_contracts/generated.py`). Rust elle kalır, drift golden ile yakalanır. Bkz. ADR-0003.
- apps/api: NATS'a bağlı + **Postgres kalıcılık (Prisma)**. `POST /orders` → doğrula → DB'ye yaz (Tenant/Account/OrderTree/OrderLeg) → `exec.order.submit`; `exec.order.state`/rejected/risk → read-model DB'ye yazar (leg statü + append-only `OrderEvent`) + WebSocket. Okuma DB'den: `GET /orders` (blotter), `GET /orders/:id`, `GET /orders/:id/events`. Kalıcılık canlı doğrulandı (Postgres'te tree+leg+17 event). Şema: `apps/api/prisma/schema.prisma`. Çalıştırma: `DATABASE_URL=postgresql://<user>@localhost:5432/vantix`, `prisma db push` + `generate`. Eski in-memory motor **@deprecated**.
- services/broker-gateway/gateway (`@vantix/broker-gateway`): ayrı mock servis; `exec.broker.send`/`cancel` tüketir, `exec.broker.ack`/`fill`/`cancelled` yayınlar (MARKET anında, LIMIT tick çaprazında dolar).
- exec-core kalıcılık & replay (main.rs): giriş komutları JetStream `EXEC_IN` stream'inde durable; boot'ta tek ephemeral consumer (DeliverPolicy All) baştan verir → `seq<=boot_last_seq` REPLAY (state kurulur, outbound bastırılır), sonrası CANLI. Çıkış olayları append-only `EventSink` → FileSink JSONL (`EXEC_EVENT_LOG`); PostgresSink kaldı (infra/sql/0001_order_event.sql). **JetStream gerekir (nats-server -js).**
- `@vantix/shared` artık `dist`'e build'lenir (main→dist) → api/gateway **tsx'siz** plain node ile koşar.
- **Uçtan uca + replay canlı doğrulandı**: POST → exec-core → gateway → OCO; parent fill sonrası exec-core öldürülüp yeniden başlatıldı → JetStream'den replay (replayed=5, 0 illegal) → canlı tick ile bracket tamamlandı (leg0 FILLED/leg1 CANCELLED/leg2 FILLED). Smoke: `pnpm --filter @vantix/broker-gateway smoke`.
- **exec-core Faz 1 TAMAM** (hepsi test + çoğu canlı doğrulandı):
  - `CompositeRisk` (MAX_QTY/MAX_NOTIONAL/FAT_FINGER, referans=son fiyat)
  - **kill switch** (`exec.control.kill` → canlı emir iptal + submit blok; resume). API: `POST /control/kill`.
  - **modify order** (`exec.order.modify` → risk yeniden-kontrol → `exec.broker.modify`; qty≤filled reddi). API: `PATCH /orders/:id`.
  - **TIF/GTD** (`Leg.expire_at_ms`; tick zamanı geçince Expired + borsada cancel; IOC/FOK broker-native pass-through).
  - **boot snapshot** (`EXEC_SNAPSHOT`; boot'ta yüklenir, JetStream yalnız snapshot_seq sonrasını replay eder — canlı doğrulandı).
  - **reconciliation** (`exec.control.reconcile` → `exec.reconcile.mismatch`: ems_orphan / broker_orphan).
  - Test: **23 birim + 1 + 2**. Contracts: **17 golden** (serde/zod/pydantic).
- backend auth + tenant izolasyonu ✅: `POST /auth/register|login` → JWT (scrypt parola + HS256, node crypto, bağımlılıksız — bkz. `src/auth/crypto.ts`). Tüm /orders + /control uçları `AuthGuard`'lı; **tenant JWT'den zorlanır** (body'deki tenant_id yok sayılır), blotter/GET tenant'a göre filtreli. Canlı doğrulandı (tokensuz 401; body tenant 'hacker' → DB 'demo'; çapraz-tenant GET 404). `JWT_SECRET` env.
- Henüz yok: backend Faz 1 kalan → positions/balances endpoint'leri + WS auth/tenant filtre; web/quant/admin/mobile; gerçek broker adapter; algolar (Faz 2). (Persist+blotter+auth+tenant ✅)

## Sıradaki işler (sırayla)
1. ~~`cargo test -p exec-core` yeşil.~~ ✅
2. ~~`packages/contracts`: JSON Schema → zod / serde / pydantic üretimi (tek kaynak).~~ ✅ v0 (emir/exec/tick yüzeyi). Genişleme: rapor §14 kanonik model + §9 adapter contract (ADR-0003).
3. ~~apps/api: NATS client; `POST /orders` → `exec.order.submit`; `exec.order.state` → WebSocket push. Mock gateway'i `exec.broker.send` tüketen ayrı küçük servis yap.~~ ✅
4. ~~exec-core: append-only `order_event` persist + JetStream replay on boot. (+ `@vantix/shared` tsc build.)~~ ✅ (FileSink; PostgresSink + snapshot kaldı)
5. apps/web: Trade workspace — DOM/ladder, order ticket (simple/advanced), chain builder, blotter. Grafik **TradingView** (ADR-0004). `docs/SCREENS.md`.
6. services/quant v0: TEFAS ingestion + factor engine + Portfolio Doctor.
7. apps/admin: tenant, broker adapter config, risk limitleri, audit.

## Kurallar
- Emir mantığı **asla** frontend'de yaşamaz; her şey exec-core'da, sunucu tarafında.
- Para/miktar için float kullanma: Rust `rust_decimal`, TS `string`/decimal.js, Python `Decimal`.
- Yeni NATS subject eklerken önce `packages/contracts/subjects.md` güncelle.
- Her modül kendi verisinin sahibi; başka servisin tablosuna yazma.
- Mimari kararı değiştirmek gerekiyorsa `docs/adr/` altına yeni ADR yaz, eskisini silme.
- Uyum: gözetimden kaçırmaya yönelik özellik yok; fund execution yalnızca lisanslı tenant için.
- Dil: kod/yorum İngilizce veya Türkçe olabilir, commit mesajları İngilizce; kullanıcıya dönen metinler Türkçe.
- Test: motor davranışı değişiyorsa exec-core testine senaryo ekle; PR'da `cargo test`, `pnpm test` yeşil olmalı.
