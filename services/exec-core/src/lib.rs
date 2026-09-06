//! Vantix Execution Core
//!
//! Tek deterministik emir motoru. Hem Fund (TEFAS) hem Trade (BIST/VİOP) modülünün
//! her emri, her tetiği ve her tick'i buradan geçer.
//!
//! Tasarım: `Engine` saf bir durum makinesidir — komut alır, olay üretir, I/O yapmaz.
//! NATS bağlantısı ve gateway çağrıları `main.rs`'de dıştan sarılır. Böylece motor
//! tamamen test edilebilir ve JetStream replay ile yeniden kurulabilir.

pub mod engine;
pub mod events;
pub mod order;
pub mod persist;
pub mod state_machine;
pub mod trigger;

pub use engine::Engine;
pub use events::{Command, Event};
pub use order::*;
pub use state_machine::Status;
