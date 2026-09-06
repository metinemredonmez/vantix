//! Engine: saf, deterministik emir orkestrasyonu.
//! Komut alır → durum günceller → olay listesi döner. I/O yok.

use crate::events::{Command, Event};
use crate::order::*;
use crate::state_machine::Status;
use crate::trigger::TriggerRule;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegState {
    pub leg: Leg,
    pub status: Status,
    pub filled: Decimal,
    pub broker_order_id: Option<String>,
    pub rule: Option<TriggerRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeState {
    pub tree: OrderTree,
    pub legs: Vec<LegState>,
}

/// Boot snapshot: motor durumunun anlık kopyası. `seq` = bu duruma dahil son stream sequence'i.
/// Boot'ta snapshot yüklenir, JetStream yalnız seq'ten SONRAKİ komutları replay eder (tüm tarih değil).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub seq: u64,
    pub trees: HashMap<Uuid, TreeState>,
    pub last_price: HashMap<String, Decimal>,
    pub killed: Vec<(Option<String>, Option<String>)>,
}

/// Pre-trade risk kancası: motor risk politikası bilmez, dışarıdan verilir.
/// `reference`: sembolün son bilinen fiyatı (fat-finger/price-band için); yoksa None.
pub trait RiskPolicy: Send {
    fn check(&self, tree: &OrderTree, leg: &Leg, reference: Option<Decimal>) -> Result<(), (String, String)>; // (rule, detail)
}

pub struct NoRisk;
impl RiskPolicy for NoRisk {
    fn check(&self, _: &OrderTree, _: &Leg, _: Option<Decimal>) -> Result<(), (String, String)> {
        Ok(())
    }
}

/// Yalnız max qty (geriye dönük uyum / basit senaryolar).
pub struct BasicRisk {
    pub max_qty: Decimal,
}
impl RiskPolicy for BasicRisk {
    fn check(&self, _: &OrderTree, leg: &Leg, _: Option<Decimal>) -> Result<(), (String, String)> {
        if leg.qty > self.max_qty {
            return Err(("MAX_QTY".into(), format!("{} > {}", leg.qty, self.max_qty)));
        }
        Ok(())
    }
}

/// Pre-trade risk paketi (tenant bazlı config sonra). Alanlar None ise o kural kapalı.
/// - MAX_QTY: leg.qty > max_qty
/// - MAX_NOTIONAL: leg.qty * fiyat > max_notional  (fiyat = limit price, market'te reference)
/// - FAT_FINGER: |leg.price - reference| / reference * 100 > max_price_deviation_pct
pub struct CompositeRisk {
    pub max_qty: Option<Decimal>,
    pub max_notional: Option<Decimal>,
    pub max_price_deviation_pct: Option<Decimal>,
}
impl RiskPolicy for CompositeRisk {
    fn check(&self, _: &OrderTree, leg: &Leg, reference: Option<Decimal>) -> Result<(), (String, String)> {
        if let Some(mq) = self.max_qty {
            if leg.qty > mq {
                return Err(("MAX_QTY".into(), format!("{} > {}", leg.qty, mq)));
            }
        }
        // Notional için fiyat: limit price varsa o, yoksa market → reference.
        let px = leg.price.or(reference);
        if let (Some(mn), Some(p)) = (self.max_notional, px) {
            let notional = leg.qty * p;
            if notional > mn {
                return Err(("MAX_NOTIONAL".into(), format!("{notional} > {mn}")));
            }
        }
        // Fat-finger: yalnız limit price + reference varken.
        if let (Some(pct), Some(p), Some(r)) = (self.max_price_deviation_pct, leg.price, reference) {
            if r > Decimal::ZERO {
                let dev = (p - r).abs() / r * Decimal::from(100);
                if dev > pct {
                    return Err(("FAT_FINGER".into(), format!("{}% sapma > {}% (ref {})", dev.round_dp(2), pct, r)));
                }
            }
        }
        Ok(())
    }
}

pub struct Engine {
    trees: HashMap<Uuid, TreeState>,
    by_ems_id: HashMap<String, (Uuid, usize)>,
    last_price: HashMap<String, Decimal>,
    risk: Box<dyn RiskPolicy>,
    /// Aktif kill-switch kapsamları: (tenant?, account?). None = joker (hepsi).
    killed: Vec<(Option<String>, Option<String>)>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(Box::new(NoRisk))
    }
}

impl Engine {
    pub fn new(risk: Box<dyn RiskPolicy>) -> Self {
        Self {
            trees: HashMap::new(),
            by_ems_id: HashMap::new(),
            last_price: HashMap::new(),
            risk,
            killed: Vec::new(),
        }
    }

    pub fn tree(&self, id: &Uuid) -> Option<&TreeState> {
        self.trees.get(id)
    }

    /// Anlık durum kopyası (kalıcılaştırmak için). `seq` = son işlenen stream sequence.
    pub fn export_snapshot(&self, seq: u64) -> Snapshot {
        Snapshot {
            seq,
            trees: self.trees.clone(),
            last_price: self.last_price.clone(),
            killed: self.killed.clone(),
        }
    }

