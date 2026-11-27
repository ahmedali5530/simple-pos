-- Matrix SKU sales own their stock. The trigger runs inside the POS bill
-- transaction so an oversell rolls the whole bill back before commit.
CREATE OR REPLACE FUNCTION matrix_sku_apply_sale() RETURNS trigger AS $$
DECLARE
    sku_variant INT;
    on_hand INT;
    cost BIGINT;
    parent_item INT;
BEGIN
    IF NEW.source = 'matrix.sku' AND NEW.reference LIKE 'sku:%' THEN
        BEGIN
            sku_variant := substring(NEW.reference from 5)::INT;
        EXCEPTION WHEN others THEN
            RAISE EXCEPTION 'That matrix SKU reference is invalid.'
                USING ERRCODE = 'P0001';
        END;

        SELECT s.stock, s.avg_cost, s.item_id
          INTO on_hand, cost, parent_item
          FROM matrix_skus s
         WHERE s.variant_id = sku_variant
         FOR UPDATE;

        IF NOT FOUND THEN
            RAISE EXCEPTION 'That variant is not a matrix SKU.'
                USING ERRCODE = 'P0001';
        END IF;

        IF NEW.item_id IS NOT NULL AND NEW.item_id <> parent_item THEN
            RAISE EXCEPTION 'That matrix SKU does not belong to this item.'
                USING ERRCODE = 'P0001';
        END IF;

        IF on_hand < NEW.qty THEN
            RAISE EXCEPTION 'Only % left for that variant.', on_hand
                USING ERRCODE = 'P0001';
        END IF;

        UPDATE matrix_skus
           SET stock = stock - NEW.qty,
               updated_at = now()
         WHERE variant_id = sku_variant;

        INSERT INTO matrix_sku_movements (variant_id, item_id, qty, reason)
        VALUES (
            sku_variant,
            parent_item,
            -NEW.qty,
            format('POS bill · %s', coalesce(NEW.name, 'sale'))
        );

        NEW.unit_cost := cost;
        NEW.cost_total := cost * NEW.qty::bigint;
        IF NEW.variant_id IS NULL THEN
            NEW.variant_id := sku_variant;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS matrix_sku_apply_sale ON bill_lines;
CREATE TRIGGER matrix_sku_apply_sale
BEFORE INSERT ON bill_lines
FOR EACH ROW EXECUTE FUNCTION matrix_sku_apply_sale();
