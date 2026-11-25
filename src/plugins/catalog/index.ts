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
    {
      id: 'catalog',
      label: 'Catalog',
      permission: 'catalog.write',
      order: 10,
      children: [
        { id: 'items', label: 'Items', href: '/settings/items', permission: 'catalog.write', order: 10 },
        { id: 'sections', label: 'Sections', href: '/settings/sections', permission: 'catalog.write', order: 20 },
        { id: 'categories', label: 'Categories', href: '/settings/categories', permission: 'catalog.write', order: 30 },
        { id: 'units', label: 'Units', href: '/settings/units', permission: 'catalog.write', order: 35 },
      ],
    },
  ],
  reportPages: [],
  slotsProvided: ['catalog.item-form'],
  pages: [
    { path: '/settings/items', title: 'Items', permission: 'catalog.write', load: () => import('./screens/ItemsPage.svelte') },
    { path: '/settings/sections', title: 'Sections', permission: 'catalog.write', load: () => import('./screens/SectionsPage.svelte') },
    { path: '/settings/categories', title: 'Categories', permission: 'catalog.write', load: () => import('./screens/CategoriesPage.svelte') },
    { path: '/settings/units', title: 'Units', permission: 'catalog.write', load: () => import('./screens/UnitsPage.svelte') },
  ],
})
