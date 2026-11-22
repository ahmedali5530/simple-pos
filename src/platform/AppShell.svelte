<script lang="ts">
  import BillingScreen from '../lib/billing/BillingScreen.svelte'
  import LoginScreen from '../lib/LoginScreen.svelte'
  import { location } from '../lib/route.svelte'
  import { session } from '../lib/session.svelte'
  import { request } from '../lib/api'
  import {
  filterNav,
  type PlatformManifest,
    type UiNavItem,
  } from './registry'
  import { staticManifestNav, staticSettingsPages } from '../plugins/registry'

  let manifest = $state<PlatformManifest | null>(null)
  let permissions = $state<string[]>(['*'])
  let nav = $state<UiNavItem[]>([])
  let settingsPages = $state<{ id: string; label: string; href: string }[]>([])
  let available = $state<{ id: string; enabled: boolean }[] | null>(null)

  const owners: Record<string, string> = {
    billing: 'pos',
    stock: 'inventory',
    reports: 'reports',
    cash: 'cash',
    settings: 'catalog',
  }

  function applyManifest(body: PlatformManifest) {
    manifest = body
    permissions = body.permissions?.length ? body.permissions : ['*']
    available = body.available ?? []
    const enabled = new Set(available.filter((plugin) => plugin.enabled).map((plugin) => plugin.id))
    const compiled = new Set((body.available ?? []).map((plugin) => plugin.id))
    const pageOwner: Record<string, string> = {
      items: 'catalog',
      sections: 'catalog',
      categories: 'catalog',
      units: 'catalog',
      system: 'printing',
    }
    settingsPages = staticSettingsPages().filter((page) => {
      const owner = pageOwner[page.id]
      if (!owner || !compiled.has(owner)) return true
      return enabled.has(owner)
    })
    const items = staticManifestNav().filter((item) => {
      if (item.id === 'settings') return settingsPages.length > 0
      const owner = owners[item.id]
      return !owner || enabled.size === 0 || enabled.has(owner)
    })
    if (!items.some((item) => item.id === 'settings') && settingsPages.length > 0) {
      items.push({
        id: 'settings',
        label: 'Settings',
        href: settingsPages[0].href,
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
      history.replaceState(null, '', '/billing')
      location.path = '/billing'
    }
  })

  $effect(() => {
    if (!session.user) return
    const load = () => {
      request<PlatformManifest>('/api/v1/platform/manifest')
        .then(applyManifest)
        .catch(() => {
          nav = []
        })
    }
    load()
    const onChange = () => load()
    window.addEventListener('plugins-changed', onChange)
    return () => window.removeEventListener('plugins-changed', onChange)
  })
</script>

{#if session.user}
  <BillingScreen pluginNav={nav} {settingsPages} {available} />
{:else}
  <LoginScreen />
{/if}

{#if manifest}
  <span class="sr-only" data-tenant={manifest.tenant_id} data-plugins={manifest.plugins?.length ?? 0}></span>
{/if}
