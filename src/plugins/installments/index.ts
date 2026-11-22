import { defineUiPlugin } from '../../platform/registry'

export const installmentsPlugin = defineUiPlugin({
  id: 'installments',
  permissions: ['installments.read', 'installments.write'],
  nav: [
    {
      id: 'installments',
      label: 'Installments',
      href: '/installments',
      permission: 'installments.read',
      order: 47,
    },
  ],
  settingsPages: [],
  reportPages: [],
  pages: [
    {
      path: '/installments',
      title: 'Installments',
      permission: 'installments.read',
      load: () => import('../../platform/PluginPage.svelte'),
    },
  ],
  contributions: [{ id: 'installments.schedule', slot: 'pos.tender', order: 30 }],
})
