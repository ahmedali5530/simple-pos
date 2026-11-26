import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const cashPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    { path: '/cash', title: 'Cash closing', permission: 'cash.read', load: () => import('./screens/CashPage.svelte') },
  ],
})
