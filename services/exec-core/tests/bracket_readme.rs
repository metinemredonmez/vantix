//! Integration test: README'deki bracket örneğinin ("Örnek: bracket emri") uçtan uca akışı.
//!
//! README JSON'u aynen `exec.order.submit` payload'ı gibi `OrderTree`'ye deserialize edilir
//! (bkz. main.rs `parse()` → `Command::Submit(serde_json::from_slice(payload)?)`), sonra
//! README'de tarif edilen akış adım adım doğrulanır:
//!
//!   leg0 → exec.broker.send; fill gelince leg1 borsaya, leg2 tetik bekler;
//!   md.tick.GARAN last ≤ 121 → leg2 MARKET gider; dolunca leg1 için exec.broker.cancel.
//!
//! Bu test lib'in yalnızca public API'sini kullanır (ayrı crate).

use exec_core::{Command, Engine, Event};
use exec_core::{Fill, NativeOrder, NativeType, OrderTree, Side, Tick};
use rust_decimal_macros::dec;

/// README "Örnek: bracket emri (exec.order.submit)" bloğundaki JSON, birebir.
const BRACKET_JSON: &str = r#"{
  "tenant_id": "demo", "account_id": "acc1", "broker_id": "mock",
  "legs": [
    { "symbol": "GARAN", "side": "BUY",  "qty": "1000", "price": "125" },
    { "symbol": "GARAN", "side": "SELL", "qty": "1000", "price": "130" },
    { "symbol": "GARAN", "side": "SELL", "qty": "1000", "native_type": "MARKET", "trigger_price": "121" }
  ],
  "tree": [ { "parent": 0, "child": 1, "on": "FILLED" }, { "parent": 0, "child": 2, "on": "FILLED" } ],
  "oco_groups": [[1, 2]]
}"#;

fn sends(ev: &[Event]) -> Vec<&NativeOrder> {
    ev.iter()
        .filter_map(|e| if let Event::SendToBroker(o) = e { Some(o) } else { None })
        .collect()
}

#[test]
fn readme_bracket_end_to_end() {
    // README payload'ı exec.order.submit'in yaptığı gibi doğrudan OrderTree'ye çözülür.
    let tree: OrderTree = serde_json::from_str(BRACKET_JSON).expect("README bracket JSON parse");
    let id = tree.id; // id JSON'da yok → serde default Uuid::new_v4 üretir
    assert_eq!(tree.legs.len(), 3);
    assert_eq!(tree.tenant_id, "demo");

    let mut e = Engine::default();

    // 1) Submit: yalnızca root leg0 borsaya gider; leg1 ve leg2 parent'ı bekler.
    let ev = e.apply(Command::Submit(tree));
    let s = sends(&ev);
    assert_eq!(s.len(), 1, "submit'te sadece leg0 (root) borsaya gider");
    let entry = s[0];
    assert_eq!(entry.ems_order_id, format!("{id}:0"));
    assert_eq!(entry.symbol, "GARAN");
    assert_eq!(entry.side, Side::Buy);
    assert_eq!(entry.qty, dec!(1000));
    assert_eq!(entry.price, Some(dec!(125)));
    assert_eq!(entry.native_type, NativeType::Limit);
    {
        let ts = e.tree(&id).unwrap();
        assert_eq!(ts.legs[1].status, exec_core::Status::PendingTrigger);
        assert_eq!(ts.legs[2].status, exec_core::Status::PendingTrigger);
    }

    // 2) leg0 borsada ack + tam fill → child'lar aktive olur.
    e.apply(Command::BrokerAck {
        ems_order_id: format!("{id}:0"),
        broker_order_id: "B1".into(),
    });
    let ev = e.apply(Command::BrokerFill(Fill {
        ems_order_id: format!("{id}:0"),
        broker_order_id: "B1".into(),
        qty: dec!(1000),
        price: dec!(125),
        remaining: dec!(0),
        ts_ms: 1,
    }));
    // Fill gelince: leg1 (TP LIMIT) borsaya gider, leg2 (SL) tetik bekler.
    let s = sends(&ev);
    assert_eq!(s.len(), 1, "fill sonrası sadece TP (leg1) borsaya gider");
    assert_eq!(s[0].ems_order_id, format!("{id}:1"));
    assert_eq!(s[0].side, Side::Sell);
    assert_eq!(s[0].price, Some(dec!(130)));
    assert_eq!(s[0].native_type, NativeType::Limit);
    {
        let ts = e.tree(&id).unwrap();
        assert_eq!(ts.legs[0].status, exec_core::Status::Filled);
        assert_eq!(ts.legs[1].status, exec_core::Status::Sending);
        assert_eq!(ts.legs[2].status, exec_core::Status::PendingTrigger);
        assert!(ts.legs[2].rule.is_some(), "leg2 stop kuralı armed olmalı");
    }

    // 3) TP borsada ack (Working).
    e.apply(Command::BrokerAck {
        ems_order_id: format!("{id}:1"),
        broker_order_id: "B2".into(),
    });

    // 4) md.tick.GARAN last ≤ 121 → leg2 stop tetiklenir ve MARKET olarak gider.
    let ev = e.apply(Command::Tick(Tick {
        symbol: "GARAN".into(),
        last: dec!(121),
        bid: None,
        ask: None,
        ts_ms: 2,
    }));
    let s = sends(&ev);
    assert_eq!(s.len(), 1, "tick 121 → sadece SL (leg2) borsaya gider");
    assert_eq!(s[0].ems_order_id, format!("{id}:2"));
    assert_eq!(s[0].native_type, NativeType::Market, "stop MARKET olarak gider");
    assert_eq!(s[0].price, None);

    // 5) SL borsada ack + tam fill → OCO kardeş TP (B2) borsada iptal edilir.
    e.apply(Command::BrokerAck {
        ems_order_id: format!("{id}:2"),
        broker_order_id: "B3".into(),
    });
    let ev = e.apply(Command::BrokerFill(Fill {
        ems_order_id: format!("{id}:2"),
        broker_order_id: "B3".into(),
        qty: dec!(1000),
        price: dec!(121),
        remaining: dec!(0),
        ts_ms: 3,
    }));
    assert!(
        ev.iter().any(|x| matches!(
            x,
            Event::CancelAtBroker { broker_order_id, .. } if broker_order_id == "B2"
        )),
        "SL dolunca OCO kardeş TP için exec.broker.cancel yayılmalı"
    );
    assert_eq!(
        e.tree(&id).unwrap().legs[1].status,
        exec_core::Status::Cancelling
    );

    // 6) Borsa iptali onaylayınca TP CANCELLED olur; SL FILLED kalır.
    e.apply(Command::BrokerCancelled {
        ems_order_id: format!("{id}:1"),
    });
    let ts = e.tree(&id).unwrap();
    assert_eq!(ts.legs[1].status, exec_core::Status::Cancelled);
    assert_eq!(ts.legs[2].status, exec_core::Status::Filled);
}
