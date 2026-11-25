import type { UiNavItem, UiPlugin, UiSettingsPage, UiSlotContribution } from '../platform/registry'
import { enabledPlugins } from '../platform/enabled.svelte'

/**
 * UI plugins are discovered from the filesystem, mirroring the generated Rust composition root.
 * A plugin is one directory: `src/plugins/<id>/index.ts` exports exactly one UI plugin.
 */
const modules = import.meta.glob<Record<string, unknown>>('./*/index.ts', { eager: true })

function isUiPlugin(value: unknown): value is UiPlugin {
  const candidate = value as Partial<UiPlugin> | null
  return (
    !!candidate &&
    typeof candidate.id === 'string' &&
    Array.isArray(candidate.nav) &&
    Array.isArray(candidate.settingsPages) &&
    Array.isArray(candidate.reportPages) &&
    Array.isArray(candidate.pages)
  )
}

function discover(): UiPlugin[] {
  return Object.entries(modules)
    .map(([path, exports]) => {
      const found = Object.values(exports).filter(isUiPlugin)
      if (found.length !== 1) {
        throw new Error(`${path} must export exactly one UI plugin (found ${found.length})`)
      }
      return found[0]
    })
    .sort((a, b) => a.id.localeCompare(b.id))
}

/** Discovered UI plugin registry — mirrors Rust static linking. */
export const uiPlugins: UiPlugin[] = discover()

  if (import.meta.env.DEV) {
  const claims = new Map<string, string>()
  const claim = (href: string, owner: string) => {
    const existing = claims.get(href)
    if (existing && existing !== owner) {
      console.error(`[plugins] ${href} is claimed by both ${existing} and ${owner}`)
    }
    claims.set(href, owner)
  }
  for (const plugin of uiPlugins) {
    for (const href of navHrefs(plugin.nav)) claim(href, plugin.id)
    for (const href of settingsHrefs(plugin.settingsPages)) claim(href, plugin.id)
    for (const page of plugin.reportPages) {
      if (page.href) claim(page.href, plugin.id)
    }
  }
}

function pluginIsOn(plugin: UiPlugin, enabled: Set<string>, compiled: Set<string>) {
  if (!plugin.requiresBackend) return true
  if (!compiled.has(plugin.id)) return false
  return enabled.has(plugin.id)
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
    .filter((plugin) => pluginIsOn(plugin, enabled, compiled))
    .flatMap((plugin) => plugin.nav)
    .sort((a, b) => a.order - b.order)
}

export function settingsForEnabled(enabled: Set<string>, compiled: Set<string>) {
  return uiPlugins
    .filter((plugin) => pluginIsOn(plugin, enabled, compiled))
    .flatMap((plugin) => plugin.settingsPages)
    .sort((a, b) => a.order - b.order)
}

/** First navigable settings href (walks into groups). */
export function firstSettingsHref(pages: UiSettingsPage[]): string | null {
  for (const page of pages) {
    if (page.href) return page.href
    const nested = firstSettingsHref(page.children ?? [])
    if (nested) return nested
  }
  return null
}

export function staticManifestNav() {
  return uiPlugins.flatMap((p) => p.nav).sort((a, b) => a.order - b.order)
}

export function staticSettingsPages() {
  return uiPlugins.flatMap((p) => p.settingsPages).sort((a, b) => a.order - b.order)
}

export function pageForPath(path: string, enabled: Set<string>, compiled: Set<string>) {
  const pages = uiPlugins
    .filter((plugin) => pluginIsOn(plugin, enabled, compiled))
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
    .filter((plugin) => pluginIsOn(plugin, enabled, compiled))
    .flatMap((plugin) => plugin.reportPages)
    .sort((a, b) => a.order - b.order)
}

export function staticReportPages() {
  return uiPlugins.flatMap((p) => p.reportPages).sort((a, b) => a.order - b.order)
}

function navHrefs(items: UiNavItem[]): string[] {
  return items.flatMap((item) => [
    ...(item.href ? [item.href] : []),
    ...navHrefs(item.children ?? []),
  ])
}

function settingsHrefs(items: UiSettingsPage[]): string[] {
  return items.flatMap((item) => [
    ...(item.href ? [item.href] : []),
    ...settingsHrefs(item.children ?? []),
  ])
}

/** The plugin that owns a screen. Null means the path is not a plugin screen. */
export function pluginForPath(path: string): string | null {
  for (const plugin of uiPlugins) {
    const hrefs = [
      ...navHrefs(plugin.nav),
      ...settingsHrefs(plugin.settingsPages),
      ...plugin.reportPages.map((page) => page.href).filter(Boolean),
    ]
    if (hrefs.some((href) => path === href || path.startsWith(`${href}/`))) return plugin.id
    if (plugin.reportPages.length > 0 && (path === '/reports' || path.startsWith('/reports/'))) return plugin.id
  }
  return null
}
