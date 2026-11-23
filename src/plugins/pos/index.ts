import { defineUiPlugin } from '../../platform/registry'

export const posPlugin = defineUiPlugin({
  id: 'pos',
  permissions: ['pos.read', 'pos.bills.post'],
  nav: [
    {
      id: 'billing',
      label: 'POS Billing',
      href: '/billing',
      permission: 'pos.bills.post',
      order: 10,
    },
  ],
  settingsPages: [],
  reportPages: [],
  slotsProvided: ['pos.line-source', 'pos.line-detail', 'pos.item-options'],
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
