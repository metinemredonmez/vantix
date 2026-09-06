# NATS subject → payload sözleşmesi

Payload şekilleri `schema/exec.schema.json` `$defs`'inde tanımlıdır (tek kaynak).
Giriş payload'ları etiketsiz spesifik struct; çıkış olayları `type` ile tag'li tek `Event`.

| Subject | Yayıncı → Tüketici | Payload ($def) |
|---|---|---|
| exec.order.submit | api → exec-core | `OrderTree` |
| exec.order.cancel | api → exec-core | `CancelRequest` `{ order_id, leg? }` |
| exec.order.modify | api → exec-core | `ModifyRequest` `{ order_id, leg, price?, qty? }` |
| exec.control.kill | api/admin → exec-core | `KillRequest` `{ tenant_id?, account_id?, active }` |
| exec.control.reconcile | gateway/admin → exec-core | `ReconcileRequest` `{ open_broker_ids }` |
| exec.broker.ack | gateway → exec-core | `BrokerAck` `{ ems_order_id, broker_order_id }` |
| exec.broker.reject | gateway → exec-core | `BrokerReject` `{ ems_order_id, reason }` |
| exec.broker.fill | gateway → exec-core | `Fill` |
| exec.broker.cancelled | gateway → exec-core | `BrokerCancelled` `{ ems_order_id }` |
| md.tick.<SYMBOL> | gateway/feed → exec-core, ws | `Tick` |
| exec.order.state | exec-core → api/ws | `StateChanged` `{ type, order_id, leg, from, to, reason }` |
| exec.broker.send | exec-core → gateway | `SendToBroker` (NativeOrder alanları + type) |
| exec.broker.cancel | exec-core → gateway | `CancelAtBroker` `{ type, ems_order_id, broker_order_id }` |
| exec.broker.modify | exec-core → gateway | `ModifyAtBroker` `{ type, ems_order_id, broker_order_id, price?, qty? }` |
| exec.risk.violation | exec-core → api | `RiskViolation` `{ type, order_id, leg, rule, detail }` |
| exec.order.rejected | exec-core → api | `Rejected` `{ type, order_id, reason }` |
| exec.reconcile.mismatch | exec-core → api/admin | `ReconcileMismatch` `{ type, ems_order_id?, broker_order_id, kind }` |

Aşağıdakiler henüz `exec.schema.json`'da yok — sözleşmeye eklenince buraya bağlanacak (ADR-0003 §Sonuç):

| Subject | Yayıncı → Tüketici | Payload |
|---|---|---|
| quant.job.<type> | api → quant | `{ job_id, params }` |
| quant.result.<type> | quant → api | `{ job_id, result }` |
| fund.order.submit | api → exec-core | `OrderTree` (TEFAS legs) |
