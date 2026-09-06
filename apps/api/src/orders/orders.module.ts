import { Module } from "@nestjs/common";
import { OrdersController } from "./orders.controller";
import { OrderReadModelService } from "./order-read-model.service";
import { PersistenceService } from "./persistence.service";
import { OrdersGateway } from "../ws/orders.gateway";
import { ControlController } from "../control/control.controller";

/**
 * Emir modülü artık exec-core'a NATS ile bağlanır. Eski in-memory OrderEngineService
 * (@deprecated) request yolundan çıkarıldı; exec-core hazır olduğu için devre dışı.
 */
@Module({
  controllers: [OrdersController, ControlController],
  providers: [OrderReadModelService, PersistenceService, OrdersGateway],
})
export class OrdersModule {}
