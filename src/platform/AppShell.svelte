<script lang="ts">
  import LoginScreen from '../lib/LoginScreen.svelte'
  import { location } from './route.svelte'
  import { session } from './session.svelte'
  import { request } from './http'
  import { filterNav, type PlatformManifest, type UiNavItem, type UiSettingsPage } from './registry'
  import { firstSettingsHref, navForEnabled, settingsForEnabled } from '../plugins/registry'
  import { prepareItemOptions, resetItemOptions } from './itemOptions'
  import {
    markPluginsReady,
    setCompiledPlugins,
    setEnabledPlugins,
    resetPluginState,
  } from './enabled.svelte'
  import Shell from './Shell.svelte'

  let manifest = $state<PlatformManifest | null>(null)
  let permissions = $state<string[]>(['*'])
  let nav = $state<UiNavItem[]>([])
  let settingsPages = $state<UiSettingsPage[]>([])
  let enabledIds = $state<string[]>([])
  let compiledIds = $state<string[]>([])
  const enabled = $derived(new Set(enabledIds))
  const compiled = $derived(new Set(compiledIds))

  function applyManifest(body: PlatformManifest) {
    manifest = body
    permissions = body.permissions?.length ? body.permissions : ['*']
    const available = body.available ?? []
    enabledIds = available.filter((plugin) => plugin.enabled).map((plugin) => plugin.id)
    compiledIds = available.map((plugin) => plugin.id)
    setEnabledPlugins(enabledIds)
    setCompiledPlugins(compiledIds)
    markPluginsReady()
    resetItemOptions()
    void prepareItemOptions()
    settingsPages = settingsForEnabled(enabled, compiled)
    const items = navForEnabled(enabled, compiled).filter((item) => {
      if (item.id === 'settings') return settingsPages.length > 0
      return true
    })
    const settingsHome = firstSettingsHref(settingsPages)
    if (!items.some((item) => item.id === 'settings') && settingsHome) {
      items.push({
        id: 'settings',
        label: 'Settings',
        href: settingsHome,
        order: 50,
      })
    }
    nav = filterNav(items, permissions)
  }

  $effect(() => {
    if (!session.user && location.path !== '/login') {
      history.replaceState(null, '', '/login')
      location.path = '/login'
    } else if (session.user && location.path === '/login') {
      const first = nav.length ? nav[0].href : '/settings/system'
      history.replaceState(null, '', first)
      location.path = first
    }
  })

  $effect(() => {
    if (!session.user) {
      resetPluginState()
      resetItemOptions()
      return
    }
    const load = () => {
      request<PlatformManifest>('/api/v1/platform/manifest')
        .then(applyManifest)
        .catch((error: unknown) => {
          nav = []
          resetPluginState()
          console.error(error)
        })
    }
    load()
    const onChange = () => load()
    window.addEventListener('plugins-changed', onChange)
    return () => window.removeEventListener('plugins-changed', onChange)
  })
</script>

{#if session.user}
  <Shell {nav} {settingsPages} {enabled} {compiled} />
{:else}
  <LoginScreen {nav} />
{/if}

{#if manifest}
  <span class="sr-only" data-tenant={manifest.tenant_id} data-plugins={manifest.plugins?.length ?? 0}></span>
{/if}