    /// Snapshot'tan durumu geri yükle (boot). by_ems_id ağaçlardan yeniden kurulur.
    pub fn restore(&mut self, snap: Snapshot) {
        self.trees = snap.trees;
        self.last_price = snap.last_price;
        self.killed = snap.killed;
        self.by_ems_id.clear();
        for (id, ts) in &self.trees {
            for i in 0..ts.legs.len() {
                self.by_ems_id.insert(format!("{id}:{i}"), (*id, i));
            }
        }
    }

    pub fn apply(&mut self, cmd: Command) -> Vec<Event> {
        let mut ev = Vec::new();
        match cmd {
            Command::Submit(tree) => self.submit(tree, &mut ev),
            Command::Cancel { order_id, leg } => self.cancel(order_id, leg, &mut ev),
            Command::Modify { order_id, leg, price, qty } => self.modify(order_id, leg, price, qty, &mut ev),
            Command::BrokerAck { ems_order_id, broker_order_id } => {
                if let Some((id, i)) = self.lookup(&ems_order_id) {
                    self.set_broker_id(id, i, broker_order_id);
                    self.transition(id, i, Status::Working, None, &mut ev);
                }
            }
            Command::BrokerReject { ems_order_id, reason } => {
                if let Some((id, i)) = self.lookup(&ems_order_id) {
                    self.transition(id, i, Status::Rejected, Some(reason), &mut ev);
                }
            }
            Command::BrokerFill(f) => self.on_fill(f, &mut ev),
            Command::BrokerCancelled { ems_order_id } => {
                if let Some((id, i)) = self.lookup(&ems_order_id) {
                    self.transition(id, i, Status::Cancelled, None, &mut ev);
                }
            }
            Command::Tick(t) => self.on_tick(t, &mut ev),
            Command::KillSwitch { tenant_id, account_id, active } => {
                self.kill_switch(tenant_id, account_id, active, &mut ev)
            }
            Command::Reconcile { open_broker_ids } => self.reconcile(open_broker_ids, &mut ev),
        }
        ev
    }

    // ---------------- submit ----------------
    fn submit(&mut self, tree: OrderTree, ev: &mut Vec<Event>) {
        if let Err(reason) = tree.validate() {
            ev.push(Event::Rejected { order_id: tree.id, reason });
            return;
        }
        if self.is_killed(&tree.tenant_id, &tree.account_id) {
            ev.push(Event::Rejected { order_id: tree.id, reason: "kill switch aktif".into() });
            return;
        }
        let id = tree.id;
        let legs = tree
            .legs
            .iter()
            .map(|l| LegState { leg: l.clone(), status: Status::Draft, filled: Decimal::ZERO, broker_order_id: None, rule: None })
            .collect();
        self.trees.insert(id, TreeState { tree, legs });
        let n = self.trees[&id].legs.len();
        for i in 0..n {
            let ems = format!("{id}:{i}");
            self.by_ems_id.insert(ems, (id, i));
            if self.trees[&id].tree.is_root(i) {
                self.activate(id, i, ev);
            } else {
                self.transition(id, i, Status::PendingTrigger, Some("waiting parent".into()), ev);
            }
        }
    }

    /// Leg'i canlandır: risk → (trigger bekle | broker'a gönder)
    fn activate(&mut self, id: Uuid, i: usize, ev: &mut Vec<Event>) {
        let ts = &self.trees[&id];
        let leg = ts.legs[i].leg.clone();
        let tree = ts.tree.clone();

        self.transition(id, i, Status::PendingRisk, None, ev);
        let reference = self.last_price.get(&leg.symbol).copied();
        if let Err((rule, detail)) = self.risk.check(&tree, &leg, reference) {
            ev.push(Event::RiskViolation { order_id: id, leg: i, rule: rule.clone(), detail: detail.clone() });
            self.transition(id, i, Status::Rejected, Some(rule), ev);
            return;
        }

        if leg.needs_trigger() {
            let rule = TriggerRule::from_leg(&leg, reference);
            self.trees.get_mut(&id).unwrap().legs[i].rule = rule;
            self.transition(id, i, Status::PendingTrigger, Some("waiting trigger".into()), ev);
            return;
        }
        self.send(id, i, leg.price, leg.native_type, ev);
    }

    fn send(&mut self, id: Uuid, i: usize, price: Option<Decimal>, native_type: NativeType, ev: &mut Vec<Event>) {
        self.transition(id, i, Status::Sending, None, ev);
        let ts = &self.trees[&id];
        let l = &ts.legs[i].leg;
        ev.push(Event::SendToBroker(NativeOrder {
            ems_order_id: format!("{id}:{i}"),
            tenant_id: ts.tree.tenant_id.clone(),
            account_id: ts.tree.account_id.clone(),
            broker_id: ts.tree.broker_id.clone(),
            symbol: l.symbol.clone(),
            side: l.side,
            qty: l.qty - ts.legs[i].filled,
            price,
            native_type,
            tif: l.tif,
        }));
    }

