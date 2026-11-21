import { defineUiPlugin } from '../../platform/registry'

export const cashPlugin = defineUiPlugin({
  id: 'cash',
  permissions: ['cash.read', 'cash.write'],
  nav: [
    {
      id: 'cash',
      label: 'Cash Closing',
      href: '/cash',
      permission: 'cash.read',
      order: 40,
    },
  ],
  settingsPages: [],
  reportPages: [],
})
