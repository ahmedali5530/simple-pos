import { defineUiPlugin } from '../../platform/registry'

export const inventoryPlugin = defineUiPlugin({
  id: 'inventory',
  permissions: ['inventory.read', 'inventory.write'],
  nav: [
    {
      id: 'stock',
      label: 'Stock In / Out',
      href: '/stock',
      permission: 'inventory.write',
      order: 20,
    },
  ],
  settingsPages: [],
  reportPages: [],
  pages: [
    {
      path: '/stock',
      title: 'Stock',
      permission: 'inventory.write',
      // Split layout: each panel owns its scroll area.
      density: 'touch',
      load: () => import('./screens/StockPage.svelte'),
    },
  ],
})