    // ---------------- cancel ----------------
    fn cancel(&mut self, id: Uuid, leg: Option<usize>, ev: &mut Vec<Event>) {
        let Some(ts) = self.trees.get(&id) else { return };
        let idxs: Vec<usize> = match leg {
            Some(i) => vec![i],
            None => (0..ts.legs.len()).collect(),
        };
        for i in idxs {
            self.cancel_leg(id, i, ev);
        }
    }

    fn cancel_leg(&mut self, id: Uuid, i: usize, ev: &mut Vec<Event>) {
        let st = self.trees[&id].legs[i].status;
        if st.is_terminal() || st == Status::Cancelling {
            return;
        }
        if st.is_live_at_broker() {
            if let Some(bid) = self.trees[&id].legs[i].broker_order_id.clone() {
                self.transition(id, i, Status::Cancelling, None, ev);
                ev.push(Event::CancelAtBroker { ems_order_id: format!("{id}:{i}"), broker_order_id: bid });
                return; // BrokerCancelled gelince CANCELLED olur
            }
        }
        self.transition(id, i, Status::Cancelled, None, ev);
    }

    // ---------------- modify (amend) ----------------
    /// Bir bacağın price/qty'sini değiştir. Risk yeniden kontrol edilir; borsadaysa ModifyAtBroker yayılır.
    fn modify(&mut self, id: Uuid, leg: usize, price: Option<Decimal>, qty: Option<Decimal>, ev: &mut Vec<Event>) {
        let Some(ts) = self.trees.get(&id) else { return };
        if leg >= ts.legs.len() {
            return;
        }
        let status = ts.legs[leg].status;
        if status.is_terminal() {
            return; // biten emir değiştirilemez
        }
        let filled = ts.legs[leg].filled;
        let tree = ts.tree.clone();
        let mut cand = ts.legs[leg].leg.clone();
        if let Some(p) = price {
            cand.price = Some(p);
        }
        if let Some(q) = qty {
            cand.qty = q;
        }
        if cand.qty <= filled {
            tracing::warn!(%id, leg, "modify qty ({}) <= gerçekleşen ({}), yok sayıldı", cand.qty, filled);
            return;
        }
        // Değişen emir için pre-trade risk yeniden çalışır (fat-finger/notional yeni değerlere göre).
        let reference = self.last_price.get(&cand.symbol).copied();
        if let Err((rule, detail)) = self.risk.check(&tree, &cand, reference) {
            ev.push(Event::RiskViolation { order_id: id, leg, rule, detail });
            return; // reddedildi, emir olduğu gibi kalır
        }
        // Uygula
        let bid = {
            let ls = &mut self.trees.get_mut(&id).unwrap().legs[leg];
            ls.leg.price = cand.price;
            ls.leg.qty = cand.qty;
            ls.broker_order_id.clone()
        };
        // Borsada açıksa gerçek değişikliği ilet; değilse (PendingTrigger) yalnız yerel güncellenir.
        if status.is_live_at_broker() {
            if let Some(broker_order_id) = bid {
                ev.push(Event::ModifyAtBroker {
                    ems_order_id: format!("{id}:{leg}"),
                    broker_order_id,
                    price: cand.price,
                    qty: Some(cand.qty),
                });
            }
        }
    }

    // ---------------- fill ----------------
    fn on_fill(&mut self, f: Fill, ev: &mut Vec<Event>) {
        let Some((id, i)) = self.lookup(&f.ems_order_id) else { return };
        {
            let ls = &mut self.trees.get_mut(&id).unwrap().legs[i];
            ls.filled += f.qty;
            if ls.broker_order_id.is_none() {
                ls.broker_order_id = Some(f.broker_order_id.clone());
            }
        }
        let (filled, qty) = {
            let ls = &self.trees[&id].legs[i];
            (ls.filled, ls.leg.qty)
        };
        let to = if f.remaining == Decimal::ZERO || filled >= qty { Status::Filled } else { Status::PartiallyFilled };
        self.transition(id, i, to, None, ev);
        self.last_price.insert(self.trees[&id].legs[i].leg.symbol.clone(), f.price);

        // OCO: aynı gruptaki kardeşleri iptal et
        let groups: Vec<Vec<usize>> = self.trees[&id].tree.oco_groups.clone();
        for g in groups.iter().filter(|g| g.contains(&i)) {
            for &j in g.iter().filter(|&&j| j != i) {
                self.cancel_leg(id, j, ev);
            }
        }

        // OTO / Chain / Bracket: child'ları aktive et
        let edges: Vec<Edge> = self.trees[&id].tree.tree.clone();
        for e in edges.iter().filter(|e| e.parent == i) {
            let ready = match e.on {
                TriggerOn::Filled => to == Status::Filled,
                TriggerOn::Partial => filled > Decimal::ZERO,
            };
            let child_status = self.trees[&id].legs[e.child].status;
            if ready && child_status == Status::PendingTrigger && self.trees[&id].legs[e.child].rule.is_none() {
                // child qty, parent'ın gerçekleşen miktarını aşamaz (stop qty = gerçek pozisyon)
                {
                    let ts = self.trees.get_mut(&id).unwrap();
                    let cq = ts.legs[e.child].leg.qty;
                    ts.legs[e.child].leg.qty = cq.min(filled);
                }
                self.activate(id, e.child, ev);
            }
        }
    }

