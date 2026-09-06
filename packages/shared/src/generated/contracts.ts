// OTOMATİK ÜRETİLDİ — elle düzenleme. Kaynak: packages/contracts/schema/exec.schema.json
// Yeniden üret: pnpm --filter @vantix/contracts gen
import { z } from "zod";

export const Decimal = z.string().regex(/^-?[0-9]+(\.[0-9]+)?$/, 'decimal string');
export type Decimal = z.infer<typeof Decimal>;

export const Uuid = z.string().uuid();
export type Uuid = z.infer<typeof Uuid>;

export const Side = z.enum(["BUY","SELL"]);
export type Side = z.infer<typeof Side>;

export const NativeType = z.enum(["LIMIT","MARKET","MARKET_TO_LIMIT"]);
export type NativeType = z.infer<typeof NativeType>;

export const TimeInForce = z.enum(["DAY","IOC","FOK","GTD"]);
export type TimeInForce = z.infer<typeof TimeInForce>;

export const TriggerOn = z.enum(["FILLED","PARTIAL"]);
export type TriggerOn = z.infer<typeof TriggerOn>;

export const CondField = z.enum(["LAST","BID","ASK"]);
export type CondField = z.infer<typeof CondField>;

export const CondOp = z.enum([">",">=","<","<="]);
export type CondOp = z.infer<typeof CondOp>;

export const Status = z.enum(["DRAFT","PENDING_TRIGGER","PENDING_RISK","SENDING","WORKING","PARTIALLY_FILLED","FILLED","CANCELLING","CANCELLED","REJECTED","EXPIRED","ERROR"]);
export type Status = z.infer<typeof Status>;

export const Condition = z.object({
  "symbol": z.string(),
  "field": CondField,
  "op": CondOp,
  "value": Decimal,
}).strict();
export type Condition = z.infer<typeof Condition>;

export const Leg = z.object({
  "symbol": z.string(),
  "side": Side,
  "qty": Decimal,
  "price": Decimal.nullable().optional(),
  "native_type": NativeType.default("LIMIT"),
  "tif": TimeInForce.default("DAY"),
  "trigger_price": Decimal.nullable().optional(),
  "trail_amount": Decimal.nullable().optional(),
  "trail_percent": Decimal.nullable().optional(),
  "condition": Condition.nullable().optional(),
  "expire_at_ms": z.number().int().nullable().optional(),
}).strict();
export type Leg = z.infer<typeof Leg>;

export const Edge = z.object({
  "parent": z.number().int().min(0),
  "child": z.number().int().min(0),
  "on": TriggerOn,
}).strict();
export type Edge = z.infer<typeof Edge>;

export const OrderTree = z.object({
  "id": Uuid.optional(),
  "tenant_id": z.string(),
  "account_id": z.string(),
  "broker_id": z.string(),
  "legs": z.array(Leg).min(1),
  "tree": z.array(Edge).default([]),
  "oco_groups": z.array(z.array(z.number().int().min(0))).default([]),
  "client_ref": z.string().nullable().optional(),
}).strict();
export type OrderTree = z.infer<typeof OrderTree>;

export const CancelRequest = z.object({
  "order_id": Uuid,
  "leg": z.number().int().min(0).nullable().optional(),
}).strict();
export type CancelRequest = z.infer<typeof CancelRequest>;

export const BrokerAck = z.object({
  "ems_order_id": z.string(),
  "broker_order_id": z.string(),
}).strict();
export type BrokerAck = z.infer<typeof BrokerAck>;

export const BrokerReject = z.object({
  "ems_order_id": z.string(),
  "reason": z.string(),
}).strict();
export type BrokerReject = z.infer<typeof BrokerReject>;

export const BrokerCancelled = z.object({
  "ems_order_id": z.string(),
}).strict();
export type BrokerCancelled = z.infer<typeof BrokerCancelled>;

export const ReconcileRequest = z.object({
  "open_broker_ids": z.array(z.string()),
}).strict();
export type ReconcileRequest = z.infer<typeof ReconcileRequest>;

