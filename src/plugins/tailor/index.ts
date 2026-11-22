import { defineUiPlugin } from '../../platform/registry'

export const tailorPlugin = defineUiPlugin({
  id: 'tailor',
  permissions: ['tailor.read', 'tailor.write'],
  nav: [{ id: 'tailor', label: 'Jobs', href: '/tailor', permission: 'tailor.read', order: 46 }],
  settingsPages: [],
  reportPages: [],
  pages: [{ path: '/tailor', title: 'Jobs', permission: 'tailor.read', load: () => import('./screens/JobsPage.svelte') }],
  contributions: [{ id: 'tailor.job', slot: 'pos.line-source', order: 20 }],
})
