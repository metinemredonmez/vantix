import { Injectable, Logger, OnModuleDestroy, OnModuleInit } from "@nestjs/common";
import { connect, JSONCodec, type NatsConnection } from "nats";

/**
 * Tek NATS bağlantısı. Servisler yalnızca subject + contracts şemalarıyla konuşur.
 * api'nin görevi: web/mobile HTTP/WS ↔ NATS köprüsü (exec-core kararları verir).
 */
@Injectable()
export class NatsService implements OnModuleInit, OnModuleDestroy {
  private readonly log = new Logger(NatsService.name);
  private nc?: NatsConnection;
  private readonly jc = JSONCodec();

  async onModuleInit() {
    const servers = process.env.NATS_URL ?? "nats://localhost:4222";
    this.nc = await connect({ servers, name: "vantix-api" });
    this.log.log(`NATS bağlandı: ${servers}`);
  }

  async onModuleDestroy() {
    await this.nc?.drain();
  }

  publish(subject: string, payload: unknown) {
    if (!this.nc) throw new Error("NATS bağlı değil");
    this.nc.publish(subject, this.jc.encode(payload));
  }

  /** subject aboneliği; her mesaj için handler çağrılır (JSON decode edilmiş). */
  subscribe(subject: string, handler: (data: unknown, subject: string) => void) {
    if (!this.nc) throw new Error("NATS bağlı değil");
    const sub = this.nc.subscribe(subject);
    (async () => {
      for await (const m of sub) {
        try {
          handler(this.jc.decode(m.data), m.subject);
        } catch (e) {
          this.log.warn(`bad payload ${m.subject}: ${(e as Error).message}`);
        }
      }
    })();
    return sub;
  }
}
