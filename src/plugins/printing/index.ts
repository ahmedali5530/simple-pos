import { defineUiPlugin } from '../../platform/registry'

export const printingPlugin = defineUiPlugin({
  id: 'printing',
  permissions: ['printing.use'],
  nav: [],
  settingsPages: [
    { id: 'system', label: 'System', href: '/settings/system', permission: 'settings.write', order: 50 },
  ],
  reportPages: [],
})
