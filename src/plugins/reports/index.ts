import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const reportsPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    { path: '/reports/sales', title: 'Sales', permission: 'reports.read', load: () => import('./screens/SalesPage.svelte') },
    { path: '/reports/ledger', title: 'Stock ledger', permission: 'reports.read', load: () => import('./screens/LedgerPage.svelte') },
    { path: '/reports/mix', title: 'Product mix', permission: 'reports.read', load: () => import('./screens/MixPage.svelte') },
  ],
})
