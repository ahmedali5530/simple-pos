import { defineUiPlugin } from '../../platform/registry'
import './options'

export const mobilePlugin = defineUiPlugin({
  id: 'mobile',
  permissions: ['mobile.read', 'mobile.write'],
  nav: [
    {
      id: 'mobile',
      label: 'Mobile',
      permission: 'mobile.read',
      order: 48,
      children: [
        {
          id: 'mobile_devices',
          label: 'Devices',
          href: '/mobile/devices',
          permission: 'mobile.read',
          order: 10,
        },
        {
          id: 'mobile_warranty',
          label: 'Warranty',
          href: '/mobile/warranty',
          permission: 'mobile.read',
          order: 20,
        },
      ],
    },
  ],
  settingsPages: [],
  reportPages: [],
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
  contributions: [
    {
      id: 'mobile.model',
      slot: 'catalog.item-form',
      order: 10,
      component: () => import('./screens/ModelFields.svelte'),
    },
    {
      id: 'mobile.device',
      slot: 'pos.item-options',
      order: 25,
      component: () => import('./screens/DeviceOptions.svelte'),
    },
  ],
})
