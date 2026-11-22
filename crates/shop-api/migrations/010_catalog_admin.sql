CREATE UNIQUE INDEX sections_short_name_lower_idx ON sections (lower(short_name));
CREATE UNIQUE INDEX categories_name_lower_idx ON categories (lower(name));

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('cashier', 'manager')),
    active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE UNIQUE INDEX users_name_lower_idx ON users (lower(name));

CREATE TABLE shop_settings (
    id INT PRIMARY KEY CHECK (id = 1),
    name TEXT NOT NULL,
    currency TEXT NOT NULL,
    locale TEXT NOT NULL
);

INSERT INTO shop_settings (id, name, currency, locale)
VALUES (1, 'Chaska Corner', 'PKR', 'en-PK');

CREATE TABLE barcode_rules (
    id SERIAL PRIMARY KEY,
    prefix TEXT NOT NULL UNIQUE,
    mode TEXT NOT NULL CHECK (mode IN ('weight', 'price')),
    scale INT NOT NULL CHECK (scale >= 1),
    sort_order INT NOT NULL DEFAULT 0
);

INSERT INTO barcode_rules (prefix, mode, scale, sort_order)
VALUES
    ('21', 'weight', 1000, 0),
    ('22', 'price', 1, 1);
