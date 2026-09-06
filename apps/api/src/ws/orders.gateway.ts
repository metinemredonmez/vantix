import { Logger } from "@nestjs/common";
import { WebSocketGateway, WebSocketServer } from "@nestjs/websockets";

// ws paketi @nestjs/platform-ws'in transitive bağımlılığı; tipini yapısal tanımlıyoruz
// (top-level bağımlılık eklemeden). Çalışma anında gerçek ws.Server enjekte edilir.
interface WsClient { readyState: number; send(data: string): void }
interface WsServer { clients?: Set<WsClient> }

/**
 * exec-core'dan gelen emir olaylarını bağlı istemcilere iter (WS).
 * v0: tüm olaylar tüm istemcilere broadcast. Sonraki adım: tenant/account'a göre filtre + auth.
 * Yol: ws://<host>/ws/orders
 */
@WebSocketGateway({ path: "/ws/orders" })
export class OrdersGateway {
  private readonly log = new Logger(OrdersGateway.name);
  @WebSocketServer() server?: WsServer;

  broadcast(event: unknown) {
    const msg = JSON.stringify(event);
    let n = 0;
    this.server?.clients?.forEach((c) => {
      if (c.readyState === 1 /* OPEN */) {
        c.send(msg);
        n++;
      }
    });
    if (n) this.log.debug(`broadcast → ${n} istemci`);
  }
}
