import { Injectable } from "@nestjs/common";
import type { OrderStatus } from "@vantix/shared";

const transitions: Record<OrderStatus, OrderStatus[]> = {
  DRAFT: ["PENDING_TRIGGER", "PENDING_RISK", "CANCELLED"],
  PENDING_TRIGGER: ["PENDING_RISK", "CANCELLED", "EXPIRED"],
  PENDING_RISK: ["SENDING", "REJECTED"],
  SENDING: ["WORKING", "REJECTED", "ERROR"],
  WORKING: ["PARTIALLY_FILLED", "FILLED", "CANCELLING", "EXPIRED", "ERROR"],
  PARTIALLY_FILLED: ["PARTIALLY_FILLED", "FILLED", "CANCELLING", "EXPIRED"],
  CANCELLING: ["CANCELLED", "FILLED", "PARTIALLY_FILLED", "ERROR"],
  FILLED: [], CANCELLED: [], REJECTED: [], EXPIRED: [], ERROR: [],
};

@Injectable()
export class OrderStateMachine {
  canTransition(from: OrderStatus, to: OrderStatus) { return transitions[from].includes(to); }
  assert(from: OrderStatus, to: OrderStatus) {
    if (!this.canTransition(from, to)) throw new Error(`Illegal transition ${from} -> ${to}`);
  }
  isTerminal(s: OrderStatus) { return transitions[s].length === 0; }
}
