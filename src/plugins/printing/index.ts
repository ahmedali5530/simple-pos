import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const printingPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    {
      path: '/settings/printing',
      title: 'Printing',
      permission: 'settings.write',
      load: () => import('./screens/PrintingPage.svelte'),
    },
  ],
})
