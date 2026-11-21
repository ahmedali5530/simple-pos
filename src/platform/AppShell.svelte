<script lang="ts">
  import BillingScreen from '../lib/billing/BillingScreen.svelte'
  import LoginScreen from '../lib/LoginScreen.svelte'
  import { location } from '../lib/route.svelte'
  import { session } from '../lib/session.svelte'
  import { request } from '../lib/api'
  import {
    filterNav,
    mergeNav,
    type PlatformManifest,
    type UiNavItem,
  } from './registry'
  import { staticManifestNav } from '../plugins/registry'

  let manifest = $state<PlatformManifest | null>(null)
  let permissions = $state<string[]>(['*'])
  let nav = $state<UiNavItem[]>(filterNav(staticManifestNav(), ['*']))

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
    request<PlatformManifest>('/api/v1/platform/manifest')
      .then((body) => {
        manifest = body
        permissions = body.permissions?.length ? body.permissions : ['*']
        const apiNav = body.nav?.length
          ? body.nav
          : (body.plugins ?? []).flatMap((p) => p.nav ?? [])
        nav = filterNav(mergeNav(apiNav, staticManifestNav()), permissions)
      })
      .catch(() => {
        nav = filterNav(staticManifestNav(), permissions)
      })
  })
</script>

{#if session.user}
  <BillingScreen pluginNav={nav} />
{:else}
  <LoginScreen />
{/if}

{#if manifest}
  <span class="sr-only" data-tenant={manifest.tenant_id} data-plugins={manifest.plugins?.length ?? 0}></span>
{/if}
