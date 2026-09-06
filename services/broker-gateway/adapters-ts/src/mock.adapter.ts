import type { BrokerAdapter, NativeOrder, BrokerAck, Position, Balance } from "./adapter";
import type { FillEvent } from "@vantix/shared";

/** Gerçek kurum bağlanana kadar: anında ACK, kısa gecikmeyle tam fill simülasyonu */
export class MockBrokerAdapter implements BrokerAdapter {
  readonly id = "mock";
  private fillCbs: ((f: FillEvent) => void)[] = [];
  private updCbs: ((id: string, s: string) => void)[] = [];
  private seq = 0;

  async connect() {}
  async disconnect() {}

  async placeOrder(o: NativeOrder): Promise<BrokerAck> {
    const brokerOrderId = `MOCK-${++this.seq}`;
    setTimeout(() => {
      this.updCbs.forEach(cb => cb(brokerOrderId, "WORKING"));
      this.fillCbs.forEach(cb => cb({
        orderId: o.emsOrderId, brokerOrderId, symbol: o.symbol,
        qty: o.qty, price: o.price ?? 0, ts: new Date().toISOString(), remaining: 0,
      }));
    }, 300);
    return { brokerOrderId, status: "ACCEPTED" };
  }
  async modifyOrder(brokerOrderId: string): Promise<BrokerAck> { return { brokerOrderId, status: "ACCEPTED" }; }
  async cancelOrder(brokerOrderId: string): Promise<BrokerAck> {
    this.updCbs.forEach(cb => cb(brokerOrderId, "CANCELLED"));
    return { brokerOrderId, status: "ACCEPTED" };
  }
  async getOrders(): Promise<NativeOrder[]> { return []; }
  async getPositions(): Promise<Position[]> { return []; }
  async getBalances(): Promise<Balance[]> { return [{ currency: "TRY", available: 1_000_000, total: 1_000_000 }]; }
  onFill(cb: (f: FillEvent) => void) { this.fillCbs.push(cb); }
  onOrderUpdate(cb: (id: string, s: string) => void) { this.updCbs.push(cb); }
}
