import { Injectable, Logger } from "@nestjs/common";
import { randomUUID } from "crypto";
import type { AdvancedOrderRequest, OrderLeg, OrderStatus, FillEvent } from "@vantix/shared";
import { OrderStateMachine } from "./order-state-machine";
import { BrokerRegistry } from "../broker/broker.registry";
import { PreTradeRiskService } from "../risk/pre-trade-risk.service";

/**
 * @deprecated exec-core (Rust) NATS'a bağlandı; request yolu artık OrdersController →
 * exec.order.submit. Bu in-memory motor devre dışı ve wire'dan çıkarıldı (bkz. app.module.ts,
 * docs/adr/0001, docs/adr/0003). Referans/karşılaştırma için tutuluyor; yeni kodda kullanma.
 *
 * Advanced Order Engine — sentetik emirlerin (OCO/OTO/Chain/Trailing/...) beyni.
 * Prensip: browser kapansa da zincir devam eder; tüm state burada + DB'de yaşar.
 */
interface LegState { leg: OrderLeg; status: OrderStatus; brokerOrderId?: string; filled: number }
interface OrderTreeState {
  id: string; req: AdvancedOrderRequest; legs: LegState[]; createdAt: Date;
}

@Injectable()
export class OrderEngineService {
  private readonly log = new Logger(OrderEngineService.name);
  private readonly orders = new Map<string, OrderTreeState>();

  constructor(
    private readonly fsm: OrderStateMachine,
    private readonly brokers: BrokerRegistry,
    private readonly risk: PreTradeRiskService,
  ) {
    brokers.onFill(f => this.handleFill(f));
  }

  async submit(req: AdvancedOrderRequest) {
    const id = randomUUID();
    const state: OrderTreeState = {
      id, req, createdAt: new Date(),
      legs: req.legs.map(leg => ({ leg, status: "DRAFT", filled: 0 })),
    };
    this.orders.set(id, state);

    // Root leg'ler = kimsenin child'ı olmayanlar; hemen aktive edilir. Child'lar PENDING_TRIGGER.
    const childIdx = new Set((req.tree ?? []).map(t => t.child));
    for (let i = 0; i < state.legs.length; i++) {
      if (childIdx.has(i)) this.transition(state, i, "PENDING_TRIGGER");
      else await this.activate(state, i);
    }
    return this.view(state);
  }

  get(id: string) { const s = this.orders.get(id); return s && this.view(s); }

  async cancel(id: string) {
    const s = this.orders.get(id); if (!s) return;
    for (let i = 0; i < s.legs.length; i++) await this.cancelLeg(s, i);
    return this.view(s);
  }

  // ---- iç akış ----
  private async activate(s: OrderTreeState, i: number) {
    const ls = s.legs[i];
    this.transition(s, i, "PENDING_RISK");
    const verdict = await this.risk.check(s.req.accountId, ls.leg);
    if (!verdict.ok) { this.transition(s, i, "REJECTED"); this.log.warn(`risk reject: ${verdict.reason}`); return; }

    // STOP / TRAILING / CONDITIONAL → borsaya gitmez, market-data tetikleyene kadar bekler
    if (ls.leg.triggerPrice || ls.leg.trailAmount || ls.leg.trailPercent || ls.leg.condition) {
      this.transition(s, i, "PENDING_TRIGGER");
      // TODO: MarketDataModule'e trigger subscribe et
      return;
    }
    this.transition(s, i, "SENDING");
    const adapter = this.brokers.get(s.req.brokerId);
    const ack = await adapter.placeOrder({ ...ls.leg, accountId: s.req.accountId, emsOrderId: `${s.id}:${i}` });
    if (ack.status === "REJECTED") { this.transition(s, i, "REJECTED"); return; }
    ls.brokerOrderId = ack.brokerOrderId;
    this.transition(s, i, "WORKING");
  }

  private async cancelLeg(s: OrderTreeState, i: number) {
    const ls = s.legs[i];
    if (this.fsm.isTerminal(ls.status)) return;
    if (ls.brokerOrderId) {
      this.transition(s, i, "CANCELLING");
      await this.brokers.get(s.req.brokerId).cancelOrder(ls.brokerOrderId);
    }
    this.transition(s, i, "CANCELLED");
  }

  private async handleFill(f: FillEvent) {
    const [orderId, idxStr] = f.orderId.split(":");
    const s = this.orders.get(orderId); if (!s) return;
    const i = Number(idxStr); const ls = s.legs[i];
    ls.filled += f.qty;
    this.transition(s, i, f.remaining === 0 ? "FILLED" : "PARTIALLY_FILLED");

    // OCO: aynı gruptaki diğerlerini iptal et
    for (const group of s.req.ocoGroups ?? []) {
      if (group.includes(i)) for (const j of group) if (j !== i) await this.cancelLeg(s, j);
    }
    // OTO / Chain / Bracket: child'ları aktive et
    for (const edge of s.req.tree ?? []) {
      const ready = edge.on === "FILLED" ? ls.status === "FILLED" : ls.filled > 0;
      if (edge.parent === i && ready && s.legs[edge.child].status === "PENDING_TRIGGER") {
        // Kısmi fill'de child qty'yi parent'ın gerçekleşen miktarına indir (stop qty = kalan pozisyon)
        s.legs[edge.child].leg.qty = Math.min(s.legs[edge.child].leg.qty, ls.filled);
        await this.activate(s, edge.child);
      }
    }
  }

  private transition(s: OrderTreeState, i: number, to: OrderStatus) {
    const from = s.legs[i].status;
    this.fsm.assert(from, to);
    s.legs[i].status = to;
    this.log.log(`${s.id}:${i} ${s.legs[i].leg.symbol} ${from} -> ${to}`);
    // TODO: NATS publish Subjects.orderStateChanged + WebSocket push
  }

  private view(s: OrderTreeState) {
    return { id: s.id, type: s.req.type, createdAt: s.createdAt,
      legs: s.legs.map((l, i) => ({ index: i, ...l.leg, status: l.status, filled: l.filled, brokerOrderId: l.brokerOrderId })) };
  }
}
