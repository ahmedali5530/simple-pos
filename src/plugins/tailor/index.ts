import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const tailorPlugin = defineUiPlugin({
  ...metadata,
  pages: [{ path: '/tailor', title: 'Jobs', permission: 'tailor.read', load: () => import('./screens/JobsPage.svelte') }],
})
