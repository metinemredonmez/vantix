//! Append-only olay kalıcılığı (order_event) — audit + read-model beslemesi.
//!
//! `EventSink` motorun ürettiği her Event'i monoton `seq` ile ekler. v0'da dosya (JSONL)
//! implementasyonu var; Postgres/TimescaleDB `PostgresSink` aynı trait'i doldurur
//! (şema: infra/sql/0001_order_event.sql). JetStream, giriş komutlarının durable log'u +
//! boot replay'i içindir (bkz. main.rs); bu sink ise ÇIKIŞ olaylarının audit kaydıdır.

use crate::events::Event;
use serde_json::json;
use std::io::Write;

pub trait EventSink: Send {
    /// Bir çıkış olayını append-only kaydet. Idempotent değildir; yalnız CANLI olaylar için çağrılır
    /// (replay sırasında çağrılmaz — kayıt zaten mevcut).
    fn append(&mut self, event: &Event);
}

/// Kalıcılık kapalıyken (env yoksa) no-op.
pub struct NullSink;
impl EventSink for NullSink {
    fn append(&mut self, _: &Event) {}
}

/// Append-only JSONL dosyası. Her satır: {seq, ts_ms, kind, order_id, event}.
/// `seq` dosyadaki mevcut satır sayısından devam eder (restart'ta monoton kalır).
pub struct FileSink {
    file: std::fs::File,
    seq: u64,
}

impl FileSink {
    pub fn open(path: &str) -> std::io::Result<Self> {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        let seq = existing.lines().filter(|l| !l.trim().is_empty()).count() as u64;
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file, seq })
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl EventSink for FileSink {
    fn append(&mut self, event: &Event) {
        self.seq += 1;
        let v = serde_json::to_value(event).unwrap_or(serde_json::Value::Null);
        let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
        let order_id = v.get("order_id").cloned().unwrap_or(serde_json::Value::Null);
        let rec = json!({ "seq": self.seq, "ts_ms": now_ms(), "kind": kind, "order_id": order_id, "event": v });
        // Tek satır JSON; hata durumunda düşür (audit best-effort, motoru durdurma).
        if let Ok(line) = serde_json::to_string(&rec) {
            let _ = writeln!(self.file, "{line}");
            let _ = self.file.flush();
        }
    }
}

// ---------------- boot snapshot IO ----------------
use crate::engine::Snapshot;

/// Snapshot'ı atomik yaz (temp + rename) — yarım dosya bırakmamak için.
pub fn save_snapshot(path: &str, snap: &Snapshot) -> std::io::Result<()> {
    let tmp = format!("{path}.tmp");
    let data = serde_json::to_vec(snap).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, &data)?;
    std::fs::rename(&tmp, path)
}

/// Snapshot yükle; dosya yoksa/bozuksa None.
pub fn load_snapshot(path: &str) -> Option<Snapshot> {
    let data = std::fs::read(path).ok()?;
    match serde_json::from_slice::<Snapshot>(&data) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!("snapshot okunamadı ({e}); yok sayılıyor");
            None
        }
    }
}

/// Env'e göre sink seç: EXEC_EVENT_LOG=<path> → FileSink, yoksa NullSink.
pub fn sink_from_env() -> Box<dyn EventSink> {
    match std::env::var("EXEC_EVENT_LOG") {
        Ok(path) if !path.is_empty() => match FileSink::open(&path) {
            Ok(s) => {
                tracing::info!(%path, start_seq = s.seq(), "order_event FileSink açıldı");
                Box::new(s)
            }
            Err(e) => {
                tracing::warn!("FileSink açılamadı ({e}); NullSink kullanılıyor");
                Box::new(NullSink)
            }
        },
        _ => Box::new(NullSink),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_machine::Status;
    use uuid::Uuid;

    #[test]
    fn filesink_appends_and_seq_persists() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("exec_event_{}.jsonl", Uuid::new_v4()));
        let p = path.to_str().unwrap();

        let ev = Event::StateChanged {
            order_id: Uuid::nil(),
            leg: 0,
            from: Status::Working,
            to: Status::Filled,
            reason: None,
        };
        {
            let mut s = FileSink::open(p).unwrap();
            assert_eq!(s.seq(), 0);
            s.append(&ev);
            s.append(&ev);
            assert_eq!(s.seq(), 2);
        }
        // yeniden aç: seq mevcut satırlardan devam eder
        let s2 = FileSink::open(p).unwrap();
        assert_eq!(s2.seq(), 2);

        let content = std::fs::read_to_string(p).unwrap();
        assert_eq!(content.lines().count(), 2);
        let first: serde_json::Value = serde_json::from_str(content.lines().next().unwrap()).unwrap();
        assert_eq!(first["seq"], 1);
        assert_eq!(first["kind"], "state_changed");
        std::fs::remove_file(p).ok();
    }
}