export const ModifyRequest = z.object({
  "order_id": Uuid,
  "leg": z.number().int().min(0),
  "price": Decimal.nullable().optional(),
  "qty": Decimal.nullable().optional(),
}).strict();
export type ModifyRequest = z.infer<typeof ModifyRequest>;

export const KillRequest = z.object({
  "tenant_id": z.string().nullable().optional(),
  "account_id": z.string().nullable().optional(),
  "active": z.boolean(),
}).strict();
export type KillRequest = z.infer<typeof KillRequest>;

export const Fill = z.object({
  "ems_order_id": z.string(),
  "broker_order_id": z.string(),
  "qty": Decimal,
  "price": Decimal,
  "remaining": Decimal,
  "ts_ms": z.number().int(),
}).strict();
export type Fill = z.infer<typeof Fill>;

export const Tick = z.object({
  "symbol": z.string(),
  "last": Decimal,
  "bid": Decimal.nullable().optional(),
  "ask": Decimal.nullable().optional(),
  "ts_ms": z.number().int(),
}).strict();
export type Tick = z.infer<typeof Tick>;

export const NativeOrder = z.object({
  "ems_order_id": z.string(),
  "tenant_id": z.string(),
  "account_id": z.string(),
  "broker_id": z.string(),
  "symbol": z.string(),
  "side": Side,
  "qty": Decimal,
  "price": Decimal.nullable().optional(),
  "native_type": NativeType,
  "tif": TimeInForce,
}).strict();
export type NativeOrder = z.infer<typeof NativeOrder>;

export const StateChanged = z.object({
  "type": z.literal("state_changed"),
  "order_id": Uuid,
  "leg": z.number().int().min(0),
  "from": Status,
  "to": Status,
  "reason": z.string().nullable(),
}).strict();
export type StateChanged = z.infer<typeof StateChanged>;

export const SendToBroker = z.object({
  "type": z.literal("send_to_broker"),
  "ems_order_id": z.string(),
  "tenant_id": z.string(),
  "account_id": z.string(),
  "broker_id": z.string(),
  "symbol": z.string(),
  "side": Side,
  "qty": Decimal,
  "price": Decimal.nullable().optional(),
  "native_type": NativeType,
  "tif": TimeInForce,
}).strict();
export type SendToBroker = z.infer<typeof SendToBroker>;

export const CancelAtBroker = z.object({
  "type": z.literal("cancel_at_broker"),
  "ems_order_id": z.string(),
  "broker_order_id": z.string(),
}).strict();
export type CancelAtBroker = z.infer<typeof CancelAtBroker>;

export const ModifyAtBroker = z.object({
  "type": z.literal("modify_at_broker"),
  "ems_order_id": z.string(),
  "broker_order_id": z.string(),
  "price": Decimal.nullable().optional(),
  "qty": Decimal.nullable().optional(),
}).strict();
export type ModifyAtBroker = z.infer<typeof ModifyAtBroker>;

export const RiskViolation = z.object({
  "type": z.literal("risk_violation"),
  "order_id": Uuid,
  "leg": z.number().int().min(0),
  "rule": z.string(),
  "detail": z.string(),
}).strict();
export type RiskViolation = z.infer<typeof RiskViolation>;

export const ReconcileMismatch = z.object({
  "type": z.literal("reconcile_mismatch"),
  "ems_order_id": z.string().nullable().optional(),
  "broker_order_id": z.string(),
  "kind": z.string(),
}).strict();
export type ReconcileMismatch = z.infer<typeof ReconcileMismatch>;

export const Rejected = z.object({
  "type": z.literal("rejected"),
  "order_id": Uuid,
  "reason": z.string(),
}).strict();
export type Rejected = z.infer<typeof Rejected>;

export const Event = z.discriminatedUnion("type", [StateChanged, SendToBroker, CancelAtBroker, ModifyAtBroker, RiskViolation, Rejected, ReconcileMismatch]);
export type Event = z.infer<typeof Event>;
