import { defineUiPlugin } from '../../platform/registry'

export const catalogPlugin = defineUiPlugin({
  id: 'catalog',
  permissions: ['catalog.read', 'catalog.write'],
  nav: [
    {
      id: 'settings',
      label: 'Settings',
      href: '/settings/items',
      permission: 'catalog.write',
      order: 50,
    },
  ],
  settingsPages: [
    { id: 'items', label: 'Items', href: '/settings/items', permission: 'catalog.write', order: 10 },
    { id: 'sections', label: 'Sections', href: '/settings/sections', permission: 'catalog.write', order: 20 },
    { id: 'categories', label: 'Categories', href: '/settings/categories', permission: 'catalog.write', order: 30 },
    { id: 'units', label: 'Units', href: '/settings/units', permission: 'catalog.write', order: 35 },
  ],
  reportPages: [],
})
