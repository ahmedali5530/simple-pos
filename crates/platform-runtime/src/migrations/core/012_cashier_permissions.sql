-- Replace the original all-permissions cashier grant with least-privilege till access.
DELETE FROM role_permissions
WHERE role_id = 2
  AND tenant_id = '00000000-0000-4000-8000-000000000001';

INSERT INTO role_permissions (tenant_id, role_id, permission)
SELECT '00000000-0000-4000-8000-000000000001', 2, name
FROM platform_permissions
WHERE name IN ('catalog.read', 'inventory.read', 'pos.read', 'pos.bills.post', 'cash.read')
ON CONFLICT DO NOTHING;
