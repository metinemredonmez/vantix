//! Trigger engine: STOP / TRAILING / CONDITIONAL leg'leri tick'e göre değerlendirir.
//! Sembol başına sıralı değerlendirme; tetiklenen leg native emre dönüşür.

use crate::order::{CondField, CondOp, Leg, Side, Tick};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TriggerRule {
    /// Klasik stop: SELL için last <= trigger, BUY için last >= trigger
    Stop { trigger: Decimal },
    /// Trailing stop: referans fiyatı lehte hareketle kayar, aleyhte `distance` kadar düşünce tetikler
    Trailing {
        distance: Decimal, // mutlak fiyat mesafesi
        extreme: Decimal,  // SELL için en yüksek, BUY için en düşük görülen fiyat
    },
    /// Şartlı: field op value
    Conditional {
        field: CondField,
        op: CondOp,
        value: Decimal,
    },
}

impl TriggerRule {
    /// Leg'den kural üret; tetik gerektirmiyorsa None
    pub fn from_leg(leg: &Leg, reference_price: Option<Decimal>) -> Option<TriggerRule> {
        if let Some(c) = &leg.condition {
            return Some(TriggerRule::Conditional { field: c.field, op: c.op, value: c.value });
        }
        if let Some(t) = leg.trigger_price {
            if leg.trail_amount.is_none() && leg.trail_percent.is_none() {
                return Some(TriggerRule::Stop { trigger: t });
            }
        }
        let r = reference_price.or(leg.trigger_price)?;
        if let Some(a) = leg.trail_amount {
            return Some(TriggerRule::Trailing { distance: a, extreme: r });
        }
        if let Some(p) = leg.trail_percent {
            return Some(TriggerRule::Trailing { distance: r * p / Decimal::from(100), extreme: r });
        }
        None
    }

    /// Some(price) → tetiklendi, bu fiyatla native emir üret. Trailing kuralında extreme güncellenir.
    pub fn evaluate(&mut self, side: Side, tick: &Tick) -> Option<Decimal> {
        let last = tick.last;
        match self {
            TriggerRule::Stop { trigger } => {
                let hit = match side {
                    Side::Sell => last <= *trigger,
                    Side::Buy => last >= *trigger,
                };
                hit.then_some(last)
            }
            TriggerRule::Trailing { distance, extreme } => match side {
                Side::Sell => {
                    if last > *extreme {
                        *extreme = last;
                    }
                    (last <= *extreme - *distance).then_some(last)
                }
                Side::Buy => {
                    if last < *extreme {
                        *extreme = last;
                    }
                    (last >= *extreme + *distance).then_some(last)
                }
            },
            TriggerRule::Conditional { field, op, value } => {
                let v = match field {
                    CondField::Last => Some(last),
                    CondField::Bid => tick.bid,
                    CondField::Ask => tick.ask,
                }?;
                let hit = match op {
                    CondOp::Gt => v > *value,
                    CondOp::Gte => v >= *value,
                    CondOp::Lt => v < *value,
                    CondOp::Lte => v <= *value,
                };
                hit.then_some(last)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn tick(p: Decimal) -> Tick {
        Tick { symbol: "THYAO".into(), last: p, bid: None, ask: None, ts_ms: 0 }
    }

    #[test]
    fn stop_sell_triggers_below() {
        let mut r = TriggerRule::Stop { trigger: dec!(121) };
        assert_eq!(r.evaluate(Side::Sell, &tick(dec!(122))), None);
        assert_eq!(r.evaluate(Side::Sell, &tick(dec!(121))), Some(dec!(121)));
    }

    #[test]
    fn trailing_sell_follows_high_then_triggers() {
        let mut r = TriggerRule::Trailing { distance: dec!(2), extreme: dec!(100) };
        assert_eq!(r.evaluate(Side::Sell, &tick(dec!(103))), None); // extreme → 103
        assert_eq!(r.evaluate(Side::Sell, &tick(dec!(105))), None); // extreme → 105
        assert_eq!(r.evaluate(Side::Sell, &tick(dec!(103.5))), None);
        assert_eq!(r.evaluate(Side::Sell, &tick(dec!(103))), Some(dec!(103))); // 105-2
    }
}
