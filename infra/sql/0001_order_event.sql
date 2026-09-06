-- exec-core append-only order_event (audit + read-model beslemesi).
-- v0'da exec-core FileSink (JSONL) yazar; PostgresSink bu tabloyu doldurur (aynı EventSink trait'i).
-- TimescaleDB hypertable: zaman serisi audit için.

CREATE TABLE IF NOT EXISTS order_event (
  seq         BIGSERIAL,
  ts          TIMESTAMPTZ  NOT NULL DEFAULT now(),
  tenant_id   TEXT,                       -- event payload'ından türetilir (varsa)
  order_id    UUID,                       -- StateChanged/RiskViolation/Rejected'da dolu
  leg         INT,                        -- ilgili leg (varsa)
  kind        TEXT         NOT NULL,      -- state_changed | send_to_broker | cancel_at_broker | risk_violation | rejected
  event       JSONB        NOT NULL,      -- ham Event (tek kaynak: contracts Event)
  PRIMARY KEY (seq, ts)
);

-- Timescale varsa hypertable'a çevir (yoksa bu satır atlanabilir).
SELECT create_hypertable('order_event', 'ts', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS order_event_order_id_idx ON order_event (order_id, ts DESC);
CREATE INDEX IF NOT EXISTS order_event_kind_idx     ON order_event (kind, ts DESC);

-- NOT: append-only — UPDATE/DELETE yapılmaz. State replay JetStream EXEC_IN'den; bu tablo audit/sorgu içindir.
