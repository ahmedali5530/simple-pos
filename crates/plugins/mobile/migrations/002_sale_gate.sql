-- A mobile.device line points at one physical unit. Once a bill carries that
-- reference, no later bill may carry it again.
--
-- This is a trigger rather than a unique index because older bills may already
-- contain duplicate device lines; the trigger only guards newer writes and
-- leaves the recorded history untouched. It runs inside the POS transaction, so
-- a duplicate sale rolls the whole bill back before inventory or warranty
-- handlers see it.
CREATE OR REPLACE FUNCTION mobile_device_sold_once() RETURNS trigger AS $$
DECLARE
    existing_bill TEXT;
BEGIN
    IF NEW.source = 'mobile.device' AND NEW.reference <> '' THEN
        SELECT b.bill_no INTO existing_bill
        FROM bill_lines bl
        JOIN bills b ON b.tenant_id = bl.tenant_id AND b.id = bl.bill_id
        WHERE bl.tenant_id = NEW.tenant_id
          AND bl.source = NEW.source
          AND bl.reference = NEW.reference
        ORDER BY bl.id
        LIMIT 1;

        IF existing_bill IS NOT NULL THEN
            RAISE EXCEPTION 'That phone is already on bill %.', existing_bill
                USING ERRCODE = 'P0001';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS mobile_device_sold_once ON bill_lines;
CREATE TRIGGER mobile_device_sold_once
BEFORE INSERT ON bill_lines
FOR EACH ROW EXECUTE FUNCTION mobile_device_sold_once();
