import { enabledPlugins, whenPluginsReady } from './enabled.svelte'

/**
 * A plugin that turns a catalog item into an option picker. The POS host asks
 * before adding an item so a phone model opens its IMEI list instead of being
 * sold as an anonymous unit.
 */
export interface ItemOptionsProvider {
  plugin: string
  /** Load whatever `hasOptions` needs. Called for the current tenant. */
  prepare: () => Promise<void>
  hasOptions: (itemId: number) => boolean
  /**
   * When true, the POS variant sheet hides native multi-select rows and only
   * shows plugin contributions (matrix pick-one, etc.).
   */
  exclusive?: (itemId: number) => boolean
  /** Drop tenant-scoped state when the session changes. */
  reset?: () => void
}

const providers: ItemOptionsProvider[] = []

export function registerItemOptionsProvider(provider: ItemOptionsProvider) {
  providers.push(provider)
}

function activeProviders() {
  if (!enabledPlugins.ready) return []
  return providers.filter(
    (provider) =>
      enabledPlugins.compiled.includes(provider.plugin) &&
      enabledPlugins.ids.includes(provider.plugin),
  )
}

export async function prepareItemOptions() {
  await whenPluginsReady()
  await Promise.all(activeProviders().map((provider) => provider.prepare().catch(() => undefined)))
}

export function hasItemOptions(itemId: number) {
  return activeProviders().some((provider) => provider.hasOptions(itemId))
}

export function hasExclusiveItemOptions(itemId: number) {
  return activeProviders().some(
    (provider) => provider.hasOptions(itemId) && (provider.exclusive?.(itemId) ?? false),
  )
}

export function resetItemOptions() {
  for (const provider of providers) provider.reset?.()
}
