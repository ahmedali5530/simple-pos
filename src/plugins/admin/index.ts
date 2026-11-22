import { defineUiPlugin } from '../../platform/registry'

export const adminPlugin = defineUiPlugin({
  id: 'admin',
  permissions: ['platform.admin', 'platform.plugins.manage', 'platform.tokens.manage'],
  nav: [],
  settingsPages: [
    { id: 'users', label: 'Users', href: '/settings/users', permission: 'users.write', order: 40 },
    { id: 'system', label: 'System', href: '/settings/system', permission: 'settings.write', order: 50 },
    {
      id: 'plugins',
      label: 'Plugins',
      href: '/settings/plugins',
      permission: 'platform.plugins.manage',
      order: 60,
    },
  ],
  reportPages: [],
  pages: [
    { path: '/settings/users', title: 'Users', permission: 'users.write', load: () => import('./screens/UsersPage.svelte') },
    { path: '/settings/system', title: 'System', permission: 'settings.write', load: () => import('./screens/SystemPage.svelte') },
    {
      path: '/settings/plugins',
      title: 'Plugins',
      permission: 'platform.plugins.manage',
      load: () => import('./screens/PluginsPage.svelte'),
    },
  ],
})
