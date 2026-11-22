import { defineUiPlugin } from '../../platform/registry'

export const printingPlugin = defineUiPlugin({
  id: 'printing',
  permissions: ['printing.use'],
  nav: [],
  settingsPages: [
    { id: 'printing', label: 'Printing', href: '/settings/printing', permission: 'settings.write', order: 55 },
  ],
  reportPages: [],
  pages: [
    {
      path: '/settings/printing',
      title: 'Printing',
      permission: 'settings.write',
      load: () => import('./screens/PrintingPage.svelte'),
    },
  ],
})
