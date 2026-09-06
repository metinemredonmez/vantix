import { Module } from "@nestjs/common";
import { PreTradeRiskService } from "./pre-trade-risk.service";
@Module({ providers: [PreTradeRiskService], exports: [PreTradeRiskService] })
export class RiskModule {}
