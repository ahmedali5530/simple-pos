import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const paymentsPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    {
      path: '/settings/payments',
      title: 'Payment types',
      permission: 'payments.write',
      load: () => import('./screens/SettingsPage.svelte'),
    },
    {
      path: '/reports/payments',
      title: 'Payment mix',
      permission: 'payments.read',
      load: () => import('./screens/ReportPage.svelte'),
    },
  ],
  contributions: metadata.contributions.map((entry) => {
    if (entry.id === 'payments.tender') {
      return { ...entry, component: () => import('./screens/TenderPanel.svelte') }
    }
    return entry
  }),
})
