//! Conformance: packages/contracts/golden/*.json örnekleri exec-core'un GERÇEK serde
//! tipleriyle birebir uyuşuyor mu? Bu test, elle yazılan JSON Schema (tek kaynak) ile
//! Rust struct'ları arasındaki drift'i yakalar. Aynı golden dosyalar zod ve pydantic
//! tarafında da doğrulanır → üç dil aynı payload üzerinde anlaşır.

use exec_core::events::Event;
use exec_core::{BrokerAck, BrokerCancelled, BrokerReject, CancelRequest, Fill, KillRequest, ModifyRequest, OrderTree, ReconcileRequest, Tick};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::path::PathBuf;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts/golden")
}

fn read(rel: &str) -> String {
    let p = golden_dir().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("golden okunamadı {p:?}: {e}"))
}

/// `sub`'daki her alan `sup`'ta aynı değerle var mı (golden'da yazan hiçbir alan
/// kaybolmuyor / yeniden adlandırılmıyor / tip değiştirmiyor).
fn assert_subset(sub: &Value, sup: &Value, path: &str) {
    match (sub, sup) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, va) in a {
                let vb = b
                    .get(k)
                    .unwrap_or_else(|| panic!("{path}: '{k}' alanı serde çıktısında yok (rename/eksik?)"));
                assert_subset(va, vb, &format!("{path}.{k}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: dizi uzunluğu farklı");
            for (i, (va, vb)) in a.iter().zip(b).enumerate() {
                assert_subset(va, vb, &format!("{path}[{i}]"));
            }
        }
        (x, y) => assert_eq!(x, y, "{path}: değer uyuşmuyor"),
    }
}

/// Golden parse edilir; serde ile round-trip STABİL; golden alanları korunur.
fn check<T: DeserializeOwned + Serialize>(rel: &str) {
    let raw = read(rel);
    let orig: Value = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{rel}: JSON değil: {e}"));
    let parsed: T = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{rel}: {} tipine parse edilemedi: {e}", std::any::type_name::<T>()));
    let ser = serde_json::to_value(&parsed).unwrap();
    // round-trip stabil mi?
    let reparsed: T = serde_json::from_value(ser.clone()).unwrap();
    assert_eq!(serde_json::to_value(&reparsed).unwrap(), ser, "{rel}: round-trip kararsız");
    // golden'da yazan alanlar korunuyor mu?
    assert_subset(&orig, &ser, rel);
}

#[test]
fn inbound_payloads_match_serde() {
    check::<OrderTree>("in/order_tree.json");
    check::<CancelRequest>("in/cancel_request.json");
    check::<ModifyRequest>("in/modify.json");
    check::<KillRequest>("in/kill.json");
    check::<ReconcileRequest>("in/reconcile.json");
    check::<BrokerAck>("in/broker_ack.json");
    check::<BrokerReject>("in/broker_reject.json");
    check::<BrokerCancelled>("in/broker_cancelled.json");
    check::<Fill>("in/fill.json");
    check::<Tick>("in/tick.json");
}

#[test]
fn outbound_events_match_serde() {
    // Hepsi internally-tagged Event; discriminator: type
    for f in [
        "out/state_changed.json",
        "out/send_to_broker.json",
        "out/cancel_at_broker.json",
        "out/modify_at_broker.json",
        "out/risk_violation.json",
        "out/rejected.json",
        "out/reconcile_mismatch.json",
    ] {
        check::<Event>(f);
    }
}
