# Vantix — Wealth & Execution OS

Tek platform, iki ürün modülü, tek çekirdek:

- **Portfolio** (Fund Copilot): TEFAS fonları için açıklanabilir AI portföy yönetimi
- **Trade** (EMS): aracı kurumun mevcut OMS'i önünde gelişmiş emir katmanı (DOM, zincir emir, OCO, trailing, TWAP)
- **Execution Core** (Rust): iki modülün de emirlerinin geçtiği tek deterministik motor

Ayrıntı: `docs/ARCHITECTURE.md`

## Monorepo
```
apps/web          Next.js + Tailwind   — kullanıcı (Portfolio + Trade sekmeleri)
apps/admin        Next.js              — tenant/kurum yönetimi, risk limitleri, fon evreni, audit
apps/mobile       Expo React Native    — pozisyon/emir takibi, hızlı SL/TP, portföy doctor, bildirim
apps/api          NestJS (Fastify)     — auth, tenant, CRUD, WebSocket gateway, compliance, audit
services/exec-core    Rust (tokio+NATS) — order state machine, trigger engine, algo slicer, tick fan-out
services/quant        Python (FastAPI+worker) — factor, HRP/CVaR, backtest, stress, LangGraph, RAG
services/broker-gateway  FIX/REST/WS adapter'lar (Osmanlı, İş, Ak, ... + TEFAS)
packages/shared   zod şemaları, tipler
packages/contracts NATS subject + payload sözleşmesi (TS + Rust + Python'a üretilir)
packages/ui       ortak React bileşenleri (DOM, ticket, charts)
infra             docker-compose, k8s, migrations
docs              mimari, ADR'ler, ekran haritası, raporlar
```

## Başlangıç
```
pnpm install
pnpm infra:up            # TimescaleDB, Redis, NATS JetStream
cp .env.example .env
pnpm dev                 # turbo: api + web + admin
cargo run -p exec-core   # services/exec-core
uv run services/quant    # quant API
```
