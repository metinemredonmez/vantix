import { Body, Controller, ForbiddenException, Post, Req, UseGuards } from "@nestjs/common";
import { contracts } from "@vantix/shared";
import { NatsService } from "../nats/nats.service";
import { AuthGuard } from "../auth/auth.guard";
import { JwtClaims } from "../auth/crypto";

/**
 * Acil durdurma (kill switch). Auth'lu. Kapsam tenant JWT'den zorlanır; sistem-geneli (tenant yok)
 * yalnız admin. account_id opsiyonel (o hesaba daralt).
 */
@UseGuards(AuthGuard)
@Controller("control")
export class ControlController {
  constructor(private readonly nats: NatsService) {}

  @Post("kill")
  kill(@Body() body: unknown, @Req() req: { user: JwtClaims }) {
    const b = (body ?? {}) as { account_id?: unknown; active?: unknown; system?: unknown };
    const systemWide = b.system === true;
    if (systemWide && req.user.role !== "admin") {
      throw new ForbiddenException("sistem geneli kill yalnız admin");
    }
    const req2 = contracts.KillRequest.parse({
      tenant_id: systemWide ? null : req.user.tenantId, // tenant JWT'den; body'den değil
      account_id: b.account_id ?? null,
      active: b.active,
    });
    this.nats.publish("exec.control.kill", req2);
    return { ok: true, scope: { tenant_id: req2.tenant_id, account_id: req2.account_id }, active: req2.active };
  }
}
