use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Buy,
    Sell,
}

/// Borsanın bildiği native emir tipleri
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NativeType {
    Limit,
    Market,
    MarketToLimit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum TimeInForce {
    #[default]
    Day,
    Ioc,
    Fok,
    Gtd,
}

/// Şartlı emir koşulu (fiyat bazlı)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Condition {
    pub symbol: String,
    pub field: CondField,
    pub op: CondOp,
    pub value: Decimal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CondField {
    Last,
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CondOp {
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Gte,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Lte,
}

/// Tek leg = borsaya gidebilecek en küçük birim.
/// trigger_price / trail_* / condition doluysa leg önce PendingTrigger'da bekler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leg {
    pub symbol: String,
    pub side: Side,
    pub qty: Decimal,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default = "default_native")]
    pub native_type: NativeType,
    #[serde(default)]
    pub tif: TimeInForce,
    #[serde(default)]
    pub trigger_price: Option<Decimal>,
    #[serde(default)]
    pub trail_amount: Option<Decimal>,
    #[serde(default)]
    pub trail_percent: Option<Decimal>,
    #[serde(default)]
    pub condition: Option<Condition>,
    /// GTD için son geçerlilik (epoch ms). tif=GTD ise zorunlu; tick zamanı geçince Expired.
    #[serde(default)]
    pub expire_at_ms: Option<i64>,
}

fn default_native() -> NativeType {
    NativeType::Limit
}

impl Leg {
    pub fn needs_trigger(&self) -> bool {
        self.trigger_price.is_some()
            || self.trail_amount.is_some()
            || self.trail_percent.is_some()
            || self.condition.is_some()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TriggerOn {
    Filled,
    Partial,
}

/// parent → child bağı. Child, parent `on` durumuna gelince aktive olur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub parent: usize,
    pub child: usize,
    pub on: TriggerOn,
}

/// Bracket / Chain / OCO / Scale-out — hepsi bu tek modelle ifade edilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderTree {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub tenant_id: String,
    pub account_id: String,
    pub broker_id: String,
    pub legs: Vec<Leg>,
    #[serde(default)]
    pub tree: Vec<Edge>,
    #[serde(default)]
    pub oco_groups: Vec<Vec<usize>>,
    #[serde(default)]
    pub client_ref: Option<String>,
}

impl OrderTree {
    /// Yapısal doğrulama: index'ler geçerli mi, döngü var mı, self-edge var mı
    pub fn validate(&self) -> Result<(), String> {
        let n = self.legs.len();
        if n == 0 {
            return Err("legs boş".into());
        }
        for e in &self.tree {
            if e.parent >= n || e.child >= n {
                return Err(format!("edge index aralık dışı: {}→{}", e.parent, e.child));
            }
            if e.parent == e.child {
                return Err("self-edge".into());
            }
        }
        for g in &self.oco_groups {
            for &i in g {
                if i >= n {
                    return Err(format!("oco index aralık dışı: {i}"));
                }
            }
        }
        // döngü kontrolü (DFS)
        let mut color = vec![0u8; n];
        fn dfs(u: usize, tree: &[Edge], color: &mut [u8]) -> bool {
            color[u] = 1;
            for e in tree.iter().filter(|e| e.parent == u) {
                match color[e.child] {
                    1 => return true,
                    0 => {
                        if dfs(e.child, tree, color) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            color[u] = 2;
            false
        }
        for i in 0..n {
            if color[i] == 0 && dfs(i, &self.tree, &mut color) {
                return Err("tree döngü içeriyor".into());
            }
        }
        for leg in &self.legs {
            if leg.qty <= Decimal::ZERO {
                return Err("qty > 0 olmalı".into());
            }
            if leg.native_type == NativeType::Limit && leg.price.is_none() {
                return Err(format!("{}: LIMIT için price gerekli", leg.symbol));
            }
            if leg.tif == TimeInForce::Gtd && leg.expire_at_ms.is_none() {
                return Err(format!("{}: GTD için expire_at_ms gerekli", leg.symbol));
            }
        }
        Ok(())
    }

    pub fn is_root(&self, i: usize) -> bool {
        !self.tree.iter().any(|e| e.child == i)
    }
}

/// Broker gateway'e giden native emir
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeOrder {
    pub ems_order_id: String, // "<tree_id>:<leg_index>"
    pub tenant_id: String,
    pub account_id: String,
    pub broker_id: String,
    pub symbol: String,
    pub side: Side,
    pub qty: Decimal,
    pub price: Option<Decimal>,
    pub native_type: NativeType,
    pub tif: TimeInForce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub ems_order_id: String,
    pub broker_order_id: String,
    pub qty: Decimal,
    pub price: Decimal,
    pub remaining: Decimal,
    pub ts_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tick {
    pub symbol: String,
    pub last: Decimal,
    #[serde(default)]
    pub bid: Option<Decimal>,
    #[serde(default)]
    pub ask: Option<Decimal>,
    pub ts_ms: i64,
}

// -------- NATS command payload'ları (per-subject, contracts ile tek kaynak) --------
// Bu struct'lar exec.order.cancel / exec.broker.* subject'lerinin gövdesidir.
// Şema: packages/contracts/schema/exec.schema.json (CancelRequest, BrokerAck, ...).

/// exec.order.cancel gövdesi
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CancelRequest {
    pub order_id: Uuid,
    #[serde(default)]
    pub leg: Option<usize>,
}

/// exec.broker.ack gövdesi
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrokerAck {
    pub ems_order_id: String,
    pub broker_order_id: String,
}

/// exec.broker.reject gövdesi
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrokerReject {
    pub ems_order_id: String,
    pub reason: String,
}

/// exec.broker.cancelled gövdesi
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrokerCancelled {
    pub ems_order_id: String,
}

/// exec.order.modify gövdesi (amend). price/qty verilenler değişir.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModifyRequest {
    pub order_id: Uuid,
    pub leg: usize,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default)]
    pub qty: Option<Decimal>,
}

/// exec.control.reconcile gövdesi. Gateway'in o an açık gördüğü broker order id'leri.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconcileRequest {
    pub open_broker_ids: Vec<String>,
}

/// exec.control.kill gövdesi (acil durdurma). tenant_id/account_id yoksa joker (hepsi).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KillRequest {
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    pub active: bool,
}
