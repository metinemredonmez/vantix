import {
  Body, Controller, Delete, ForbiddenException, Get, NotFoundException, Param, Patch, Post, Query, Req, UseGuards,
} from "@nestjs/common";
import { randomUUID } from "crypto";
import { contracts } from "@vantix/shared";
import { NatsService } from "../nats/nats.service";
import { PersistenceService } from "./persistence.service";
import { AuthGuard } from "../auth/auth.guard";
import { JwtClaims } from "../auth/crypto";

/**
 * HTTP → exec-core köprüsü + kalıcı okuma. Tüm uçlar auth'lu; tenant JWT'den gelir (body'den DEĞİL).
 * Emir mantığı exec-core'da; api doğrular, yayınlar, DB'den okur.
 */
@UseGuards(AuthGuard)
@Controller("orders")
export class OrdersController {
  constructor(
    private readonly nats: NatsService,
    private readonly persistence: PersistenceService,
  ) {}

  @Post()
  async create(@Body() body: unknown, @Req() req: { user: JwtClaims }) {
    const parsed = contracts.OrderTree.parse(body);
    // tenant JWT'den zorlanır; hesap başka tenant'a aitse reddedilir.
    if (await this.persistence.accountBelongsToOtherTenant(parsed.account_id, req.user.tenantId)) {
      throw new ForbiddenException("hesap bu tenant'a ait değil");
    }
    const tree = { ...parsed, tenant_id: req.user.tenantId, id: parsed.id ?? randomUUID() };
    await this.persistence.createOrder(tree);
    this.nats.publish("exec.order.submit", tree);
    return { id: tree.id, status: "SUBMITTED" };
  }

  @Get()
  list(@Req() req: { user: JwtClaims }, @Query("limit") limit?: string) {
    return this.persistence.listOrders(req.user.tenantId, limit ? Number(limit) : 50);
  }

  @Get(":id")
  async get(@Param("id") id: string, @Req() req: { user: JwtClaims }) {
    const order = await this.persistence.getOrder(id);
    if (!order || order.tenantId !== req.user.tenantId) throw new NotFoundException(`order ${id} bilinmiyor`);
    return order;
  }

  @Get(":id/events")
  async events(@Param("id") id: string, @Req() req: { user: JwtClaims }) {
    const order = await this.persistence.getOrder(id);
    if (!order || order.tenantId !== req.user.tenantId) throw new NotFoundException(`order ${id} bilinmiyor`);
    return this.persistence.getEvents(id);
  }

  @Patch(":id")
  async modify(@Param("id") id: string, @Body() body: unknown, @Req() req: { user: JwtClaims }) {
    const order = await this.persistence.getOrder(id);
    if (!order || order.tenantId !== req.user.tenantId) throw new NotFoundException(`order ${id} bilinmiyor`);
    const b = (body ?? {}) as { leg?: unknown; price?: unknown; qty?: unknown };
    const reqBody = contracts.ModifyRequest.parse({ order_id: id, leg: b.leg, price: b.price, qty: b.qty });
    this.nats.publish("exec.order.modify", reqBody);
    return { id, status: "MODIFY_SENT" };
  }

  @Delete(":id")
  async cancel(@Param("id") id: string, @Req() req: { user: JwtClaims }) {
    const order = await this.persistence.getOrder(id);
    if (!order || order.tenantId !== req.user.tenantId) throw new NotFoundException(`order ${id} bilinmiyor`);
    this.nats.publish("exec.order.cancel", { order_id: id });
    return { id, status: "CANCELLING" };
  }
}
