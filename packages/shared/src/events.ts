/**
 * @deprecated GEÇİCİ. Tek kaynak DEĞİL — subject listesi packages/contracts/subjects.md,
 * payload'lar packages/contracts/schema/exec.schema.json'da. Bkz. docs/adr/0003.
 */
// NATS subject sözleşmesi — contracts/subjects.md ile hizalandı (eski `ems.*` yanlıştı).
export const Subjects = {
  orderSubmit: "exec.order.submit",
  orderCancel: "exec.order.cancel",
  orderStateChanged: "exec.order.state",
  brokerSend: "exec.broker.send",
  brokerAck: "exec.broker.ack",
  brokerFill: "exec.broker.fill",
  brokerReject: "exec.broker.reject",
  brokerCancel: "exec.broker.cancel",
  marketTick: "md.tick.*",          // md.tick.GARAN
  riskViolation: "exec.risk.violation",
} as const;

export interface FillEvent {
  orderId: string;
  brokerOrderId: string;
  symbol: string;
  qty: number;
  price: number;
  ts: string;
  remaining: number;
}
