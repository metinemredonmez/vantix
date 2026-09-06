# exec-core — Vantix Execution Core (Rust)

Tek deterministik emir motoru. Fund (TEFAS) ve Trade (BIST/VİOP) modüllerinin her emri, tetiği ve tick'i buradan geçer.

## Tasarım
- `Engine` saf durum makinesidir: `apply(Command) -> Vec<Event>`; I/O yapmaz, tek task'ta sıralı çalışır → deterministik, replay edilebilir, test edilebilir.
- `OrderTree` = legs + tree (parent→child, FILLED/PARTIAL) + oco_groups. Bracket, chain, OCO, OTO, scale-out hepsi bu modelle ifade edilir.
- Trigger engine: STOP / TRAILING / CONDITIONAL leg'ler borsaya gitmez, tick ile tetiklenince native emre dönüşür.
- Risk: `RiskPolicy` trait'i — motor politika bilmez, dışarıdan verilir (tenant bazlı config sonra).
- `main.rs`: NATS subject'lerini `Command`'a çevirir, olayları `exec.*` subject'lerine yayınlar.

## Çalıştırma
```
cargo test                      # 13 birim + bracket_readme + contracts_conformance
# JetStream gerekir (nats-server -js). EXEC_EVENT_LOG verilirse order_event JSONL'e yazılır.
NATS_URL=nats://localhost:4222 EXEC_EVENT_LOG=./order_event.jsonl RUST_LOG=info cargo run
```

## Kalıcılık & replay (main.rs)
- Giriş komutları JetStream `EXEC_IN` stream'inde durable (publisher core publish yapsa bile yakalanır).
- Boot'ta tek ephemeral consumer (DeliverPolicy::All) stream'i baştan verir: `seq <= boot_last_seq`
  REPLAY (state yeniden kurulur, outbound bastırılır), sonrası CANLI (outbound + sink).
- Çıkış olayları append-only `EventSink`'e yazılır: v0 FileSink (JSONL), sonra PostgresSink
  (infra/sql/0001_order_event.sql). Bkz. docs/adr/0003 ve ARCHITECTURE §3.

## Örnek: bracket emri (exec.order.submit)
```json
{
  "tenant_id": "demo", "account_id": "acc1", "broker_id": "mock",
  "legs": [
    { "symbol": "GARAN", "side": "BUY",  "qty": "1000", "price": "125" },
    { "symbol": "GARAN", "side": "SELL", "qty": "1000", "price": "130" },
    { "symbol": "GARAN", "side": "SELL", "qty": "1000", "native_type": "MARKET", "trigger_price": "121" }
  ],
  "tree": [ { "parent": 0, "child": 1, "on": "FILLED" }, { "parent": 0, "child": 2, "on": "FILLED" } ],
  "oco_groups": [[1, 2]]
}
```
Akış: leg0 → `exec.broker.send`; fill gelince leg1 borsaya, leg2 tetik bekler; `md.tick.GARAN` last ≤ 121 → leg2 MARKET gider; dolunca leg1 için `exec.broker.cancel`.

## Sırada
~~append-only order_event~~ (FileSink ✅, PostgresSink kaldı) · ~~JetStream replay on boot~~ ✅ ·
boot snapshot (tüm tick'i replay etmemek için) · algo slicer (TWAP/VWAP/iceberg) ·
sembol başına trigger index (O(1)) · pre-trade risk: bakiye/pozisyon/fat-finger · broker reconciliation
