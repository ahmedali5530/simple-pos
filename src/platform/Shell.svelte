<script lang="ts">
  import { Button } from '../lib/ui'
  import { loadBootstrap, logout } from '../lib/api'
  import { followLink, location } from './route.svelte'
  import { session, signOut } from './session.svelte'
  import type { UiNavItem, UiPage, UiSettingsPage } from './registry'
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
  let mobileNavOpen = $state(false)
  let sidebarCollapsed = $state(false)
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

  $effect(() => {
    if (!inSettings) {
      openSettingsGroup = null
      return
    }
    const activeEntry = settingsPages.find((entry) => settingsGroupActive(entry))
    if (activeEntry) openSettingsGroup = activeEntry.id
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

  $effect(() => {
    location.path
    mobileNavOpen = false
  })

  // Keep the group that owns the current page open.
  $effect(() => {
    const match = nav.find((item) => item.children?.length && groupActive(item))
    if (match) openGroup = match.id
  })

  async function leave() {
    try {
      await logout()
    } catch {
      // The local session still closes.
    }
    signOut()
  }

  function go(event: MouseEvent, href: string) {
    followLink(event, href)
    mobileNavOpen = false
  }
</script>

<div class="ui-shell app">
  <header class="shell-mobile-bar">
    <Button variant="secondary" size="sm" onclick={() => (mobileNavOpen = !mobileNavOpen)}>
      Menu
    </Button>
    <strong>{shopName}</strong>
  </header>

  <aside
    class="ui-sidebar"
    class:ui-sheet={mobileNavOpen}
    data-side="left"
    data-collapsed={sidebarCollapsed ? 'true' : undefined}
    data-open={mobileNavOpen ? 'true' : undefined}
    data-state={mobileNavOpen ? 'open' : undefined}
    aria-label="Main"
  >
    <div class="sidebar-brand">
      <div class="sidebar-mark">{initials(shopName)}</div>
      <div data-slot="label">
        <h1>{shopName}</h1>
      </div>
      <button
        type="button"
        class="ui-btn sidebar-collapse"
        data-variant="ghost"
        data-size="sm"
        data-icon=""
        aria-label={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        aria-pressed={sidebarCollapsed}
        onclick={() => (sidebarCollapsed = !sidebarCollapsed)}
      >
        {sidebarCollapsed ? '»' : '«'}
      </button>
    </div>

    <nav class="sidebar-nav">
      {#each nav as item (item.id)}
        {@const href = item.href ?? ''}
        {#if item.children?.length}
          {@const open = openGroup === item.id}
          <div class="ui-sidebar-group sidebar-group" class:open>
            <button
              type="button"
              class="ui-nav-item sidebar-group-toggle"
              data-active={groupActive(item) ? 'true' : undefined}
              aria-expanded={open}
              onclick={() => toggleGroup(item.id)}
            >
              <span data-slot="label">{item.label}</span>
              <span class="chevron" aria-hidden="true"></span>
            </button>
            {#if open}
              <div class="sidebar-submenu" role="group" aria-label={item.label}>
                {#each item.children as child (child.id)}
                  {@const childHref = child.href ?? ''}
                  {#if childHref}
                    <a
                      class="ui-nav-item"
                      href={childHref}
                      aria-current={active(childHref) ? 'page' : undefined}
                      data-active={active(childHref) ? 'true' : undefined}
                      onclick={(event) => go(event, childHref)}
                    >
                      <span data-slot="label">{child.label}</span>
                    </a>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
        {:else if href}
          <a
            class="ui-nav-item"
            {href}
            aria-current={active(href) ? 'page' : undefined}
            data-active={active(href) ? 'true' : undefined}
            onclick={(event) => go(event, href)}
          >
            <span data-slot="label">{item.label}</span>
          </a>
        {/if}
      {/each}
    </nav>

    <div class="sidebar-footer">
      {#if session.user}
        <div class="sidebar-who" data-slot="label">{session.user.name}</div>
        <Button variant="secondary" data-block="" onclick={leave}>Sign out</Button>
      {:else}
        <Button variant="secondary" href="/login" onclick={(event) => go(event, '/login')}>Sign in</Button>
      {/if}
    </div>
  </aside>

  <div class="shell-main">
    {#if inSettings}
      <div class="settings-layout">
        <nav class="settings-rail" aria-label="Settings">
          {#each settingsPages as entry (entry.id)}
            {#if entry.children?.length}
              {@const open = openSettingsGroup === entry.id}
              <div class="settings-group" class:open>
                <button
                  type="button"
                  class="ui-nav-item settings-group-toggle"
                  data-active={settingsGroupActive(entry) ? 'true' : undefined}
                  aria-expanded={open}
                  onclick={() => toggleSettingsGroup(entry.id)}
                >
                  <span data-slot="label">{entry.label}</span>
                  <span class="chevron" aria-hidden="true"></span>
                </button>
                {#if open}
                  <div class="settings-submenu" role="group" aria-label={entry.label}>
                    {#each entry.children as child (child.id)}
                      {#if child.href}
                        <a
                          class="ui-nav-item"
                          href={child.href}
                          aria-current={location.path === child.href ? 'page' : undefined}
                          data-active={location.path === child.href ? 'true' : undefined}
                          onclick={(event) => followLink(event, child.href!)}
                        >
                          <span data-slot="label">{child.label}</span>
                        </a>
                      {/if}
                    {/each}
                  </div>
                {/if}
              </div>
            {:else if entry.href}
              <a
                class="ui-nav-item"
                href={entry.href}
                aria-current={location.path === entry.href ? 'page' : undefined}
                data-active={location.path === entry.href ? 'true' : undefined}
                onclick={(event) => followLink(event, entry.href!)}
              >
                <span data-slot="label">{entry.label}</span>
              </a>
            {/if}
          {/each}
        </nav>
        <div class="settings-body">
          {@render outlet(page)}
        </div>
      </div>
    {:else if inReports}
      <div class="settings-layout">
        <nav class="settings-rail" aria-label="Reports">
          {#each reports as entry (entry.href)}
            <a
              class="ui-nav-item"
              href={entry.href}
              aria-current={location.path === entry.href ? 'page' : undefined}
              data-active={location.path === entry.href ? 'true' : undefined}
              onclick={(event) => followLink(event, entry.href)}
            >
              <span data-slot="label">{entry.label}</span>
            </a>
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
