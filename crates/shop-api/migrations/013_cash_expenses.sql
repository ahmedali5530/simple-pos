CREATE TABLE cash_expenses (
    id SERIAL PRIMARY KEY,
    shift_id INT NOT NULL REFERENCES cash_shifts (id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    category TEXT NOT NULL,
    amount INT NOT NULL CHECK (amount >= 0),
    sort_order INT NOT NULL DEFAULT 0
);

CREATE INDEX cash_expenses_shift_idx ON cash_expenses (shift_id, sort_order, id);
