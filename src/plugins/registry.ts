import { defineUiPlugin, type UiPlugin } from '../platform/registry'
import { posPlugin } from './pos'
import { catalogPlugin } from './catalog'
import { inventoryPlugin } from './inventory'
import { cashPlugin } from './cash'
import { reportsPlugin } from './reports'
import { printingPlugin } from './printing'
import { adminPlugin } from './admin'

/** Static UI plugin registry — mirrors Rust static linking. */
export const uiPlugins: UiPlugin[] = [
  posPlugin,
  catalogPlugin,
  inventoryPlugin,
  cashPlugin,
  reportsPlugin,
  printingPlugin,
  adminPlugin,
].map(defineUiPlugin)

export function staticManifestNav() {
  return uiPlugins.flatMap((p) => p.nav).sort((a, b) => a.order - b.order)
}

export function staticSettingsPages() {
  return uiPlugins.flatMap((p) => p.settingsPages).sort((a, b) => a.order - b.order)
}

export function staticReportPages() {
  return uiPlugins.flatMap((p) => p.reportPages).sort((a, b) => a.order - b.order)
}
