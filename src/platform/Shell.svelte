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
  let openGroup = $state<string | null>(null)
  let openSettingsGroup = $state<string | null>(null)
  let navEl = $state<HTMLElement | null>(null)
  const page = $derived(pageForPath(location.path, enabled, compiled))
  const reports = $derived(reportPagesForEnabled(enabled, compiled))
  const inSettings = $derived(location.path.startsWith('/settings'))
  const inReports = $derived(location.path.startsWith('/reports'))

  function settingsGroupActive(entry: UiSettingsPage) {
    return (entry.children ?? []).some((child) => child.href === location.path)
  }

  function toggleSettingsGroup(id: string) {
    openSettingsGroup = openSettingsGroup === id ? null : id
  }

  // Keep the group that owns the current settings page open.
  $effect(() => {
    if (!inSettings) {
      openSettingsGroup = null
      return
    }
    const active = settingsPages.find((entry) => settingsGroupActive(entry))
    if (active) openSettingsGroup = active.id
  })

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
    if (!href) return false
    if (location.path === href || location.path.startsWith(`${href}/`)) return true
    const section = href.split('/').filter(Boolean)[0]
    const current = location.path.split('/').filter(Boolean)[0]
    return (section === 'settings' || section === 'reports') && section === current
  }

  function groupActive(item: UiNavItem) {
    return (item.children ?? []).some(
      (child) => active(child.href ?? '') || groupActive(child),
    )
  }

  function toggleGroup(id: string) {
    openGroup = openGroup === id ? null : id
  }

  function closeOnOutside(event: MouseEvent) {
    if (!openGroup) return
    const target = event.target as Node | null
    if (navEl && target && navEl.contains(target)) return
    openGroup = null
  }

  function closeOnEscape(event: KeyboardEvent) {
    if (event.key === 'Escape') openGroup = null
  }

  // A new page closes whatever dropdown is open.
  $effect(() => {
    location.path
    openGroup = null
  })

  async function leave() {
    try {
      await logout()
    } catch {
      // The local session still closes.
    }
    signOut()
  }
</script>

<svelte:window onclick={closeOnOutside} onkeydown={closeOnEscape} />

<div class="app">
  <header class="topbar on-dark">
    <div class="brand">
      <div class="mark">{initials(shopName)}</div>
      <div>
        <h1>{shopName}</h1>
      </div>
    </div>
    <nav class="nav" aria-label="Main" bind:this={navEl}>
      {#each nav as item (item.id)}
        {@const href = item.href ?? ''}
        {#if item.children?.length}
          <div class="nav-group" class:open={openGroup === item.id}>
            <button
              type="button"
              class="nav-toggle"
              class:active={groupActive(item)}
              aria-haspopup="menu"
              aria-expanded={openGroup === item.id}
              onclick={() => toggleGroup(item.id)}
            >
              {item.label}
              <span class="chevron" aria-hidden="true"></span>
            </button>
            {#if openGroup === item.id}
              <div class="nav-menu" role="menu" aria-label={item.label}>
                {#each item.children as child (child.id)}
                  {@const childHref = child.href ?? ''}
                  <a
                    role="menuitem"
                    href={childHref || '#'}
                    class:active={active(childHref)}
                    onclick={(event) => followLink(event, childHref)}
                  >{child.label}</a>
                {/each}
              </div>
            {/if}
          </div>
        {:else if href}
          <a href={href} class:active={active(href)} onclick={(event) => followLink(event, href)}>{item.label}</a>
        {/if}
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
        {#each settingsPages as entry (entry.id)}
          {#if entry.children?.length}
            {@const open = openSettingsGroup === entry.id}
            <div class="settings-group" class:open>
              <button
                type="button"
                class="settings-toggle"
                class:active={settingsGroupActive(entry)}
                aria-haspopup="menu"
                aria-expanded={open}
                onclick={() => toggleSettingsGroup(entry.id)}
              >
                {entry.label}
                <span class="chevron" aria-hidden="true"></span>
              </button>
              {#if open}
                <div class="settings-menu" role="menu" aria-label={entry.label}>
                  {#each entry.children as child (child.id)}
                    {#if child.href}
                      <a
                        role="menuitem"
                        href={child.href}
                        class:active={location.path === child.href}
                        onclick={(event) => followLink(event, child.href!)}
                      >{child.label}</a>
                    {/if}
                  {/each}
                </div>
              {/if}
            </div>
          {:else if entry.href}
            <a href={entry.href} class:active={location.path === entry.href} onclick={(event) => followLink(event, entry.href!)}>{entry.label}</a>
          {/if}
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
