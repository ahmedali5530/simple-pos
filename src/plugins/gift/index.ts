import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const giftPlugin = defineUiPlugin({
  ...metadata,
  pages: [
    {
      path: '/settings/gift',
      title: 'Gift cards',
      permission: 'gift.write',
      load: () => import('./screens/SettingsPage.svelte'),
    },
    {
      path: '/reports/gift',
      title: 'Gift liability',
      permission: 'gift.read',
      load: () => import('./screens/LiabilityPage.svelte'),
    },
  ],
  contributions: metadata.contributions.map((entry) => {
    if (entry.id === 'gift.redeem') {
      return { ...entry, component: () => import('./screens/RedeemTender.svelte') }
    }
    return entry
  }),
})
