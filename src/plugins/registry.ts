import { defineUiPlugin, type UiPlugin, type UiSlotContribution } from '../platform/registry'
import { enabledPlugins } from '../platform/enabled.svelte'
import { posPlugin } from './pos'
import { catalogPlugin } from './catalog'
import { inventoryPlugin } from './inventory'
import { cashPlugin } from './cash'
import { reportsPlugin } from './reports'
import { printingPlugin } from './printing'
import { adminPlugin } from './admin'
import { customersPlugin } from './customers'
import { tailorPlugin } from './tailor'
import { installmentsPlugin } from './installments'
import { mobilePlugin } from './mobile'

/** Static UI plugin registry — mirrors Rust static linking. */
export const uiPlugins: UiPlugin[] = [
  posPlugin,
  catalogPlugin,
  inventoryPlugin,
  cashPlugin,
  reportsPlugin,
  printingPlugin,
  adminPlugin,
  customersPlugin,
  tailorPlugin,
  installmentsPlugin,
  mobilePlugin,
].map(defineUiPlugin)

function pluginIsOn(id: string, enabled: Set<string>, compiled: Set<string>) {
  if (!compiled.has(id)) return true
  return enabled.has(id)
}

export interface ResolvedSlotContribution extends UiSlotContribution {
  owner: string
}

/**
 * Contributions for a named outlet. A slot only renders when its owning plugin is both compiled
 * into the backend and enabled for this shop; before the manifest loads, nothing renders.
 */
export function slotsFor(slot: string): ResolvedSlotContribution[] {
  if (!enabledPlugins.ready) return []
  return uiPlugins
    .filter(
      (plugin) =>
        enabledPlugins.compiled.includes(plugin.id) && enabledPlugins.ids.includes(plugin.id),
    )
    .flatMap((plugin) =>
      plugin.contributions
        .filter((contribution) => contribution.slot === slot && contribution.component != null)
        .map((contribution) => ({ ...contribution, owner: plugin.id })),
    )
    .sort(
      (a, b) =>
        a.order - b.order || a.owner.localeCompare(b.owner) || a.id.localeCompare(b.id),
    )
}

export function navForEnabled(enabled: Set<string>, compiled: Set<string>) {
  return uiPlugins
    .filter((plugin) => pluginIsOn(plugin.id, enabled, compiled))
    .flatMap((plugin) => plugin.nav)
    .sort((a, b) => a.order - b.order)
}

export function settingsForEnabled(enabled: Set<string>, compiled: Set<string>) {
  return uiPlugins
    .filter((plugin) => pluginIsOn(plugin.id, enabled, compiled))
    .flatMap((plugin) => plugin.settingsPages)
    .sort((a, b) => a.order - b.order)
}

export function staticManifestNav() {
  return uiPlugins.flatMap((p) => p.nav).sort((a, b) => a.order - b.order)
}

export function staticSettingsPages() {
  return uiPlugins.flatMap((p) => p.settingsPages).sort((a, b) => a.order - b.order)
}

export function pageForPath(path: string, enabled: Set<string>, compiled: Set<string>) {
  const pages = uiPlugins
    .filter((plugin) => pluginIsOn(plugin.id, enabled, compiled))
    .flatMap((plugin) => plugin.pages)
  return (
    pages.find((page) => page.path === path) ??
    pages
      .filter((page) => path.startsWith(`${page.path}/`))
      .sort((a, b) => b.path.length - a.path.length)[0] ??
    null
  )
}

export function reportPagesForEnabled(enabled: Set<string>, compiled: Set<string>) {
  return uiPlugins
    .filter((plugin) => pluginIsOn(plugin.id, enabled, compiled))
    .flatMap((plugin) => plugin.reportPages)
    .sort((a, b) => a.order - b.order)
}

export function staticReportPages() {
  return uiPlugins.flatMap((p) => p.reportPages).sort((a, b) => a.order - b.order)
}

/** The plugin that owns a screen. Null means the path is not a plugin screen. */
export function pluginForPath(path: string): string | null {
  for (const plugin of uiPlugins) {
    const hrefs = [
      ...plugin.nav.map((item) => item.href),
      ...plugin.settingsPages.map((page) => page.href),
      ...plugin.reportPages.map((page) => page.href),
    ]
    if (hrefs.some((href) => path === href || path.startsWith(`${href}/`))) return plugin.id
    if (plugin.reportPages.length > 0 && (path === '/reports' || path.startsWith('/reports/'))) return plugin.id
  }
  return null
}
