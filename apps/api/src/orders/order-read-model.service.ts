import { Injectable, Logger, OnModuleInit } from "@nestjs/common";
import { contracts } from "@vantix/shared";
import { NatsService } from "../nats/nats.service";
import { OrdersGateway } from "../ws/orders.gateway";
import { PersistenceService } from "./persistence.service";

/**
 * exec-core olaylarını Postgres'e yazar (kalıcı okuma modeli) ve WS'e iter.
 * Emir state'inin sahibi exec-core; burası kalıcılık + push. Restart'ta veri kalır.
 */
@Injectable()
export class OrderReadModelService implements OnModuleInit {
  private readonly log = new Logger(OrderReadModelService.name);

  constructor(
    private readonly nats: NatsService,
    private readonly gateway: OrdersGateway,
    private readonly persistence: PersistenceService,
  ) {}

  onModuleInit() {
    this.nats.subscribe("exec.order.state", (d) => this.onState(d));
    this.nats.subscribe("exec.order.rejected", (d) => this.onRejected(d));
    this.nats.subscribe("exec.risk.violation", (d) => this.onRisk(d));
    this.log.log("read-model exec.order.* → Postgres dinliyor");
  }

  private async onState(data: unknown) {
    const e = contracts.StateChanged.safeParse(data);
    if (!e.success) return;
    const { order_id, leg, to, reason } = e.data;
    try {
      await this.persistence.applyState(order_id, leg, to, reason ?? null);
      await this.persistence.appendEvent("state_changed", order_id, leg, e.data);
    } catch (err) {
      this.log.warn(`state persist failed: ${(err as Error).message}`);
    }
    this.gateway.broadcast(e.data);
  }

  private async onRejected(data: unknown) {
    const e = contracts.Rejected.safeParse(data);
    if (!e.success) return;
    await this.persistence.appendEvent("rejected", e.data.order_id, null, e.data).catch(() => {});
    this.gateway.broadcast(e.data);
  }

  private async onRisk(data: unknown) {
    const e = contracts.RiskViolation.safeParse(data);
    if (!e.success) return;
    await this.persistence.appendEvent("risk_violation", e.data.order_id, e.data.leg, e.data).catch(() => {});
    this.gateway.broadcast(e.data);
  }
}
