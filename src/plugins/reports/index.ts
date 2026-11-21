import { defineUiPlugin } from '../../platform/registry'

export const reportsPlugin = defineUiPlugin({
  id: 'reports',
  permissions: ['reports.read'],
  nav: [
    {
      id: 'reports',
      label: 'Reports & Summary',
      href: '/reports/sales',
      permission: 'reports.read',
      order: 30,
    },
  ],
  settingsPages: [],
  reportPages: [
    { id: 'sales', label: 'Sales', href: '/reports/sales', permission: 'reports.read', order: 10 },
    { id: 'ledger', label: 'Stock ledger', href: '/reports/ledger', permission: 'reports.read', order: 20 },
    { id: 'mix', label: 'Product mix', href: '/reports/mix', permission: 'reports.read', order: 30 },
  ],
})
