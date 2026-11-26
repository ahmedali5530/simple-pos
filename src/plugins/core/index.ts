import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const corePlugin = defineUiPlugin({
  ...metadata,
  pages: [
    { path: '/settings/system', title: 'System', permission: 'settings.write', load: () => import('./screens/SystemPage.svelte') },
  ],
})
