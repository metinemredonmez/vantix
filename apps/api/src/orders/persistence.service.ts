import { Injectable, Logger } from "@nestjs/common";
import { contracts } from "@vantix/shared";
import { PrismaService } from "../prisma/prisma.service";

type Tree = ReturnType<typeof contracts.OrderTree.parse>;

/**
 * Emir kalıcılığı (Postgres). Para/miktar Decimal string olarak saklanır (float değil).
 * order_event append-only. Not: state'in sahibi exec-core; bu DB okuma/blotter/audit içindir.
 */
@Injectable()
export class PersistenceService {
  private readonly log = new Logger(PersistenceService.name);

  constructor(private readonly prisma: PrismaService) {}

  /** Submit anında ağaç + bacaklar (DRAFT) yazılır ki blotter hemen görsün. */
  async createOrder(tree: Tree & { id: string }) {
    await this.prisma.tenant.upsert({
      where: { id: tree.tenant_id },
      update: {},
      create: { id: tree.tenant_id, name: tree.tenant_id },
    });
    await this.prisma.account.upsert({
      where: { id: tree.account_id },
      update: {},
      create: { id: tree.account_id, tenantId: tree.tenant_id, name: tree.account_id },
    });
    await this.prisma.orderTree.create({
      data: {
        id: tree.id,
        tenantId: tree.tenant_id,
        accountId: tree.account_id,
        brokerId: tree.broker_id,
        clientRef: tree.client_ref ?? null,
        legs: {
          create: tree.legs.map((l, i) => ({
            idx: i,
            symbol: l.symbol,
            side: l.side,
            qty: String(l.qty),
            price: l.price != null ? String(l.price) : null,
            nativeType: l.native_type ?? "LIMIT",
            tif: l.tif ?? "DAY",
            status: "DRAFT",
            filled: "0",
          })),
        },
      },
    });
  }

  /** exec.order.state → bacak statüsü güncelle. */
  async applyState(orderId: string, leg: number, to: string, reason: string | null) {
    await this.prisma.orderLeg.updateMany({
      where: { treeId: orderId, idx: leg },
      data: { status: to, reason },
    });
  }

  async appendEvent(kind: string, orderId: string | null, leg: number | null, payload: unknown) {
    await this.prisma.orderEvent.create({
      data: { kind, orderId, leg, payload: payload as object },
    });
  }

  getOrder(id: string) {
    return this.prisma.orderTree.findUnique({ where: { id }, include: { legs: { orderBy: { idx: "asc" } } } });
  }

  listOrders(tenantId: string, limit = 50) {
    return this.prisma.orderTree.findMany({
      where: { tenantId },
      orderBy: { createdAt: "desc" },
      take: limit,
      include: { legs: { orderBy: { idx: "asc" } } },
    });
  }

  /** Hesap başka tenant'a aitse true değil — çapraz-tenant erişimi engellemek için. */
  async accountBelongsToOtherTenant(accountId: string, tenantId: string): Promise<boolean> {
    const acc = await this.prisma.account.findUnique({ where: { id: accountId } });
    return !!acc && acc.tenantId !== tenantId;
  }

  async getEvents(id: string, limit = 200) {
    const rows = await this.prisma.orderEvent.findMany({
      where: { orderId: id },
      orderBy: { seq: "asc" },
      take: limit,
    });
    // BigInt JSON'a serialize edilemez → seq string'e çevrilir.
    return rows.map((e) => ({ ...e, seq: String(e.seq) }));
  }
}
