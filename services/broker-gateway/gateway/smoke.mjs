// Uçtan uca smoke: api olmadan, doğrudan NATS'a bracket yayınlar ve exec-core + mock-gateway
// döngüsünü doğrular. Gereksinim: NATS çalışıyor + exec-core çalışıyor + mock-gateway çalışıyor.
//
// Çalıştırma (3 terminal):
//   1) docker compose -f infra/docker-compose.yml up -d nats
//   2) NATS_URL=nats://localhost:4222 cargo run -p exec-core
//   3) NATS_URL=nats://localhost:4222 pnpm --filter @vantix/broker-gateway start
//   4) NATS_URL=nats://localhost:4222 pnpm --filter @vantix/broker-gateway smoke
import { connect, JSONCodec } from "nats";

const url = process.env.NATS_URL ?? "nats://localhost:4222";
const jc = JSONCodec();
const nc = await connect({ servers: url, name: "smoke" });
const id = "11111111-1111-1111-1111-111111111111";
const seen = [];

for (const subj of [
  "exec.order.state", "exec.broker.send", "exec.broker.ack", "exec.broker.fill",
  "exec.broker.cancel", "exec.broker.cancelled", "exec.order.rejected", "exec.risk.violation",
]) {
  const sub = nc.subscribe(subj);
  (async () => {
    for await (const m of sub) {
      const d = jc.decode(m.data);
      seen.push({ subj: m.subject, d });
      console.log(`« ${m.subject}`, JSON.stringify(d));
    }
  })();
}

const pub = (s, p) => { nc.publish(s, jc.encode(p)); console.log(`» ${s}`); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 1) bracket gönder
pub("exec.order.submit", {
  id, tenant_id: "demo", account_id: "acc1", broker_id: "mock",
  legs: [
    { symbol: "GARAN", side: "BUY", qty: "1000", price: "125" },
    { symbol: "GARAN", side: "SELL", qty: "1000", price: "130" },
    { symbol: "GARAN", side: "SELL", qty: "1000", native_type: "MARKET", trigger_price: "121" },
  ],
  tree: [{ parent: 0, child: 1, on: "FILLED" }, { parent: 0, child: 2, on: "FILLED" }],
  oco_groups: [[1, 2]],
});

await sleep(600);
pub("md.tick.GARAN", { symbol: "GARAN", last: "125", ts_ms: Date.now() }); // parent dolar
await sleep(600);
pub("md.tick.GARAN", { symbol: "GARAN", last: "121", ts_ms: Date.now() }); // stop tetiklenir

// doğrulama yardımcıları
const filled = (leg) => seen.some((e) => e.subj === "exec.order.state" && e.d.order_id === id && e.d.leg === leg && e.d.to === "FILLED");
const cancelled = (leg) => seen.some((e) => e.subj === "exec.order.state" && e.d.order_id === id && e.d.leg === leg && e.d.to === "CANCELLED");
// terminal duruma kadar bekle (leg2 FILLED + leg1 CANCELLED), en fazla 3sn
for (let i = 0; i < 30 && !(filled(2) && cancelled(1)); i++) await sleep(100);
const stopMarket = seen.some((e) => e.subj === "exec.broker.send" && e.d.ems_order_id === `${id}:2` && e.d.native_type === "MARKET");
const tpCancel = seen.some((e) => e.subj === "exec.broker.cancel" && e.d.ems_order_id === `${id}:1`);

const checks = [
  ["leg0 (parent) FILLED", filled(0)],
  ["leg2 (stop) MARKET borsaya gitti", stopMarket],
  ["leg2 (stop) FILLED", filled(2)],
  ["leg1 (TP) OCO ile iptal edildi", tpCancel],
];
console.log("\n=== SONUÇ ===");
let ok = true;
for (const [label, pass] of checks) { console.log(`${pass ? "✅" : "❌"} ${label}`); ok &&= pass; }
await nc.drain();
process.exit(ok ? 0 : 1);
