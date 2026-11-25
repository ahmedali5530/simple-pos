import { defineUiPlugin } from '../../platform/registry'

export const corePlugin = defineUiPlugin({
  id: 'core',
  permissions: ['core.read', 'settings.write'],
  nav: [],
  settingsPages: [
    { id: 'system', label: 'System', href: '/settings/system', permission: 'settings.write', order: 50 },
  ],
  reportPages: [],
  pages: [
    { path: '/settings/system', title: 'System', permission: 'settings.write', load: () => import('./screens/SystemPage.svelte') },
  ],
})
