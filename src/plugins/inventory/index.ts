import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const inventoryPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    {
      path: '/stock',
      title: 'Stock',
      permission: 'inventory.write',
      density: 'touch',
      load: () => import('./screens/StockPage.svelte'),
    },
  ],
})
