CREATE TABLE pos_state (
    id INT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_section_id INT REFERENCES sections (id) ON DELETE SET NULL
);

INSERT INTO pos_state (id, last_section_id)
SELECT 1, id
FROM sections
ORDER BY pos_order, id
LIMIT 1;
