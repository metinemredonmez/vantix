//! exec-core binary: NATS ile dış dünyaya bağlanır, Engine'i tek task içinde sıralı çalıştırır.
//!
//! Kalıcılık & replay:
//! - Giriş komutları (exec.order.* / exec.broker.{ack,reject,fill,cancelled} / md.tick.*) JetStream
//!   `EXEC_IN` stream'inde durable tutulur (publisher core publish yapsa bile stream yakalar).
//! - Boot'ta TEK ephemeral consumer (DeliverPolicy::All) stream'i baştan verir: `seq <= boot_last_seq`
//!   olan mesajlar REPLAY'dir → engine state'i yeniden kurulur, outbound BASTIRILIR. Sonraki mesajlar
//!   CANLI'dır → outbound yayınlanır + append-only sink'e (order_event) yazılır.
//! - Böylece process restart'ta state deterministik biçimde yeniden kurulur (saf motor + aynı komut sırası).
//!
//! Çıkış  : exec.order.state | exec.broker.send | exec.broker.cancel | exec.risk.violation | exec.order.rejected
//! NOT: JetStream gereklidir (nats-server -js).

use exec_core::persist::{sink_from_env, EventSink};
use exec_core::{engine::CompositeRisk, BrokerAck, BrokerCancelled, BrokerReject, CancelRequest, Command, Engine, Event, KillRequest, ModifyRequest, ReconcileRequest};
use futures::StreamExt;
use rust_decimal_macros::dec;
use std::time::Duration;

const STREAM: &str = "EXEC_IN";
const SUBJECTS_IN: &[&str] = &[
    "exec.order.submit",
    "exec.order.cancel",
    "exec.order.modify",
    "exec.control.kill",
    "exec.control.reconcile",
    "exec.broker.ack",
    "exec.broker.reject",
    "exec.broker.fill",
    "exec.broker.cancelled",
    "md.tick.>",
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());
    let nc = async_nats::connect(&url).await?;
    tracing::info!(%url, "exec-core connected");

    // Durable giriş log'u — replay kaynağı.
    let js = async_nats::jetstream::new(nc.clone());
    let mut stream = js
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: STREAM.to_string(),
            subjects: SUBJECTS_IN.iter().map(|s| s.to_string()).collect(),
            max_age: Duration::from_secs(60 * 60 * 24 * 7), // 7 gün; snapshot gelince kısalır
            ..Default::default()
        })
        .await?;
    let boot_last_seq = stream.info().await?.state.last_sequence;
    tracing::info!(boot_last_seq, "JetStream EXEC_IN hazır");

    let mut engine = Engine::new(Box::new(CompositeRisk {
        max_qty: Some(dec!(1_000_000)),
        max_notional: Some(dec!(50_000_000)),
        max_price_deviation_pct: Some(dec!(20)),
    }));
    let mut sink: Box<dyn EventSink> = sink_from_env();

    // Boot snapshot: varsa durumu yükle; JetStream yalnız snapshot_seq'ten SONRAKİni replay eder.
    let snap_path = std::env::var("EXEC_SNAPSHOT").ok().filter(|s| !s.is_empty());
    let mut snapshot_seq = 0u64;
    if let Some(p) = &snap_path {
        if let Some(snap) = exec_core::persist::load_snapshot(p) {
            snapshot_seq = snap.seq;
            engine.restore(snap);
            tracing::info!(snapshot_seq, "snapshot yüklendi");
        }
    }
    let snapshot_every = std::env::var("EXEC_SNAPSHOT_EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(20u64);
    let mut since_snapshot = 0u64;

    // Tek ephemeral consumer: önce backlog (replay), sonra canlı akış — sıralı, seam'siz.
    let consumer = stream
        .create_consumer(async_nats::jetstream::consumer::pull::Config {
            deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
            ack_policy: async_nats::jetstream::consumer::AckPolicy::None,
            inactive_threshold: Duration::from_secs(600),
            ..Default::default()
        })
        .await?;
    let mut messages = consumer.messages().await?;

    let mut replayed = 0u64;
    let mut live = boot_last_seq == 0;
    if live {
        tracing::info!("replay yok (boş stream) → canlı");
    }

    while let Some(next) = messages.next().await {
        let msg = match next {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("consumer error: {e}");
                continue;
            }
        };
        let seq = msg.info().map(|i| i.stream_sequence).unwrap_or(0);
        // snapshot'a dahil olanları atla (zaten state'te). Kalanlar: replay veya canlı.
        if seq != 0 && seq <= snapshot_seq {
            continue;
        }
        let is_replay = boot_last_seq != 0 && seq <= boot_last_seq;

        let cmd = match parse(msg.subject.as_str(), msg.payload.as_ref()) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(subject = %msg.subject, "bad payload: {e}");
                continue;
            }
        };
        let events = engine.apply(cmd);

        if is_replay {
            replayed += 1;
            continue; // state yeniden kuruluyor; outbound YOK, sink YOK (kayıt zaten mevcut)
        }
        if !live {
            live = true;
            tracing::info!(replayed, snapshot_seq, "replay tamam → canlı");
        }
        for ev in &events {
            sink.append(ev);
            let (subject, payload) = route(ev);
            if let Err(e) = nc.publish(subject, payload.into()).await {
                tracing::error!("publish failed: {e}");
            }
        }
        // Periyodik snapshot (canlı komut sayısına göre).
        if let Some(p) = &snap_path {
            since_snapshot += 1;
            if since_snapshot >= snapshot_every {
                since_snapshot = 0;
                if let Err(e) = exec_core::persist::save_snapshot(p, &engine.export_snapshot(seq)) {
                    tracing::warn!("snapshot yazılamadı: {e}");
                }
            }
        }
    }
    Ok(())
}

