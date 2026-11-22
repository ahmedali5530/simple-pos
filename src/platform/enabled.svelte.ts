/** Plugin ids enabled for the signed-in shop. The shell fills this from the platform manifest. */
export const enabledPlugins = $state({ ids: [] as string[] })

export function setEnabledPlugins(ids: string[]) {
  enabledPlugins.ids = ids
}

export function pluginEnabled(id: string) {
  return enabledPlugins.ids.includes(id)
}
