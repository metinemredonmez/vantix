# Vantix — Mimari

## 1. Tez
Tek platform, iki ürün modülü, tek çekirdek. Fund Copilot ve Trading EMS ayrı ürün gibi satılır ama aynı hesap, aynı broker bağlantısı, aynı Execution Core üzerinde çalışır. LLM ürünün merkezi değil arayüzüdür; kararlar deterministik motorlarda (quant, exec-core) alınır.

## 2. Üst düzey yapı
```
                Next.js (web)      Next.js (admin)      Expo (mobile)
                        └──────────────┬────────────────┘
                          NestJS API / BFF  (auth, tenant, CRUD, WS gateway, compliance, audit)
                                       │  NATS JetStream (event log + replay)
        ┌──────────────────────────────┼──────────────────────────────┐
  Python Quant/AI                Rust Execution Core              Broker Gateways
  factor, HRP/CVaR, backtest,    order state machine, trigger     FIX/REST/WS adapter'lar
  stress, LangGraph, KAP RAG     engine, algo slicer, tick fan-out Osmanlı, İş, Ak... + TEFAS
        └──────────────────────────────┼──────────────────────────────┘
                     PostgreSQL + TimescaleDB │ Redis │ pgvector
```

## 3. Execution Core — neden Rust, neden tek
Amaç: platformdaki **her emir, her tetik, her tick** tek yerden geçer. Fund modülü TEFAS emri gönderirken de, trader DOM'dan tek tık emir atarken de aynı çekirdek çalışır. Böylece:
- Emir durum makinesi tek yerde tanımlı, tek yerde test edilir.
- Zincir / OCO / trailing tarayıcıya değil sunucuya aittir; bağlantı kopsa da devam eder.
- NATS JetStream'den replay ile process restart'ta state yeniden kurulur.
- Tick fan-out ve trigger değerlendirme sembol başına sıralı, deterministik ve ölçülebilir.
Rust: bellek güvenli, GC duraksaması yok, tokio ile yüksek eşzamanlılık. Kapsam bilinçli olarak dar tutulur (~3–5k satır): order tree, state machine, trigger rules, slicer (TWAP/VWAP/iceberg), risk hook, NATS I/O. Backtest, UI, CRUD Rust'a girmez.

Referans olarak incelenecek: NautilusTrader `ExecutionEngine` ve contingent order semantiği (kopyalanmaz, tasarım okunur).

## 4. Servis sınırları ve sözleşme
Tüm servisler yalnızca NATS subject'leri ve `packages/contracts` şemalarıyla konuşur (bkz. `packages/contracts/subjects.md`). Senkron HTTP yalnızca web/mobile → api arasında.

| Servis | Sahip olduğu veri | Konuştuğu subject'ler |
|---|---|---|
| api (Nest) | users, tenants, accounts, portfolios, audit | exec.order.*, quant.job.*, ws push |
| exec-core (Rust) | order_tree, order_leg, order_event (append-only) | exec.*, md.tick.* |
| quant (Python) | fund_universe, nav_series, factors, backtests | quant.* |
| broker-gateway | broker_session, client_order_id map | exec.broker.*, md.tick.* |

## 5. Ürün modülleri
### Portfolio (Fund Copilot)
Profil → fon skorlama → HRP+CVaR optimizasyon → look-through → stress test → rebalancing trigger → (lisanslı kurumla) TEFAS emri. LLM: profil çıkarımı, açıklama, soru-cevap; hesap yapmaz.
### Trade (EMS)
Watchlist / chart / DOM-ladder / order ticket / chain builder / blotter. Emir tipleri: OCO, OTO, OTOCO, bracket, chain, trailing, TWAP, VWAP, POV, iceberg, scale in/out, basket, conditional, order templates, mevcut pozisyona SL/TP.
### Ortak
Hesap, pozisyon, bakiye, işlem geçmişi, bildirim, risk profili, compliance görünürlüğü.

## 6. Admin
Tenant (aracı kurum) yönetimi ve white-label ayarları · broker adapter konfigürasyonu · pre-trade risk limitleri (hesap/sembol/tenant) · fon evreni ve skor ağırlıkları · kullanıcı & yerindelik kayıtları · audit log ve emir event replay · sistem sağlığı (NATS lag, adapter session, feed gecikmesi).

## 7. Multi-tenant ve uyum
Her kurum bir tenant; veri satır seviyesinde `tenant_id` ile ayrılır, adapter ve risk limitleri tenant bazlı. Fund modülünde kişiselleştirilmiş öneri + al/sat aynı üründe olduğunda SPK yatırım danışmanlığı/PYS lisansı gerekir; bu yüzden fund execution yalnızca lisanslı tenant için açılır, diğerlerinde analiz + simülasyon. Gözetimden kaçırmaya yönelik hiçbir özellik kapsam dışıdır.

## 8. Monorepo
```
apps/web  apps/admin  apps/mobile  apps/api
services/exec-core (Rust)  services/quant (Python)  services/broker-gateway
packages/shared  packages/contracts  packages/ui
infra  docs
```
`bist-ems` iskeletindeki `packages/shared` ve `apps/api/src/orders` buraya taşınır; Nest içindeki in-memory OrderEngine, exec-core hazır olana kadar geçici motor olarak kalır ve aynı NATS sözleşmesini konuşur.

## 9. Geçiş planı
1. contracts: JSON Schema → zod / serde / pydantic üretimi
2. api: auth + tenant + WS gateway; Nest OrderEngine'i NATS'a bağla
3. exec-core v0: state machine + OCO/OTO + mock gateway; api ile aynı sözleşmede paralel çalıştır, sonra Nest motorunu kapat
4. broker-gateway: ilk gerçek kurum adapter'ı (spec alınacak)
5. web: Trade workspace (DOM + ticket + chain builder + blotter)
6. quant v0: TEFAS ingestion + factor + Portfolio Doctor
7. web: Portfolio sekmesi; mobile: takip + hızlı SL/TP
8. admin: tenant, risk limitleri, audit
9. trigger engine + algo slicer (exec-core); rebalancing engine (quant)
10. lisanslı tenant ile fund execution
