# ADR-0003: Sözleşme tek kaynağı — JSON Schema → zod / serde / pydantic

Durum: Kabul edildi (2026-09-05). İlgili: ADR-0001 (exec-core tek servis), ARCHITECTURE §4/§9.

## Bağlam
Aynı NATS payload'ları üç dilde tekrar tanımlanıyordu ve birbirinden kaymıştı:
`packages/shared` (zod) `ems.*` subject + camelCase + `number` (float) para kullanıyordu;
`services/exec-core` (serde) `exec.*` + snake_case + `Decimal` (wire'da **string**) üretiyor;
pydantic hiç yoktu. Bu, CLAUDE.md'nin "para float olamaz" kuralını da ihlal ediyordu ve
zod, exec-core'un gerçek JSON'unu reddedecek durumdaydı.

## Karar
`packages/contracts/schema/exec.schema.json` **tek yetkili kaynak** (JSON Schema 2020-12, elle yazılır).
Ondan sıfır-bağımlılıklı bir Node üreteci (`codegen/generate.mjs`) şunları üretir:
- **zod** → `packages/shared/src/generated/contracts.ts` (`contracts` namespace'i ile export)
- **pydantic** → `services/quant/vantix_contracts/generated.py`

**Rust yeniden üretilmez.** exec-core serde struct'ları elle kalır (deterministik çekirdek, gerçek runtime).
Drift, **golden conformance** ile yakalanır: `packages/contracts/golden/*.json` örnekleri
üç dilde de doğrulanır — Rust'ta `services/exec-core/tests/contracts_conformance.rs` (serde
parse + round-trip + alan korunumu), Python'da `tests/conformance_py.py`, TS'te `tests/conformance_ts.ts`.
Aynı golden dosyalar köprü olduğundan üçü aynı payload üzerinde anlaşmak zorunda.

Reddedilenler:
- **Rust'tan schemars ile şema üretmek**: contracts'ın "elle yazılan şema" niyetini tersine çevirirdi.
- **zod'u kaynak yapmak**: TS-first; deterministik Rust çekirdeği için en zayıfı.
- **datamodel-code-generator / json-schema-to-zod**: ağ/bağımlılık; kendi dar üretecimiz yeterli ve tekrarlanabilir.

## Sözleşme kuralları (v0 kapsamı: exec-core emir/exec/tick yüzeyi)
- Para/miktar = `Decimal` **JSON string** (`"1000"`), asla float. Regex: `^-?[0-9]+(\.[0-9]+)?$`.
- Alan adları **snake_case** (wire ile birebir). Enum casing serde ile aynı (BUY, MARKET_TO_LIMIT, PARTIALLY_FILLED...).
- Giriş payload'ları (exec.order.submit / exec.broker.* / md.tick.*) etiketsiz, spesifik struct.
- Çıkış olayları (exec.order.state / exec.broker.send / ...) `type` ile internally-tagged tek `Event`.
- `additionalProperties: false` — sözleşme katı; üretilen zod `.strict()`, pydantic `extra="forbid"`.

## Sonuç / geçiş
- `@vantix/shared`'daki eski `order.ts`/`events.ts` **@deprecated** olarak kaldı (geçici Nest motoru
  hâlâ kullanıyor). exec-core NATS'a bağlanınca (Sıradaki işler #3) silinip `contracts` yükseltilecek.
- Yeni subject veya payload eklerken önce `schema/exec.schema.json` + `subjects.md` güncellenir,
  `pnpm --filter @vantix/contracts gen` çalıştırılır, golden eklenir; `cargo test` + iki conformance yeşil olmalı.
- Sonraki genişleme (bu ADR'de üretilmedi): rapor §14 kanonik model (Instrument, Position, Balance,
  RiskReservation, Strategy*, AuditEvent) ve §9 broker adapter capability contract.
