/** Plugin ids enabled for the signed-in shop. The shell fills this from the platform manifest. */
export const enabledPlugins = $state({
  ids: [] as string[],
  /** Plugin ids compiled into the backend. */
  compiled: [] as string[],
  /** True once a manifest has been applied. Slots stay hidden before that. */
  ready: false,
})

export function setEnabledPlugins(ids: string[]) {
  enabledPlugins.ids = ids
}

export function setCompiledPlugins(ids: string[]) {
  enabledPlugins.compiled = ids
}

export function markPluginsReady() {
  enabledPlugins.ready = true
}

export function resetPluginState() {
  enabledPlugins.ids = []
  enabledPlugins.compiled = []
  enabledPlugins.ready = false
}

export function pluginEnabled(id: string) {
  return enabledPlugins.ids.includes(id)
}
