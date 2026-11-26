import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const catalogPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    { path: '/settings/items', title: 'Items', permission: 'catalog.write', load: () => import('./screens/ItemsPage.svelte') },
    { path: '/settings/sections', title: 'Sections', permission: 'catalog.write', load: () => import('./screens/SectionsPage.svelte') },
    { path: '/settings/categories', title: 'Categories', permission: 'catalog.write', load: () => import('./screens/CategoriesPage.svelte') },
    { path: '/settings/units', title: 'Units', permission: 'catalog.write', load: () => import('./screens/UnitsPage.svelte') },
  ],
})
