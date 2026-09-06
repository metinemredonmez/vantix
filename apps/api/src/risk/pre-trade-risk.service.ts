import { Injectable } from "@nestjs/common";
import type { OrderLeg } from "@vantix/shared";

/** Pre-trade risk: emir borsaya gitmeden önceki son kapı. Kurum bazlı limitler buraya. */
@Injectable()
export class PreTradeRiskService {
  async check(accountId: string, leg: OrderLeg): Promise<{ ok: boolean; reason?: string }> {
    if (leg.qty > 1_000_000) return { ok: false, reason: "MAX_QTY" };
    // TODO: bakiye, pozisyon limiti, fat-finger (fiyat sapması), günlük zarar limiti, sembol yasağı
    return { ok: true };
  }
}
