// Mock broker gateway (ayrı servis).
// Tüketir : exec.broker.send (SendToBroker), exec.broker.cancel (CancelAtBroker), md.tick.* (Tick)
// Yayınlar: exec.broker.ack, exec.broker.fill, exec.broker.cancelled
//
// Davranış (mock): MARKET emri son fiyattan hemen dolar; LIMIT emri borsada bekler ve
// fiyat çaprazlayınca (md.tick) dolar. Gerçek kurum adapter'ı bunu FIX/REST/WS ile değiştirir.
// Para değerleri wire'da STRING; sadece eşleştirme karşılaştırması number'a çevrilir.
import { connect, JSONCodec } from "nats";
import { contracts } from "@vantix/shared";
import { marketable, type Side } from "./match.ts";

type Resting = { side: Side; priceStr: string; priceNum: number; qty: string; symbol: string; brokerOrderId: string };

const url = process.env.NATS_URL ?? "nats://localhost:4222";
const DEFAULT_PX = process.env.MOCK_DEFAULT_PRICE ?? "100";
const jc = JSONCodec();

const resting = new Map<string, Resting>(); // ems_order_id → resting limit
const lastPx = new Map<string, string>(); // symbol → last (string)
let seq = 0;

const nc = await connect({ servers: url, name: "vantix-mock-gateway" });
console.log(`mock-gateway NATS bağlandı: ${url}`);

function publish(subject: string, payload: unknown) {
  nc.publish(subject, jc.encode(payload));
}

function fill(o: { ems: string; brokerOrderId: string; qty: string; priceStr: string }) {
  publish("exec.broker.fill", {
    ems_order_id: o.ems,
    broker_order_id: o.brokerOrderId,
    qty: o.qty,
    price: o.priceStr,
    remaining: "0",
    ts_ms: Date.now(),
  });
  console.log(`  fill  ${o.ems} @ ${o.priceStr} x${o.qty}`);
}

async function onSend(data: unknown) {
  const p = contracts.SendToBroker.safeParse(data);
  if (!p.success) {
    console.warn("bad send payload", p.error.message);
    return;
  }
  const o = p.data;
  const brokerOrderId = `MOCK-${++seq}`;
  publish("exec.broker.ack", { ems_order_id: o.ems_order_id, broker_order_id: brokerOrderId });
  console.log(`ack   ${o.ems_order_id} → ${brokerOrderId} (${o.native_type} ${o.side} ${o.symbol} x${o.qty})`);

  if (o.native_type === "MARKET") {
    const priceStr = lastPx.get(o.symbol) ?? o.price ?? DEFAULT_PX;
    fill({ ems: o.ems_order_id, brokerOrderId, qty: o.qty, priceStr });
    return;
  }
  // LIMIT → borsada bekle; çaprazlarsa md.tick doldurur
  if (o.price == null) {
    console.warn(`limit ${o.ems_order_id} price yok, atlandı`);
    return;
  }
  resting.set(o.ems_order_id, {
    side: o.side,
    priceStr: o.price,
    priceNum: Number(o.price),
    qty: o.qty,
    symbol: o.symbol,
    brokerOrderId,
  });
}

function onTick(data: unknown) {
  const p = contracts.Tick.safeParse(data);
  if (!p.success) return;
  const t = p.data;
  lastPx.set(t.symbol, t.last);
  const last = Number(t.last);
  for (const [ems, r] of resting) {
    if (r.symbol === t.symbol && marketable(r.side, r.priceNum, last)) {
      resting.delete(ems);
      fill({ ems, brokerOrderId: r.brokerOrderId, qty: r.qty, priceStr: r.priceStr });
    }
  }
}

function onCancel(data: unknown) {
  const p = contracts.CancelAtBroker.safeParse(data);
  if (!p.success) return;
  resting.delete(p.data.ems_order_id);
  publish("exec.broker.cancelled", { ems_order_id: p.data.ems_order_id });
  console.log(`cancelled ${p.data.ems_order_id}`);
}

function onModify(data: unknown) {
  const p = contracts.ModifyAtBroker.safeParse(data);
  if (!p.success) return;
  const r = resting.get(p.data.ems_order_id);
  if (!r) return; // borsada değil (pending trigger vb.) → yerel exec-core zaten güncelledi
  if (p.data.price != null) {
    r.priceStr = p.data.price;
    r.priceNum = Number(p.data.price);
  }
  if (p.data.qty != null) r.qty = p.data.qty;
  console.log(`modify ${p.data.ems_order_id} → @${r.priceStr} x${r.qty}`);
}

async function pump(subject: string, handler: (d: unknown) => void) {
  const sub = nc.subscribe(subject);
  for await (const m of sub) {
    try {
      handler(jc.decode(m.data));
    } catch (e) {
      console.warn(`bad ${m.subject}: ${(e as Error).message}`);
    }
  }
}

await Promise.all([
  pump("exec.broker.send", onSend),
  pump("exec.broker.cancel", onCancel),
  pump("exec.broker.modify", onModify),
  pump("md.tick.*", onTick),
]);
