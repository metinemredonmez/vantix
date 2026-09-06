import { Injectable } from "@nestjs/common";

/**
 * Tick feed (Matriks/Foreks/kurum feed'i) → NATS md.tick.<SYMBOL>
 * Trigger engine STOP/TRAILING/CONDITIONAL leg'lerini buradan besler.
 */
@Injectable()
export class MarketDataService {
  private last = new Map<string, number>();
  onTick(symbol: string, price: number) { this.last.set(symbol, price); /* TODO: trigger evaluate */ }
  getLast(symbol: string) { return this.last.get(symbol); }
}
