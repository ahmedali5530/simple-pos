CREATE TABLE cash_shifts (
    id SERIAL PRIMARY KEY,
    opened_by INT NOT NULL REFERENCES users (id),
    closed_by INT REFERENCES users (id),
    opened_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at TIMESTAMPTZ,
    opening_cash INT NOT NULL CHECK (opening_cash >= 0),
    petty_cash INT NOT NULL CHECK (petty_cash >= 0),
    sales BIGINT NOT NULL DEFAULT 0,
    expenses INT,
    cash_taken INT,
    manual_count INT,
    system_count BIGINT,
    difference BIGINT,
    cash_left INT,
    CHECK (closed_at IS NULL OR closed_at >= opened_at)
);

CREATE UNIQUE INDEX cash_shifts_open_idx ON cash_shifts ((1)) WHERE closed_at IS NULL;
