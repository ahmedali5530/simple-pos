import { defineUiPlugin } from '../../platform/registry'

export const customersPlugin = defineUiPlugin({
  id: 'customers',
  permissions: ['customers.read', 'customers.write'],
  nav: [
    { id: 'customers', label: 'Customers', href: '/customers', permission: 'customers.read', order: 45 },
  ],
  settingsPages: [],
  reportPages: [],
  pages: [
    { path: '/customers', title: 'Customers', permission: 'customers.read', load: () => import('../../platform/PluginPage.svelte') },
  ],
})