fn parse(subject: &str, payload: &[u8]) -> anyhow::Result<Command> {
    Ok(match subject {
        "exec.order.submit" => Command::Submit(serde_json::from_slice(payload)?),
        "exec.order.cancel" => {
            let c: CancelRequest = serde_json::from_slice(payload)?;
            Command::Cancel { order_id: c.order_id, leg: c.leg }
        }
        "exec.order.modify" => {
            let m: ModifyRequest = serde_json::from_slice(payload)?;
            Command::Modify { order_id: m.order_id, leg: m.leg, price: m.price, qty: m.qty }
        }
        "exec.control.kill" => {
            let k: KillRequest = serde_json::from_slice(payload)?;
            Command::KillSwitch { tenant_id: k.tenant_id, account_id: k.account_id, active: k.active }
        }
        "exec.control.reconcile" => {
            let r: ReconcileRequest = serde_json::from_slice(payload)?;
            Command::Reconcile { open_broker_ids: r.open_broker_ids }
        }
        "exec.broker.ack" => {
            let a: BrokerAck = serde_json::from_slice(payload)?;
            Command::BrokerAck { ems_order_id: a.ems_order_id, broker_order_id: a.broker_order_id }
        }
        "exec.broker.reject" => {
            let r: BrokerReject = serde_json::from_slice(payload)?;
            Command::BrokerReject { ems_order_id: r.ems_order_id, reason: r.reason }
        }
        "exec.broker.fill" => Command::BrokerFill(serde_json::from_slice(payload)?),
        "exec.broker.cancelled" => {
            let x: BrokerCancelled = serde_json::from_slice(payload)?;
            Command::BrokerCancelled { ems_order_id: x.ems_order_id }
        }
        s if s.starts_with("md.tick.") => Command::Tick(serde_json::from_slice(payload)?),
        s => anyhow::bail!("unknown subject {s}"),
    })
}

fn route(ev: &Event) -> (&'static str, Vec<u8>) {
    let subject = match ev {
        Event::StateChanged { .. } => "exec.order.state",
        Event::SendToBroker(_) => "exec.broker.send",
        Event::CancelAtBroker { .. } => "exec.broker.cancel",
        Event::ModifyAtBroker { .. } => "exec.broker.modify",
        Event::RiskViolation { .. } => "exec.risk.violation",
        Event::Rejected { .. } => "exec.order.rejected",
        Event::ReconcileMismatch { .. } => "exec.reconcile.mismatch",
    };
    (subject, serde_json::to_vec(ev).expect("serialize event"))
}
