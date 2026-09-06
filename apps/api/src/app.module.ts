import { Module } from "@nestjs/common";
import { PrismaModule } from "./prisma/prisma.module";
import { NatsModule } from "./nats/nats.module";
import { AuthModule } from "./auth/auth.module";
import { OrdersModule } from "./orders/orders.module";

// exec-core NATS'a bağlandığı için eski BrokerModule/RiskModule/MarketDataModule
// (in-memory engine'in parçaları) request yolundan çıkarıldı. Dosyalar @deprecated olarak duruyor.
@Module({ imports: [PrismaModule, NatsModule, AuthModule, OrdersModule] })
export class AppModule {}
