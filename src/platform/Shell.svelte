<script lang="ts">
  import { Button } from '../lib/ui'
  import { loadBootstrap, logout } from '../lib/api'
  import { followLink, location } from './route.svelte'
  import { session, signOut } from './session.svelte'
  import type { UiNavItem, UiPage, UiReportPage, UiSettingsPage } from './registry'
  import { pageForPath, reportPagesForEnabled } from '../plugins/registry'

  interface Props {
    nav: UiNavItem[]
    settingsPages: UiSettingsPage[]
    enabled: Set<string>
    compiled: Set<string>
  }

  let { nav, settingsPages, enabled, compiled }: Props = $props()

  let shopName = $state('Shop')
  let loadedPath = $state('')
  let loaded = $state<{ default: import('svelte').Component } | null>(null)
  const page = $derived(pageForPath(location.path, enabled, compiled))
  const reports = $derived(reportPagesForEnabled(enabled, compiled))
  const inSettings = $derived(location.path.startsWith('/settings'))
  const inReports = $derived(location.path.startsWith('/reports'))

  $effect(() => {
    const current = page
    if (!current) {
      loaded = null
      loadedPath = ''
      return
    }
    let cancelled = false
    current.load().then((mod) => {
      if (cancelled) return
      loaded = mod
      loadedPath = current.path
    })
    return () => {
      cancelled = true
    }
  })

  $effect(() => {
    loadBootstrap()
      .then((status) => {
        shopName = status.shop_name.trim() || 'Shop'
      })
      .catch((error: unknown) => {
        shopName = error instanceof Error ? error.message : 'Shop'
      })
  })

  function initials(name: string) {
    const words = name.trim().split(/\s+/).filter(Boolean)
    const letters = words.length >= 2 ? `${words[0][0]}${words[1][0]}` : name.slice(0, 2)
    return letters.toUpperCase() || 'SH'
  }

  function active(href: string) {
    if (location.path === href || location.path.startsWith(`${href}/`)) return true
    const section = href.split('/').filter(Boolean)[0]
    const current = location.path.split('/').filter(Boolean)[0]
    return (section === 'settings' || section === 'reports') && section === current
  }

  async function leave() {
    try {
      await logout()
    } catch {
      // The local session still closes.
    }
    signOut()
  }
</script>

<div class="app">
  <header class="topbar on-dark">
    <div class="brand">
      <div class="mark">{initials(shopName)}</div>
      <div>
        <h1>{shopName}</h1>
      </div>
    </div>
    <nav class="nav" aria-label="Main">
      {#each nav as item (item.id)}
        <a href={item.href} class:active={active(item.href)} onclick={(event) => followLink(event, item.href)}>{item.label}</a>
      {/each}
    </nav>
    <div class="top-actions">
      {#if session.user}
        <span class="who">{session.user.name}</span>
        <Button variant="secondary" onclick={leave}>Sign out</Button>
      {:else}
        <Button variant="secondary" href="/login" onclick={(event) => followLink(event, '/login')}>Sign in</Button>
      {/if}
    </div>
  </header>

  {#if inSettings}
    <div class="settings">
      <nav class="settings-nav" aria-label="Settings">
        {#each settingsPages as entry (entry.href)}
          <a href={entry.href} class:active={location.path === entry.href} onclick={(event) => followLink(event, entry.href)}>{entry.label}</a>
        {/each}
      </nav>
      <div class="settings-body">
        {@render outlet(page)}
      </div>
    </div>
  {:else if inReports}
    <div class="settings">
      <nav class="settings-nav" aria-label="Reports">
        {#each reports as entry (entry.href)}
          <a href={entry.href} class:active={location.path === entry.href} onclick={(event) => followLink(event, entry.href)}>{entry.label}</a>
        {/each}
      </nav>
      <div class="settings-body">
        {@render outlet(page)}
      </div>
    </div>
  {:else}
    {@render outlet(page)}
  {/if}
</div>

{#snippet outlet(current: UiPage | null)}
  <div class="outlet" data-density={current?.density}>
    {#if current && loaded && loadedPath === current.path}
      {@const Page = loaded.default}
      <Page />
    {:else if current}
      <p class="loading">Opening…</p>
    {:else}
      <main class="stage">
        <section class="place">
          <h2>404</h2>
          <p class="hint">That page is not in the till.</p>
          <p class="hint"><a href="/billing" onclick={(event) => followLink(event, '/billing')}>Back to billing</a></p>
        </section>
      </main>
    {/if}
  </div>
{/snippet}
