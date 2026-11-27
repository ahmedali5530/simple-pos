import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const fiscalPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    {
      path: '/settings/fiscal',
      title: 'Fiscal (FBR/PRA)',
      permission: 'fiscal.write',
      load: () => import('./screens/SettingsPage.svelte'),
    },
    {
      path: '/reports/fiscal',
      title: 'Fiscal submissions',
      permission: 'fiscal.read',
      load: () => import('./screens/ReportPage.svelte'),
    },
  ],
})
