-- Keep the operator name on the shift so cash history never needs a users join.
ALTER TABLE cash_shifts
    ADD COLUMN IF NOT EXISTS opened_by_name TEXT NOT NULL DEFAULT '';

ALTER TABLE cash_shifts
    ADD COLUMN IF NOT EXISTS closed_by_name TEXT;

UPDATE cash_shifts c
SET opened_by_name = u.name
FROM users u
WHERE c.opened_by = u.id
  AND c.opened_by_name = '';

UPDATE cash_shifts c
SET closed_by_name = u.name
FROM users u
WHERE c.closed_by = u.id
  AND c.closed_by_name IS NULL;
