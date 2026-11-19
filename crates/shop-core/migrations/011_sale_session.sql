ALTER TABLE bills
    ADD COLUMN user_id INT REFERENCES users (id);

CREATE INDEX bills_user_idx ON bills (user_id);

CREATE TABLE sessions (
    token TEXT PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE shop_settings
    ADD COLUMN print_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN printer_kind TEXT NOT NULL DEFAULT 'network',
    ADD COLUMN printer_target TEXT NOT NULL DEFAULT '',
    ADD COLUMN printer_port INT NOT NULL DEFAULT 9100;
