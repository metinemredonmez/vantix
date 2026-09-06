import type { OrderLeg, FillEvent } from "@vantix/shared";

export interface NativeOrder extends OrderLeg { accountId: string; emsOrderId: string }
export interface BrokerAck { brokerOrderId: string; status: "ACCEPTED" | "REJECTED"; reason?: string }
export interface Position { symbol: string; qty: number; avgCost: number }
export interface Balance { currency: string; available: number; total: number }

/**
 * Tek sözleşme: her aracı kurum (Osmanlı, İş, Ak, Garanti...) bunu implement eder.
 * Üstteki Advanced Order Engine bu interface dışında hiçbir şeyi bilmez.
 */
export interface BrokerAdapter {
  readonly id: string;
  connect(): Promise<void>;
  disconnect(): Promise<void>;

  placeOrder(o: NativeOrder): Promise<BrokerAck>;
  modifyOrder(brokerOrderId: string, changes: Partial<Pick<NativeOrder, "qty" | "price">>): Promise<BrokerAck>;
  cancelOrder(brokerOrderId: string): Promise<BrokerAck>;

  getOrders(accountId: string): Promise<NativeOrder[]>;
  getPositions(accountId: string): Promise<Position[]>;
  getBalances(accountId: string): Promise<Balance[]>;

  onFill(cb: (f: FillEvent) => void): void;
  onOrderUpdate(cb: (brokerOrderId: string, status: string, raw?: unknown) => void): void;
}
