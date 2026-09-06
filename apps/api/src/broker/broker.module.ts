import { Module } from "@nestjs/common";
import { BrokerRegistry } from "./broker.registry";
@Module({ providers: [BrokerRegistry], exports: [BrokerRegistry] })
export class BrokerModule {}
