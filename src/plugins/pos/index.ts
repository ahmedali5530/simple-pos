import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const posPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    {
      path: '/billing',
      title: 'POS Billing',
      permission: 'pos.bills.post',
      density: 'touch',
      load: () => import('./screens/BillingScreen.svelte'),
    },
  ],
})
