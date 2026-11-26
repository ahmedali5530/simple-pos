import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'
import './options'

export const mobilePlugin = defineUiPlugin({
  ...metadata,
  pages: [
    {
      path: '/mobile/devices',
      title: 'Devices',
      permission: 'mobile.read',
      load: () => import('./screens/DevicesPage.svelte'),
    },
    {
      path: '/mobile/warranty',
      title: 'Warranty',
      permission: 'mobile.read',
      load: () => import('./screens/WarrantyPage.svelte'),
    },
  ],
  contributions: metadata.contributions.map((entry) => {
    if (entry.id === 'mobile.model') {
      return { ...entry, component: () => import('./screens/ModelFields.svelte') }
    }
    if (entry.id === 'mobile.device') {
      return { ...entry, component: () => import('./screens/DeviceOptions.svelte') }
    }
    return entry
  }),
})