    // ---------------- tick / trigger ----------------
    fn on_tick(&mut self, t: Tick, ev: &mut Vec<Event>) {
        self.last_price.insert(t.symbol.clone(), t.last);
        // Zaman global ilerler: herhangi bir tick, süresi geçen GTD emirlerini Expired yapar.
        self.expire_gtd(t.ts_ms, ev);
        // Deterministik sıra: tree id'ye göre sıralı, sonra leg index
        let mut ids: Vec<Uuid> = self.trees.keys().copied().collect();
        ids.sort();
        for id in ids {
            let n = self.trees[&id].legs.len();
            for i in 0..n {
                let (matches, side) = {
                    let ls = &self.trees[&id].legs[i];
                    (ls.status == Status::PendingTrigger && ls.rule.is_some() && ls.leg.symbol == t.symbol, ls.leg.side)
                };
                if !matches {
                    continue;
                }
                let hit = self.trees.get_mut(&id).unwrap().legs[i].rule.as_mut().unwrap().evaluate(side, &t);
                if let Some(px) = hit {
                    let (price, nt) = {
                        let l = &self.trees[&id].legs[i].leg;
                        match l.native_type {
                            NativeType::Market => (None, NativeType::Market),
                            _ => (Some(l.price.unwrap_or(px)), NativeType::Limit),
                        }
                    };
                    // PendingTrigger → PendingRisk → Sending
                    self.transition(id, i, Status::PendingRisk, Some(format!("triggered @ {px}")), ev);
                    self.send(id, i, price, nt, ev);
                }
            }
        }
    }

    // ---------------- GTD expiry ----------------
    /// tif=GTD ve expire_at_ms <= now olan canlı/bekleyen bacakları Expired yapar.
    /// Borsada açıksa temizlik için CancelAtBroker da yayılır. IOC/FOK broker-native (pass-through).
    fn expire_gtd(&mut self, now_ms: i64, ev: &mut Vec<Event>) {
        let mut ids: Vec<Uuid> = self.trees.keys().copied().collect();
        ids.sort();
        for id in ids {
            let n = self.trees[&id].legs.len();
            for i in 0..n {
                let (expired, live, bid) = {
                    let ls = &self.trees[&id].legs[i];
                    let due = ls.leg.tif == TimeInForce::Gtd
                        && ls.leg.expire_at_ms.map_or(false, |e| e <= now_ms)
                        && matches!(ls.status, Status::Working | Status::PartiallyFilled | Status::PendingTrigger);
                    (due, ls.status.is_live_at_broker(), ls.broker_order_id.clone())
                };
                if !expired {
                    continue;
                }
                if live {
                    if let Some(broker_order_id) = bid {
                        ev.push(Event::CancelAtBroker { ems_order_id: format!("{id}:{i}"), broker_order_id });
                    }
                }
                self.transition(id, i, Status::Expired, Some("GTD expired".into()), ev);
            }
        }
    }

    // ---------------- kill switch ----------------
    /// Acil durdurma: kapsamdaki (tenant?/account?) tüm canlı emirleri iptal et ve yeni
    /// submit'leri blokla. `active=false` kapsamı kaldırır. None alan = joker.
    fn kill_switch(
        &mut self,
        tenant_id: Option<String>,
        account_id: Option<String>,
        active: bool,
        ev: &mut Vec<Event>,
    ) {
        let scope = (tenant_id, account_id);
        if !active {
            self.killed.retain(|s| s != &scope);
            return;
        }
        if !self.killed.contains(&scope) {
            self.killed.push(scope.clone());
        }
        // Kapsamdaki tüm ağaçların canlı bacaklarını iptal et.
        let ids: Vec<Uuid> = self.trees.keys().copied().collect();
        for id in ids {
            let (t, a) = {
                let tr = &self.trees[&id].tree;
                (tr.tenant_id.clone(), tr.account_id.clone())
            };
            if scope_matches(&scope, &t, &a) {
                let n = self.trees[&id].legs.len();
                for i in 0..n {
                    self.cancel_leg(id, i, ev);
                }
            }
        }
    }

    fn is_killed(&self, tenant: &str, account: &str) -> bool {
        self.killed.iter().any(|s| scope_matches(s, tenant, account))
    }

