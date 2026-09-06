/**
 * @deprecated GEÇİCİ. bist-ems'ten taşınan Nest in-memory motoru için. Tek kaynak DEĞİL —
 * gerçek sözleşme `contracts` namespace'inde (packages/contracts/schema/exec.schema.json'dan üretilir).
 * Bu dosya camelCase alan + `number` para kullanır; wire ise snake_case + Decimal STRING'dir.
 * exec-core NATS'a bağlanınca silinecek. Bkz. docs/adr/0003-contracts-single-source.md
 */
import { z } from "zod";

// --- Borsa'ya giden native emir tipleri (BIST'in bildiği) ---
export const NativeOrderType = z.enum(["LIMIT", "MARKET", "MARKET_TO_LIMIT"]);
export const TimeInForce = z.enum(["DAY", "IOC", "FOK", "GTD"]);
export const Side = z.enum(["BUY", "SELL"]);

// --- Bizim sentetik/advanced emir tipleri (server-side orkestrasyon) ---
export const SyntheticOrderType = z.enum([
  "STOP", "STOP_LIMIT", "TRAILING_STOP",
  "OCO", "OTO", "OTOCO", "BRACKET", "CHAIN",
  "TWAP", "VWAP", "POV", "ICEBERG",
  "SCALE_IN", "SCALE_OUT", "BASKET", "CONDITIONAL",
]);

// Emir durum makinesi
export const OrderStatus = z.enum([
  "DRAFT",           // kullanıcı oluşturdu, henüz aktif değil
  "PENDING_TRIGGER", // parent/şart bekliyor (borsaya gitmedi)
  "PENDING_RISK",    // pre-trade risk kontrolünde
  "SENDING",         // broker adapter'a verildi, ack bekleniyor
  "WORKING",         // borsada açık
  "PARTIALLY_FILLED",
  "FILLED",
  "CANCELLING",
  "CANCELLED",
  "REJECTED",
  "EXPIRED",
  "ERROR",
]);

export const Condition = z.object({
  symbol: z.string(),
  field: z.enum(["LAST", "BID", "ASK", "VOLUME", "TIME"]),
  op: z.enum([">", ">=", "<", "<=", "=="]),
  value: z.number(),
});

export const OrderLeg = z.object({
  id: z.string().uuid().optional(),
  symbol: z.string(),
  side: Side,
  qty: z.number().positive(),
  price: z.number().positive().optional(),
  nativeType: NativeOrderType.default("LIMIT"),
  tif: TimeInForce.default("DAY"),
  triggerPrice: z.number().positive().optional(),
  trailAmount: z.number().positive().optional(),
  trailPercent: z.number().positive().optional(),
  condition: Condition.optional(),
});

export const AdvancedOrderRequest = z.object({
  accountId: z.string(),
  brokerId: z.string(),
  type: SyntheticOrderType.or(z.literal("SIMPLE")),
  legs: z.array(OrderLeg).min(1),
  // Ağaç ilişkisi: child'lar parent FILLED olunca aktive olur
  tree: z.array(z.object({ parent: z.number(), child: z.number(), on: z.enum(["FILLED", "PARTIAL"]) })).optional(),
  // OCO grupları: legs indexleri, biri dolunca diğerleri iptal
  ocoGroups: z.array(z.array(z.number())).optional(),
  algoParams: z.record(z.unknown()).optional(), // TWAP süresi, dilim sayısı vb.
  clientRef: z.string().optional(),
});

export type AdvancedOrderRequest = z.infer<typeof AdvancedOrderRequest>;
export type OrderLeg = z.infer<typeof OrderLeg>;
export type OrderStatus = z.infer<typeof OrderStatus>;
