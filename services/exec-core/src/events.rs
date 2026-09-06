use crate::order::{Fill, NativeOrder, OrderTree, Tick};
use crate::state_machine::Status;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Motora giren komutlar (NATS: exec.order.*, exec.broker.*, md.tick.*)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Submit(OrderTree),
    Cancel { order_id: Uuid, leg: Option<usize> },
    /// Çalışan/bekleyen bir bacağın fiyat/miktarını değiştir (amend). Risk yeniden kontrol edilir.
    Modify { order_id: Uuid, leg: usize, price: Option<Decimal>, qty: Option<Decimal> },
    BrokerAck { ems_order_id: String, broker_order_id: String },
    BrokerReject { ems_order_id: String, reason: String },
    BrokerFill(Fill),
    BrokerCancelled { ems_order_id: String },
    Tick(Tick),
    /// Acil durdurma. tenant_id/account_id None = joker; active=false kapsamı kaldırır.
    KillSwitch { tenant_id: Option<String>, account_id: Option<String>, active: bool },
    /// Reconciliation: gateway'in bildirdiği açık broker order id'leri ile EMS'i karşılaştır.
    Reconcile { open_broker_ids: Vec<String> },
}

/// Motordan çıkan olaylar (NATS: exec.order.state, exec.broker.send, exec.risk.violation)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    StateChanged {
        order_id: Uuid,
        leg: usize,
        from: Status,
        to: Status,
        reason: Option<String>,
    },
    SendToBroker(NativeOrder),
    CancelAtBroker {
        ems_order_id: String,
        broker_order_id: String,
    },
    ModifyAtBroker {
        ems_order_id: String,
        broker_order_id: String,
        price: Option<Decimal>,
        qty: Option<Decimal>,
    },
    RiskViolation {
        order_id: Uuid,
        leg: usize,
        rule: String,
        detail: String,
    },
    Rejected {
        order_id: Uuid,
        reason: String,
    },
    /// Reconciliation uyuşmazlığı (otomatik düzeltme yok, operatör/işleme bildirilir).
    /// kind: "ems_orphan" (EMS canlı sanıyor, broker'da yok) | "broker_orphan" (broker'da var, EMS bilmiyor).
    ReconcileMismatch {
        ems_order_id: Option<String>,
        broker_order_id: String,
        kind: String,
    },
}
