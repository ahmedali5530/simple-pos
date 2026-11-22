import { defineUiPlugin } from '../../platform/registry'

export const adminPlugin = defineUiPlugin({
  id: 'admin',
  permissions: ['platform.admin', 'platform.plugins.manage', 'platform.tokens.manage'],
  nav: [],
  settingsPages: [
    { id: 'users', label: 'Users', href: '/settings/users', permission: 'users.write', order: 40 },
    {
      id: 'plugins',
      label: 'Plugins',
      href: '/settings/plugins',
      permission: 'platform.plugins.manage',
      order: 60,
    },
  ],
  reportPages: [],
})
