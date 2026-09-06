# contracts — NATS payload sözleşmesi (TEK KAYNAK)

Payload şemaları **elle yazılan JSON Schema** olarak `schema/exec.schema.json`'da. TS (zod),
Python (pydantic) tipleri buradan **üretilir**; Rust (serde) elle yazılır ve golden ile doğrulanır.
Karar gerekçesi: [../../docs/adr/0003-contracts-single-source.md](../../docs/adr/0003-contracts-single-source.md).

```
schema/exec.schema.json     ← TEK KAYNAK (elle düzenlenir)
codegen/generate.mjs        → zod + pydantic üretir (sıfır bağımlılık, Node stdlib)
golden/                     ← kanonik örnek payload'lar (üç dilin köprüsü) + index.json
tests/conformance_ts.ts     ← golden'ı üretilen zod ile doğrular
tests/conformance_py.py     ← golden'ı üretilen pydantic ile doğrular
subjects.md                 ← subject → payload eşlemesi
```
Üretilen çıktılar: `packages/shared/src/generated/contracts.ts`, `services/quant/vantix_contracts/generated.py`.
Rust conformance: `services/exec-core/tests/contracts_conformance.rs`.

## Komutlar
```bash
pnpm --filter @vantix/contracts gen        # şemadan zod + pydantic üret
pnpm --filter @vantix/contracts test:ts    # zod conformance
pnpm --filter @vantix/contracts test:py    # pydantic conformance (uv gerektirir)
cargo test -p exec-core                     # serde conformance dahil
```

## Yeni alan/subject eklerken
1. `schema/exec.schema.json`'ı güncelle (+ gerekiyorsa `subjects.md`).
2. `pnpm --filter @vantix/contracts gen` çalıştır (üretilen dosyaları elle düzenleme).
3. Rust struct'ı gerçek wire ile eşleşiyor mu — gerekiyorsa exec-core'da güncelle.
4. `golden/`'a örnek ekle ve `golden/index.json`'a kaydet.
5. `cargo test -p exec-core` + `test:ts` + `test:py` yeşil olmalı.

## Kural
- Para/miktar = Decimal **string**, float değil. Alanlar snake_case (wire ile birebir).
- Sözleşme katı: `additionalProperties: false`.
