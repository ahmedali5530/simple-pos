CREATE TABLE IF NOT EXISTS gift_cards (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    code TEXT NOT NULL,
    balance INT NOT NULL CHECK (balance >= 0),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'void')),
    issued_bill_id INT,
    issued_bill_no TEXT NOT NULL DEFAULT '',
    issued_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, code)
);

CREATE INDEX IF NOT EXISTS gift_cards_status_idx ON gift_cards (tenant_id, status);

ALTER TABLE gift_cards ENABLE ROW LEVEL SECURITY;
ALTER TABLE gift_cards FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON gift_cards;
DROP POLICY IF EXISTS tenant_isolation_permissive ON gift_cards;
CREATE POLICY tenant_isolation_restrictive ON gift_cards AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON gift_cards AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE TABLE IF NOT EXISTS gift_ledger (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    card_id INT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('issue', 'redeem', 'void', 'adjust')),
    amount INT NOT NULL,
    balance_after INT NOT NULL CHECK (balance_after >= 0),
    bill_id INT,
    bill_no TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    CONSTRAINT gift_ledger_card_fk FOREIGN KEY (tenant_id, card_id)
        REFERENCES gift_cards (tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS gift_ledger_card_idx ON gift_ledger (tenant_id, card_id);

ALTER TABLE gift_ledger ENABLE ROW LEVEL SECURITY;
ALTER TABLE gift_ledger FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON gift_ledger;
DROP POLICY IF EXISTS tenant_isolation_permissive ON gift_ledger;
CREATE POLICY tenant_isolation_restrictive ON gift_ledger AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON gift_ledger AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