    // ---------------- reconciliation ----------------
    /// Gateway'in bildirdiği açık broker id'leri ile EMS canlı bacaklarını karşılaştırıp uyuşmazlık yayar.
    fn reconcile(&self, open_broker_ids: Vec<String>, ev: &mut Vec<Event>) {
        use std::collections::HashSet;
        let open: HashSet<&str> = open_broker_ids.iter().map(|s| s.as_str()).collect();
        let mut known: HashSet<String> = HashSet::new();
        let mut ids: Vec<Uuid> = self.trees.keys().copied().collect();
        ids.sort();
        for id in ids {
            let n = self.trees[&id].legs.len();
            for i in 0..n {
                let ls = &self.trees[&id].legs[i];
                if !ls.status.is_live_at_broker() {
                    continue;
                }
                if let Some(bid) = &ls.broker_order_id {
                    known.insert(bid.clone());
                    if !open.contains(bid.as_str()) {
                        // EMS canlı sanıyor ama broker açık listesinde yok.
                        ev.push(Event::ReconcileMismatch {
                            ems_order_id: Some(format!("{id}:{i}")),
                            broker_order_id: bid.clone(),
                            kind: "ems_orphan".into(),
                        });
                    }
                }
            }
        }
        for b in &open_broker_ids {
            if !known.contains(b) {
                // Broker'da açık ama EMS bilmiyor.
                ev.push(Event::ReconcileMismatch {
                    ems_order_id: None,
                    broker_order_id: b.clone(),
                    kind: "broker_orphan".into(),
                });
            }
        }
    }

    // ---------------- helpers ----------------
    fn lookup(&self, ems: &str) -> Option<(Uuid, usize)> {
        self.by_ems_id.get(ems).copied()
    }

    fn set_broker_id(&mut self, id: Uuid, i: usize, bid: String) {
        self.trees.get_mut(&id).unwrap().legs[i].broker_order_id = Some(bid);
    }

    fn transition(&mut self, id: Uuid, i: usize, to: Status, reason: Option<String>, ev: &mut Vec<Event>) {
        let ls = &mut self.trees.get_mut(&id).unwrap().legs[i];
        let from = ls.status;
        match from.transition(to) {
            Ok(s) => {
                ls.status = s;
                ev.push(Event::StateChanged { order_id: id, leg: i, from, to, reason });
            }
            Err(e) => {
                tracing::warn!(%id, leg = i, "{e}");
            }
        }
    }
}

/// Kill-switch kapsamı bir (tenant, account) ile eşleşiyor mu? None alan = joker.
fn scope_matches(scope: &(Option<String>, Option<String>), tenant: &str, account: &str) -> bool {
    let (st, sa) = scope;
    st.as_deref().map_or(true, |t| t == tenant) && sa.as_deref().map_or(true, |a| a == account)
}

