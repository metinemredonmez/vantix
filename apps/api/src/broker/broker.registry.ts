import { Injectable, OnModuleInit } from "@nestjs/common";
import { BrokerAdapter, MockBrokerAdapter } from "@vantix/broker-adapters";
import type { FillEvent } from "@vantix/shared";

@Injectable()
export class BrokerRegistry implements OnModuleInit {
  private adapters = new Map<string, BrokerAdapter>();
  private fillCbs: ((f: FillEvent) => void)[] = [];

  async onModuleInit() {
    this.register(new MockBrokerAdapter());
    // TODO: register(new OsmanliAdapter(cfg)), IsYatirimAdapter, ... (env/config'e göre)
  }
  register(a: BrokerAdapter) {
    a.onFill(f => this.fillCbs.forEach(cb => cb(f)));
    this.adapters.set(a.id, a);
    return a.connect();
  }
  get(id: string): BrokerAdapter {
    const a = this.adapters.get(id); if (!a) throw new Error(`Unknown broker ${id}`); return a;
  }
  onFill(cb: (f: FillEvent) => void) { this.fillCbs.push(cb); }
}
