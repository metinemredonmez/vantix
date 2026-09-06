// Mock eşleştirme mantığı (saf fonksiyon → NATS'sız test edilebilir).
// Not: karşılaştırma için number kullanılır; wire'daki para değerleri STRING kalır (bu sadece mock).

export type Side = "BUY" | "SELL";

/** Bekleyen (resting) limit emri, gelen last fiyatıyla marketable mı? */
export function marketable(side: Side, limit: number, last: number): boolean {
  return side === "BUY" ? last <= limit : last >= limit;
}
