CREATE TABLE sections (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    short_name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('retail', 'food')),
    accent TEXT NOT NULL,
    show_on_pos BOOLEAN NOT NULL DEFAULT TRUE,
    pos_order INT NOT NULL DEFAULT 0
);

CREATE TABLE categories (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INT NOT NULL DEFAULT 0
);

CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    section_id INT NOT NULL REFERENCES sections (id),
    category_id INT NOT NULL REFERENCES categories (id),
    barcode TEXT UNIQUE,
    name TEXT NOT NULL,
    rate_rupees INT NOT NULL CHECK (rate_rupees > 0),
    stock INT NOT NULL DEFAULT 0 CHECK (stock >= 0),
    portion TEXT NOT NULL DEFAULT 'regular' CHECK (portion IN ('regular', 'full', 'half')),
    group_name TEXT,
    signature BOOLEAN NOT NULL DEFAULT FALSE,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    sort_order INT NOT NULL DEFAULT 0
);

CREATE INDEX items_section_idx ON items (section_id, sort_order);

CREATE TABLE bills (
    id SERIAL PRIMARY KEY,
    bill_no TEXT NOT NULL UNIQUE,
    section_id INT NOT NULL REFERENCES sections (id),
    total_rupees INT NOT NULL CHECK (total_rupees > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE bill_lines (
    id SERIAL PRIMARY KEY,
    bill_id INT NOT NULL REFERENCES bills (id) ON DELETE CASCADE,
    item_id INT NOT NULL REFERENCES items (id),
    name TEXT NOT NULL,
    rate_rupees INT NOT NULL CHECK (rate_rupees > 0),
    qty INT NOT NULL CHECK (qty > 0),
    amount_rupees INT NOT NULL CHECK (amount_rupees > 0)
);

CREATE TABLE stock_movements (
    id SERIAL PRIMARY KEY,
    item_id INT NOT NULL REFERENCES items (id),
    qty INT NOT NULL,
    reason TEXT NOT NULL,
    bill_id INT REFERENCES bills (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