// ======================= TESTS =======================
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn leg(sym: &str, side: Side, qty: Decimal, price: Option<Decimal>) -> Leg {
        Leg {
            symbol: sym.into(), side, qty, price,
            native_type: if price.is_some() { NativeType::Limit } else { NativeType::Market },
            tif: TimeInForce::Day, trigger_price: None, trail_amount: None, trail_percent: None, condition: None,
            expire_at_ms: None,
        }
    }

    /// Bracket: BUY 1000 @125 → TP SELL 1000 @130, SL SELL 1000 stop 121 (OCO)
    fn bracket() -> OrderTree {
        let mut sl = leg("GARAN", Side::Sell, dec!(1000), None);
        sl.trigger_price = Some(dec!(121));
        OrderTree {
            id: Uuid::new_v4(), tenant_id: "t1".into(), account_id: "a1".into(), broker_id: "mock".into(),
            legs: vec![leg("GARAN", Side::Buy, dec!(1000), Some(dec!(125))), leg("GARAN", Side::Sell, dec!(1000), Some(dec!(130))), sl],
            tree: vec![Edge { parent: 0, child: 1, on: TriggerOn::Filled }, Edge { parent: 0, child: 2, on: TriggerOn::Filled }],
            oco_groups: vec![vec![1, 2]],
            client_ref: None,
        }
    }

    fn sends(ev: &[Event]) -> Vec<&NativeOrder> {
        ev.iter().filter_map(|e| if let Event::SendToBroker(o) = e { Some(o) } else { None }).collect()
    }

    #[test]
    fn bracket_only_parent_goes_to_broker_first() {
        let mut e = Engine::default();
        let t = bracket();
        let id = t.id;
        let ev = e.apply(Command::Submit(t));
        let s = sends(&ev);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].ems_order_id, format!("{id}:0"));
        assert_eq!(e.tree(&id).unwrap().legs[1].status, Status::PendingTrigger);
        assert_eq!(e.tree(&id).unwrap().legs[2].status, Status::PendingTrigger);
    }

    #[test]
    fn parent_fill_activates_tp_and_arms_stop() {
        let mut e = Engine::default();
        let t = bracket();
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });
        let ev = e.apply(Command::BrokerFill(Fill {
            ems_order_id: format!("{id}:0"), broker_order_id: "B1".into(),
            qty: dec!(1000), price: dec!(125), remaining: dec!(0), ts_ms: 1,
        }));
        let s = sends(&ev);
        assert_eq!(s.len(), 1, "sadece TP borsaya gider, stop tetik bekler");
        assert_eq!(s[0].ems_order_id, format!("{id}:1"));
        let ts = e.tree(&id).unwrap();
        assert_eq!(ts.legs[0].status, Status::Filled);
        assert_eq!(ts.legs[1].status, Status::Sending);
        assert_eq!(ts.legs[2].status, Status::PendingTrigger);
        assert!(ts.legs[2].rule.is_some());
    }

    #[test]
    fn stop_triggers_on_tick_and_oco_cancels_tp() {
        let mut e = Engine::default();
        let t = bracket();
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });
        e.apply(Command::BrokerFill(Fill { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into(), qty: dec!(1000), price: dec!(125), remaining: dec!(0), ts_ms: 1 }));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:1"), broker_order_id: "B2".into() });

        // fiyat 121'e düşer → stop tetiklenir, MARKET olarak gider
        let ev = e.apply(Command::Tick(Tick { symbol: "GARAN".into(), last: dec!(120.9), bid: None, ask: None, ts_ms: 2 }));
        let s = sends(&ev);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].native_type, NativeType::Market);
        assert_eq!(s[0].ems_order_id, format!("{id}:2"));

        // stop dolar → OCO kardeş TP (B2) borsada iptal edilir
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:2"), broker_order_id: "B3".into() });
        let ev = e.apply(Command::BrokerFill(Fill { ems_order_id: format!("{id}:2"), broker_order_id: "B3".into(), qty: dec!(1000), price: dec!(120.9), remaining: dec!(0), ts_ms: 3 }));
        assert!(ev.iter().any(|x| matches!(x, Event::CancelAtBroker { broker_order_id, .. } if broker_order_id == "B2")));
        assert_eq!(e.tree(&id).unwrap().legs[1].status, Status::Cancelling);
        e.apply(Command::BrokerCancelled { ems_order_id: format!("{id}:1") });
        assert_eq!(e.tree(&id).unwrap().legs[1].status, Status::Cancelled);
    }

    #[test]
    fn partial_parent_fill_shrinks_child_qty() {
        let mut e = Engine::default();
        let mut t = bracket();
        t.tree[0].on = TriggerOn::Partial; // TP kısmi fill'de de aktive olsun
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });
        let ev = e.apply(Command::BrokerFill(Fill { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into(), qty: dec!(400), price: dec!(125), remaining: dec!(600), ts_ms: 1 }));
        let s = sends(&ev);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].qty, dec!(400), "child qty parent'ın gerçekleşen 400'üne iner");
    }

    #[test]
    fn risk_rejects_and_emits_violation() {
        let mut e = Engine::new(Box::new(BasicRisk { max_qty: dec!(500) }));
        let t = bracket();
        let id = t.id;
        let ev = e.apply(Command::Submit(t));
        assert!(ev.iter().any(|x| matches!(x, Event::RiskViolation { rule, .. } if rule == "MAX_QTY")));
        assert_eq!(e.tree(&id).unwrap().legs[0].status, Status::Rejected);
        assert!(sends(&ev).is_empty());
    }

    #[test]
    fn invalid_tree_rejected() {
        let mut e = Engine::default();
        let mut t = bracket();
        t.tree.push(Edge { parent: 1, child: 0, on: TriggerOn::Filled }); // döngü
        let ev = e.apply(Command::Submit(t));
        assert!(matches!(ev[0], Event::Rejected { .. }));
    }

    #[test]
    fn trailing_stop_end_to_end() {
        let mut e = Engine::default();
        let mut sl = leg("THYAO", Side::Sell, dec!(100), None);
        sl.trail_amount = Some(dec!(2));
        sl.trigger_price = Some(dec!(300)); // referans
        let t = OrderTree { id: Uuid::new_v4(), tenant_id: "t".into(), account_id: "a".into(), broker_id: "mock".into(), legs: vec![sl], tree: vec![], oco_groups: vec![], client_ref: None };
        let id = t.id;
        assert!(sends(&e.apply(Command::Submit(t))).is_empty());
        let tk = |p: Decimal| Command::Tick(Tick { symbol: "THYAO".into(), last: p, bid: None, ask: None, ts_ms: 0 });
        assert!(sends(&e.apply(tk(dec!(305)))).is_empty()); // extreme 305
        assert!(sends(&e.apply(tk(dec!(304)))).is_empty());
        let ev = e.apply(tk(dec!(303))); // 305-2
        assert_eq!(sends(&ev).len(), 1);
        assert_eq!(e.tree(&id).unwrap().legs[0].status, Status::Sending);
    }

    fn modifies(ev: &[Event]) -> Vec<&Event> {
        ev.iter().filter(|e| matches!(e, Event::ModifyAtBroker { .. })).collect()
    }

    fn gtd_leg(expire_at_ms: i64) -> OrderTree {
        let mut l = leg("THYAO", Side::Buy, dec!(100), Some(dec!(50)));
        l.tif = TimeInForce::Gtd;
        l.expire_at_ms = Some(expire_at_ms);
        OrderTree {
            id: Uuid::new_v4(), tenant_id: "t".into(), account_id: "a".into(), broker_id: "mock".into(),
            legs: vec![l], tree: vec![], oco_groups: vec![], client_ref: None,
        }
    }

    #[test]
    fn gtd_without_expiry_rejected() {
        let mut e = Engine::default();
        let mut t = gtd_leg(100);
        t.legs[0].expire_at_ms = None;
        let ev = e.apply(Command::Submit(t));
        assert!(matches!(ev[0], Event::Rejected { .. }));
    }

    #[test]
    fn gtd_expires_on_tick_after_deadline() {
        let mut e = Engine::default();
        let t = gtd_leg(100);
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });
        assert_eq!(e.tree(&id).unwrap().legs[0].status, Status::Working);
        // deadline öncesi tick → hâlâ Working
        e.apply(Command::Tick(Tick { symbol: "THYAO".into(), last: dec!(50), bid: None, ask: None, ts_ms: 90 }));
        assert_eq!(e.tree(&id).unwrap().legs[0].status, Status::Working);
        // deadline sonrası tick → Expired + borsada cancel
        let ev = e.apply(Command::Tick(Tick { symbol: "THYAO".into(), last: dec!(50), bid: None, ask: None, ts_ms: 150 }));
        assert_eq!(e.tree(&id).unwrap().legs[0].status, Status::Expired);
        assert!(ev.iter().any(|x| matches!(x, Event::CancelAtBroker { broker_order_id, .. } if broker_order_id == "B1")));
    }

    #[test]
    fn modify_working_order_emits_modify_at_broker() {
        let mut e = Engine::default();
        let t = bracket();
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });
        // leg0 WORKING @125 → fiyatı 124'e, miktarı 800'e çek
        let ev = e.apply(Command::Modify { order_id: id, leg: 0, price: Some(dec!(124)), qty: Some(dec!(800)) });
        let m = modifies(&ev);
        assert_eq!(m.len(), 1);
        assert!(matches!(m[0], Event::ModifyAtBroker { broker_order_id, price, qty, .. }
            if broker_order_id == "B1" && *price == Some(dec!(124)) && *qty == Some(dec!(800))));
        let ls = &e.tree(&id).unwrap().legs[0];
        assert_eq!(ls.leg.price, Some(dec!(124)));
        assert_eq!(ls.leg.qty, dec!(800));
    }

    #[test]
    fn modify_rejected_by_risk_keeps_order() {
        // fat-finger %20; referans 100; leg0 @125 submit için ref yok (geçer), sonra tick 100 gelir,
        // modify 130 → %30 sapma → RiskViolation, ModifyAtBroker YOK, fiyat 125 kalır.
        let mut e = Engine::new(Box::new(CompositeRisk { max_qty: None, max_notional: None, max_price_deviation_pct: Some(dec!(20)) }));
        let t = bracket();
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });
        e.apply(Command::Tick(Tick { symbol: "GARAN".into(), last: dec!(100), bid: None, ask: None, ts_ms: 1 }));
        let ev = e.apply(Command::Modify { order_id: id, leg: 0, price: Some(dec!(130)), qty: None });
        assert!(ev.iter().any(|x| matches!(x, Event::RiskViolation { rule, .. } if rule == "FAT_FINGER")));
        assert!(modifies(&ev).is_empty());
        assert_eq!(e.tree(&id).unwrap().legs[0].leg.price, Some(dec!(125)), "risk reddi fiyatı değiştirmez");
    }

    #[test]
    fn modify_qty_below_filled_ignored() {
        let mut e = Engine::default();
        let mut t = bracket();
        t.tree[0].on = TriggerOn::Partial;
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });
        e.apply(Command::BrokerFill(Fill { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into(), qty: dec!(400), price: dec!(125), remaining: dec!(600), ts_ms: 1 }));
        // 300 < gerçekleşen 400 → yok sayılır
        let ev = e.apply(Command::Modify { order_id: id, leg: 0, price: None, qty: Some(dec!(300)) });
        assert!(modifies(&ev).is_empty());
        assert_eq!(e.tree(&id).unwrap().legs[0].leg.qty, dec!(1000), "geçersiz qty modify yok sayılır");
    }

    #[test]
    fn reconcile_flags_ems_and_broker_orphans() {
        let mut e = Engine::default();
        let t = bracket();
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "MOCK-1".into() });
        // broker MOCK-1'i açık göstermiyor ama MOCK-9 diye bilmediğimiz bir emir var
        let ev = e.apply(Command::Reconcile { open_broker_ids: vec!["MOCK-9".into()] });
        assert!(ev.iter().any(|x| matches!(x, Event::ReconcileMismatch { broker_order_id, kind, .. }
            if broker_order_id == "MOCK-1" && kind == "ems_orphan")));
        assert!(ev.iter().any(|x| matches!(x, Event::ReconcileMismatch { broker_order_id, kind, .. }
            if broker_order_id == "MOCK-9" && kind == "broker_orphan")));
        // broker MOCK-1'i açık gösterirse uyuşmazlık yok
        let ev2 = e.apply(Command::Reconcile { open_broker_ids: vec!["MOCK-1".into()] });
        assert!(!ev2.iter().any(|x| matches!(x, Event::ReconcileMismatch { .. })));
    }

    #[test]
    fn snapshot_export_restore_rebuilds_state() {
        let mut e = Engine::default();
        let t = bracket();
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });
        e.apply(Command::BrokerFill(Fill { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into(), qty: dec!(1000), price: dec!(125), remaining: dec!(0), ts_ms: 1 }));
        // durum: leg0 FILLED, leg1 Sending (TP), leg2 PendingTrigger armed
        let snap = e.export_snapshot(42);
        let json = serde_json::to_vec(&snap).unwrap();

        // yeni motora yükle (JSON üzerinden — kalıcılık yolu)
        let restored: Snapshot = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored.seq, 42);
        let mut e2 = Engine::default();
        e2.restore(restored);
        let ts = e2.tree(&id).unwrap();
        assert_eq!(ts.legs[0].status, Status::Filled);
        assert_eq!(ts.legs[1].status, Status::Sending);
        assert_eq!(ts.legs[2].status, Status::PendingTrigger);
        assert!(ts.legs[2].rule.is_some(), "stop kuralı snapshot'ta korunur");

        // by_ems_id de kurulmuş olmalı: yeni motorda leg2 tick ile tetiklenebilir
        let ev = e2.apply(Command::Tick(Tick { symbol: "GARAN".into(), last: dec!(121), bid: None, ask: None, ts_ms: 2 }));
        assert_eq!(sends(&ev).len(), 1, "restore sonrası stop tetiklenir");
    }

    #[test]
    fn composite_risk_max_notional() {
        // leg0: BUY 1000 @125 = 125_000 notional > 100_000 limit
        let mut e = Engine::new(Box::new(CompositeRisk {
            max_qty: None,
            max_notional: Some(dec!(100_000)),
            max_price_deviation_pct: None,
        }));
        let t = bracket();
        let id = t.id;
        let ev = e.apply(Command::Submit(t));
        assert!(ev.iter().any(|x| matches!(x, Event::RiskViolation { rule, .. } if rule == "MAX_NOTIONAL")));
        assert_eq!(e.tree(&id).unwrap().legs[0].status, Status::Rejected);
        assert!(sends(&ev).is_empty());
    }

    #[test]
    fn composite_risk_fat_finger() {
        let mut e = Engine::new(Box::new(CompositeRisk {
            max_qty: None,
            max_notional: None,
            max_price_deviation_pct: Some(dec!(20)),
        }));
        // referans son fiyat 100; leg0 @125 → %25 sapma > %20 → FAT_FINGER
        e.apply(Command::Tick(Tick { symbol: "GARAN".into(), last: dec!(100), bid: None, ask: None, ts_ms: 0 }));
        let t = bracket();
        let id = t.id;
        let ev = e.apply(Command::Submit(t));
        assert!(ev.iter().any(|x| matches!(x, Event::RiskViolation { rule, .. } if rule == "FAT_FINGER")));
        assert_eq!(e.tree(&id).unwrap().legs[0].status, Status::Rejected);
    }

    #[test]
    fn kill_switch_cancels_live_and_blocks_new_then_resume() {
        let mut e = Engine::default();
        let t = bracket(); // tenant t1 / account a1
        let id = t.id;
        e.apply(Command::Submit(t));
        e.apply(Command::BrokerAck { ems_order_id: format!("{id}:0"), broker_order_id: "B1".into() });

        // kill: hesap a1 → canlı leg0 borsada iptal, bekleyen leg1/leg2 iptal
        let ev = e.apply(Command::KillSwitch {
            tenant_id: Some("t1".into()),
            account_id: Some("a1".into()),
            active: true,
        });
        assert!(ev.iter().any(|x| matches!(x, Event::CancelAtBroker { broker_order_id, .. } if broker_order_id == "B1")));
        assert_eq!(e.tree(&id).unwrap().legs[0].status, Status::Cancelling);
        assert_eq!(e.tree(&id).unwrap().legs[1].status, Status::Cancelled);
        assert_eq!(e.tree(&id).unwrap().legs[2].status, Status::Cancelled);

        // yeni submit bloklu
        let t2 = bracket();
        let id2 = t2.id;
        let ev2 = e.apply(Command::Submit(t2));
        assert!(ev2.iter().any(|x| matches!(x, Event::Rejected { reason, .. } if reason.contains("kill"))));
        assert!(sends(&ev2).is_empty());
        assert!(e.tree(&id2).is_none(), "bloklu submit ağaç oluşturmaz");

        // resume → tekrar submit borsaya gider
        e.apply(Command::KillSwitch {
            tenant_id: Some("t1".into()),
            account_id: Some("a1".into()),
            active: false,
        });
        let t3 = bracket();
        let ev3 = e.apply(Command::Submit(t3));
        assert_eq!(sends(&ev3).len(), 1, "resume sonrası leg0 borsaya gider");
    }
}
