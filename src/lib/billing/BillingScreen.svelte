<script lang="ts">
  import { tick } from 'svelte'
  import {
    currentSession,
    loadPos,
    logout,
    postBill,
    rememberSection,
    removeItemPicture,
    saveCatalogItem,
    saveItemPicture,
    type CatalogItem,
  } from '../api'
  import { embeddedSale, findEmbedded, parseEmbedded } from '../barcode'
  import { formatMoney } from '../money'
  import { printReceipt } from '../print'
  import { followLink, location, reportsPage, screenOf, settingsPage } from '../route.svelte'
  import { session, signOut } from '../session.svelte'
  import type { CartLine, ItemVariant, PosItem, PosSnapshot } from '../types'
  import AttentionDialog from './AttentionDialog.svelte'
  import MeasureDialog from './MeasureDialog.svelte'
  import CashScreen from './CashScreen.svelte'
  import ReportsScreen from './ReportsScreen.svelte'
  import SettingsScreen from './SettingsScreen.svelte'
  import StockScreen from './StockScreen.svelte'
  import SectionPanel from './SectionPanel.svelte'
  import VariantDialog from './VariantDialog.svelte'
  import PluginAdmin from '../../plugins/admin/screens/PluginAdmin.svelte'
  import type { UiNavItem } from '../../platform/registry'
  import { pluginForPath } from '../../plugins/registry'

  interface Props {
    pluginNav?: UiNavItem[]
    settingsPages?: { id: string; label: string; href: string }[]
    available?: { id: string; enabled: boolean }[] | null
  }
  let { pluginNav, settingsPages = [], available = null }: Props = $props()

  let snapshot = $state<PosSnapshot | null>(null)
  let loadFailed = $state(false)
  let attention = $state<{ tone: 'error' | 'warning'; message: string } | null>(null)
  let carts = $state<Record<number, CartLine[]>>({})
  let filters = $state<Record<number, number | 'all'>>({})
  let posted = $state<Record<number, string>>({})
  let focusId = $state<number | null>(null)
  let savingItem = $state(false)
  let measure = $state<{ sectionId: number; itemId: number } | null>(null)
  let postingId = $state<number | null>(null)
  let customers = $state<Record<number, string>>({})
  let skipPrint = $state(false)
  let now = $state(new Date())
  let pending = $state<{ sectionId: number; itemId: number } | null>(null)
  let lastKey = $state<Record<number, string>>({})
  let focusSave: Promise<void> = Promise.resolve()

  const sections = $derived(snapshot?.sections ?? [])
  const active = $derived(sections.find((section) => section.id === focusId) ?? sections[0] ?? null)
  const screen = $derived.by(() => {
    const path = location.path
    const raw = screenOf(path)
    const owner = pluginForPath(path)
    if (available == null) return owner ? 'pending' : raw
    if (owner) {
      const row = available.find((plugin) => plugin.id === owner)
      if (row && !row.enabled) return 'missing'
    }
    if (raw === 'settings' && settingsPages.length > 0 && !settingsPages.some((page) => page.href === path)) {
      return 'missing'
    }
    return raw
  })
  const subtitle = $derived(
    screen === 'settings'
      ? 'Settings'
      : screen === 'stock'
        ? 'Stock'
        : screen === 'reports'
          ? 'Reports'
          : screen === 'cash'
            ? 'Cash closing'
          : screen === 'missing'
            ? 'Not found'
            : (active?.name ?? 'No sections on this till'),
  )

  $effect(() => {
    if (session.user) currentSession().catch(() => {})
    loadPos()
      .then(async (pos) => {
        snapshot = pos
        const saved = pos.sections.some((section) => section.id === pos.last_section_id)
        focusId = saved ? pos.last_section_id : (pos.sections[0]?.id ?? null)
        await tick()
        if (screenOf(location.path) === 'billing' && focusId != null) {
          document.querySelector<HTMLInputElement>(`[data-scan="${focusId}"]`)?.focus()
        }
      })
      .catch((error: unknown) => {
        loadFailed = true
        attention = {
          tone: 'error',
          message: error instanceof Error ? error.message : 'Could not load the till.',
        }
      })
  })

  function linesFor(sectionId: number) {
    return carts[sectionId] ?? []
  }

  function reserved(sectionId: number, itemId: number) {
    return linesFor(sectionId)
      .filter((line) => line.item_id === itemId)
      .reduce((sum, line) => sum + line.qty * line.units, 0)
  }

  const pendingChoice = $derived.by(() => {
    if (!pending || !snapshot) return null
    const section = snapshot.sections.find((entry) => entry.id === pending.sectionId)
    const item = section?.items.find((entry) => entry.id === pending.itemId)
    if (!section || !item) return null
    return { sectionId: section.id, item, available: item.stock - reserved(section.id, item.id) }
  })

  function remember(id: number) {
    focusSave = focusSave
      .catch(() => undefined)
      .then(async () => {
        if (focusId !== id) return
        try {
          await rememberSection(id)
        } catch (error) {
          if (focusId !== id) return
          attention = {
            tone: 'error',
            message: error instanceof Error ? error.message : 'Could not remember this section.',
          }
        }
      })
  }

  async function focusSection(id: number, scan = true) {
    const changed = focusId !== id
    focusId = id
    if (changed) remember(id)
    if (!scan) return
    await tick()
    document.querySelector<HTMLInputElement>(`[data-scan="${id}"]`)?.focus()
  }

  function addLine(
    sectionId: number,
    item: PosItem,
    variant: ItemVariant | null,
    qty = 1,
    priced?: { rate: number; units: number | null; label: string; code?: string | null; entry?: string | null },
  ) {
    focusId = sectionId
    const units = priced?.units ?? variant?.units ?? 1
    const rate = priced?.rate ?? variant?.rate ?? item.rate
    if (qty < 1) return false
    if (item.stock - reserved(sectionId, item.id) < units * qty) {
      attention = { tone: 'warning', message: `Only ${item.stock} left for ${item.name}.` }
      return false
    }
    const key = `${item.id}:${variant?.id ?? 0}:${rate}:${units}`
    let name = variant ? `${item.name} · ${variant.name}` : item.name
    if (priced?.label) name = `${name} · ${priced.label}`
    const code = priced?.code ?? null
    const entry = priced?.entry ?? null
    const lines = linesFor(sectionId)
    const existing = lines.find((line) => line.key === key)
    carts = {
      ...carts,
      [sectionId]: existing
        ? lines.map((line) => (line.key === key ? { ...line, qty: line.qty + qty } : line))
        : [...lines, { key, item_id: item.id, variant_id: variant?.id ?? null, name, rate, units, qty, code, entry }],
    }
    lastKey = { ...lastKey, [sectionId]: key }
    posted = { ...posted, [sectionId]: '' }
    attention = null
    return true
  }

  function bumpLast(sectionId: number, by: number) {
    const key = lastKey[sectionId]
    const line = key ? linesFor(sectionId).find((entry) => entry.key === key) : undefined
    if (!line) {
      attention = { tone: 'warning', message: 'Add an item before using +quantity.' }
      return false
    }
    return setQty(sectionId, key, line.qty + by)
  }

  function asksForValue(item: PosItem) {
    return item.variants.length === 0 && (item.prompt === 'quantity' || item.prompt === 'price')
  }

  function openMeasure(sectionId: number, item: PosItem) {
    focusId = sectionId
    if (!item.base_rate) {
      attention = { tone: 'error', message: `Set a base rate for ${item.name}.` }
      return false
    }
    measure = { sectionId, itemId: item.id }
    attention = null
    return true
  }

  function requestAdd(sectionId: number, item: PosItem) {
    focusId = sectionId
    if (item.variants.length > 0) {
      pending = { sectionId, itemId: item.id }
      attention = null
      return
    }
    if (asksForValue(item)) {
      openMeasure(sectionId, item)
      return
    }
    addLine(sectionId, item, null, 1)
  }

  function confirmVariants(picks: { variant: ItemVariant; qty: number }[]) {
    if (!pendingChoice) return
    for (const pick of picks) addLine(pendingChoice.sectionId, pendingChoice.item, pick.variant, pick.qty)
    pending = null
  }

  function scan(sectionId: number, code: string, qty: number): 'done' | 'rejected' | 'search' {
    const plus = /^\+(\d+)$/.exec(code)
    if (plus) return bumpLast(sectionId, Number(plus[1])) ? 'done' : 'rejected'
    const section = sections.find((entry) => entry.id === sectionId)
    const byVariant = section?.items.find((item) => item.variants.some((variant) => variant.barcode === code))
    if (byVariant) {
      const variant = byVariant.variants.find((entry) => entry.barcode === code) ?? null
      return addLine(sectionId, byVariant, variant, qty) ? 'done' : 'rejected'
    }
    const item = section?.items.find((entry) => entry.barcode === code)
    if (item) {
      if (item.variants.length > 0) {
        focusId = sectionId
        pending = { sectionId, itemId: item.id }
        attention = null
        return 'done'
      }
      if (asksForValue(item)) return openMeasure(sectionId, item) ? 'done' : 'rejected'
      return addLine(sectionId, item, null, qty) ? 'done' : 'rejected'
    }
    const embedded = snapshot ? parseEmbedded(code, snapshot.barcode_rules) : null
    if (embedded && section) {
      const found = findEmbedded(section.items, embedded.itemCode)
      if (!found) {
        attention = { tone: 'warning', message: 'That barcode is not in this section.' }
        return 'rejected'
      }
      if (found.variant == null && found.item.variants.length > 0) {
        attention = { tone: 'warning', message: `${found.item.name} needs an option.` }
        return 'rejected'
      }
      const unitRate = found.variant?.rate ?? found.item.rate
      const priced = embeddedSale(unitRate, embedded)
      if (!priced) {
        attention = { tone: 'error', message: 'That barcode could not be priced.' }
        return 'rejected'
      }
      return addLine(sectionId, found.item, found.variant, qty, { ...priced, code }) ? 'done' : 'rejected'
    }
    return 'search'
  }

  function pick(sectionId: number, item: PosItem, variant: ItemVariant | null, qty: number) {
    focusId = sectionId
    if (variant) return addLine(sectionId, item, variant, qty)
    if (item.variants.length > 0) {
      pending = { sectionId, itemId: item.id }
      attention = null
      return true
    }
    if (asksForValue(item)) return openMeasure(sectionId, item)
    return addLine(sectionId, item, null, qty)
  }

  const measureChoice = $derived.by(() => {
    if (!measure || !snapshot) return null
    const section = snapshot.sections.find((entry) => entry.id === measure.sectionId)
    const item = section?.items.find((entry) => entry.id === measure.itemId)
    if (!section || !item || !item.base_rate) return null
    if (item.prompt !== 'quantity' && item.prompt !== 'price') return null
    return { sectionId: section.id, item }
  })

  function confirmMeasure(quote: { rate: number; units: number; qty: number; label: string; entry: string }) {
    if (!measureChoice) return false
    const ok = addLine(measureChoice.sectionId, measureChoice.item, null, quote.qty, {
      rate: quote.rate,
      units: quote.units,
      label: quote.label,
      entry: quote.entry,
    })
    if (ok) measure = null
    return ok
  }

  function setQty(sectionId: number, key: string, qty: number) {
    const section = sections.find((entry) => entry.id === sectionId)
    const current = linesFor(sectionId).find((line) => line.key === key)
    if (!section || !current) return false
    if (qty < 1) {
      carts = { ...carts, [sectionId]: linesFor(sectionId).filter((line) => line.key !== key) }
      if (lastKey[sectionId] === key) {
        const next = { ...lastKey }
        delete next[sectionId]
        lastKey = next
      }
      attention = null
      return true
    }
    const item = section.items.find((entry) => entry.id === current.item_id)
    const other = linesFor(sectionId)
      .filter((line) => line.item_id === current.item_id && line.key !== key)
      .reduce((sum, line) => sum + line.qty * line.units, 0)
    if (item && other + qty * current.units > item.stock) {
      attention = { tone: 'warning', message: `Only ${item.stock} left for ${item.name}.` }
      return false
    }
    carts = {
      ...carts,
      [sectionId]: linesFor(sectionId).map((line) => (line.key === key ? { ...line, qty } : line)),
    }
    attention = null
    return true
  }

  async function post(sectionId: number) {
    const lines = linesFor(sectionId)
    if (lines.length === 0) return
    if (!session.user || !snapshot) {
      attention = { tone: 'warning', message: 'Sign in before posting the bill.' }
      return
    }
    const printer = snapshot.printer
    const printThis = printer.enabled && !skipPrint
    postingId = sectionId
    attention = null
    try {
      const customer = (customers[sectionId] ?? '').trim()
      const result = await postBill(
        sectionId,
        lines.map((line) => ({
          item_id: line.item_id,
          variant_id: line.variant_id,
          rate: line.rate,
          qty: line.qty,
          code: line.code,
          entry: line.entry,
        })),
        customer || null,
      )
      snapshot = result.pos
      carts = { ...carts, [sectionId]: [] }
      customers = { ...customers, [sectionId]: '' }
      posted = {
        ...posted,
        [sectionId]: `${result.bill_no} posted · ${formatMoney(result.total, result.pos.currency, result.pos.locale)}`,
      }
      try {
        if (printThis) await printReceipt(result.bill_no)
      } catch (error) {
        attention = {
          tone: 'warning',
          message: error instanceof Error ? error.message : 'The receipt did not print. The sale is saved.',
        }
      }
    } catch (error) {
      attention = {
        tone: 'error',
        message: error instanceof Error ? error.message : 'Could not post the bill.',
      }
    } finally {
      postingId = null
    }
  }

  async function leave() {
    try {
      await logout()
    } catch {
      // The local session still closes.
    }
    signOut()
  }

  async function saveItem(
    itemId: number | null,
    body: CatalogItem,
    picture: { file: File | null; remove: boolean },
  ) {
    savingItem = true
    try {
      const saved = await saveCatalogItem(itemId, body)
      snapshot = saved.pos
      attention = null
      let pictureError: string | null = null
      try {
        if (picture.remove && !picture.file) {
          snapshot = (await removeItemPicture(saved.item_id)).pos
        }
        if (picture.file) {
          snapshot = (await saveItemPicture(saved.item_id, picture.file)).pos
        }
      } catch (error) {
        pictureError = error instanceof Error ? error.message : 'Could not save the picture.'
      }
      const item = snapshot.sections
        .flatMap((section) => section.items)
        .find((entry) => entry.id === saved.item_id)
      if (!item) throw new Error('The saved item did not come back from the till.')
      return { item, pictureError }
    } finally {
      savingItem = false
    }
  }

  function applyPos(pos: PosSnapshot) {
    snapshot = pos
    if (focusId != null && !pos.sections.some((section) => section.id === focusId)) {
      focusId = pos.last_section_id ?? pos.sections[0]?.id ?? null
    }
  }

  $effect(() => {
    document.title = snapshot?.shop_name.trim() || 'Shop'
  })

  function initials(name: string) {
    const words = name.trim().split(/\s+/).filter(Boolean)
    const letters = words.length >= 2 ? `${words[0][0]}${words[1][0]}` : name.slice(0, 2)
    return letters.toUpperCase()
  }

  function scanField() {
    if (focusId == null) return null
    return document.querySelector<HTMLInputElement>(`[data-scan="${focusId}"]`)
  }

  function dialogOpen() {
    return attention != null || measure != null || pending != null
  }

  function scrollbarClick(event: MouseEvent) {
    let element = event.target instanceof Element ? event.target : null
    while (element) {
      if (element instanceof HTMLElement) {
        const rect = element.getBoundingClientRect()
        const x = event.clientX - rect.left
        const y = event.clientY - rect.top
        const vertical = element.scrollHeight > element.clientHeight + 1 && x > element.clientLeft + element.clientWidth
        const horizontal = element.scrollWidth > element.clientWidth + 1 && y > element.clientTop + element.clientHeight
        if (vertical || horizontal) return true
      }
      element = element.parentElement
    }
    return false
  }

  function keepTyping(target: EventTarget | null) {
    return (
      target instanceof Element &&
      !!target.closest(
        'input, textarea, select, label, a[href], [role="dialog"], [role="alertdialog"], .sheet, .attention',
      )
    )
  }

  $effect(() => {
    const restore = () => {
      if (screenOf(location.path) !== 'billing' || focusId == null || dialogOpen()) return
      if (keepTyping(document.activeElement)) return
      scanField()?.focus({ preventScroll: true })
    }
    const onPointerDown = (event: PointerEvent) => {
      if (screen !== 'billing' || focusId == null || dialogOpen()) return
      if (keepTyping(event.target) || scrollbarClick(event)) return
      // Focus would move on this press. Cancelling it keeps the scan field,
      // and the button under the pointer still receives the click.
      event.preventDefault()
      restore()
    }
    const onPointerUp = (event: PointerEvent) => {
      if (screen !== 'billing' || focusId == null || dialogOpen()) return
      if (keepTyping(event.target) || scrollbarClick(event)) return
      // The click focuses the button after pointerdown. Put the scan field
      // back once that click has finished.
      setTimeout(restore, 0)
    }
    window.addEventListener('pointerdown', onPointerDown, true)
    window.addEventListener('pointerup', onPointerUp, true)
    return () => {
      window.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('pointerup', onPointerUp, true)
    }
  })

  $effect(() => {
    const billing = screen === 'billing' && focusId != null
    const blocked = dialogOpen()
    if (!billing || blocked) return
    const id = focusId
    queueMicrotask(() => {
      if (screenOf(location.path) !== 'billing' || dialogOpen()) return
      if (keepTyping(document.activeElement)) return
      document.querySelector<HTMLInputElement>(`[data-scan="${id}"]`)?.focus({ preventScroll: true })
    })
  })

  async function openBilling(event: MouseEvent) {
    followLink(event, '/billing')
    await tick()
    if (focusId != null) document.querySelector<HTMLInputElement>(`[data-scan="${focusId}"]`)?.focus()
  }

  function onKeydown(event: KeyboardEvent) {
    if (attention) return
    if (!snapshot) return
    if (screen !== 'billing' || !session.user) return
    if (event.key === 'F12') {
      event.preventDefault()
      if (focusId != null) void post(focusId)
      return
    }
    const match = /^F([1-8])$/.exec(event.key)
    if (match) {
      const section = sections[Number(match[1]) - 1]
      if (section) {
        event.preventDefault()
        focusSection(section.id)
      }
    }
    if (event.key === 'Escape') {
      const target = event.target
      if (target instanceof HTMLInputElement && !target.hasAttribute('data-scan') && focusId != null) {
        event.preventDefault()
        document.querySelector<HTMLInputElement>(`[data-scan="${focusId}"]`)?.focus({ preventScroll: true })
        return
      }
      if (pending) {
        pending = null
        return
      }
      if (measure) {
        measure = null
        return
      }
      if (focusId != null) carts = { ...carts, [focusId]: [] }
    }
  }
</script>

<svelte:window
  onkeydowncapture={(event) => {
    if (!attention || event.key !== 'Escape') return
    event.preventDefault()
    event.stopPropagation()
    attention = null
  }}
  onkeydown={onKeydown}
/>

<div class="app">
  <header class="topbar">
    <div class="brand">
      <div class="mark">{initials(snapshot?.shop_name ?? 'Shop')}</div>
      <div>
        <h1>
          {snapshot?.shop_name ?? 'Shop'}
        </h1>
      </div>
    </div>
    <nav class="nav" aria-label="Main">
      {#each pluginNav ?? [] as item (item.id)}
        <a
          href={item.href}
          class:active={location.path === item.href || location.path.startsWith(item.href + '/') || (item.href.startsWith('/settings') && screen === 'settings' && item.id === 'settings') || (item.href.startsWith('/reports') && screen === 'reports')}
          onclick={(event) => (item.href === '/billing' ? openBilling(event) : followLink(event, item.href))}
        >{item.label}</a>
      {/each}
    </nav>
    <div class="top-actions">
      {#if session.user}
        <span class="who">{session.user.name}</span>
        <button type="button" class="sections-button" onclick={leave}>Sign out</button>
      {:else}
        <a class="sections-button" href="/login" onclick={(event) => followLink(event, '/login')}>Sign in</a>
      {/if}
    </div>
  </header>

  {#if screen === 'pending' || (!snapshot && !loadFailed)}
    <p class="loading">Opening the till…</p>
  {:else if !snapshot}
    <p class="loading">The till did not open.</p>
  {:else if screen === 'missing'}
    <main class="stage">
      <section class="place">
        <h2>404</h2>
        <p class="hint">That page is not in the till.</p>
        <p class="hint"><a href="/billing" onclick={openBilling}>Back to billing</a></p>
      </section>
    </main>
  {:else if screen === 'settings'}
    <SettingsScreen page={settingsPage(location.path)} {snapshot} saving={savingItem} onSave={saveItem} onChange={applyPos} pages={settingsPages} />
  {:else if screen === 'stock'}
    <main class="stage">
      <StockScreen sections={snapshot.sections} categories={snapshot.categories} currency={snapshot.currency} locale={snapshot.locale} onChange={applyPos} />
    </main>
  {:else if screen === 'reports'}
    <ReportsScreen page={reportsPage(location.path)} currency={snapshot.currency} locale={snapshot.locale} />
  {:else if screen === 'cash'}
    <main class="stage">
      <CashScreen currency={snapshot.currency} locale={snapshot.locale} />
    </main>
  {:else}
    <div class="section-tabs" role="tablist" aria-label="Sections">
      {#each sections as section, index (section.id)}
        <button
          type="button"
          role="tab"
          class:active={active?.id === section.id}
          aria-selected={active?.id === section.id}
          style="--accent: {section.accent}"
          onclick={() => focusSection(section.id)}
        >
          {#if index < 8}<span class="key">F{index + 1}</span>{/if}
          {section.short_name}
          {#if linesFor(section.id).length > 0}
            <span class="count">{linesFor(section.id).length}</span>
          {/if}
        </button>
      {/each}
    </div>
    <main class="stage">
      {#if active}
        <SectionPanel
          section={active}
          categories={snapshot.categories}
          lines={linesFor(active.id)}
          focused={true}
          filter={filters[active.id] ?? 'all'}
          posted={posted[active.id] ?? ''}
          currency={snapshot.currency}
          locale={snapshot.locale}
          onFilter={(filter) => (filters = { ...filters, [active.id]: filter })}
          onScan={(code, qty) => scan(active.id, code, qty)}
          onPick={(item, variant, qty) => pick(active.id, item, variant, qty)}
          onAlert={(message, tone) => (attention = { message, tone })}
          onAdd={(item) => requestAdd(active.id, item)}
          onQty={(key, qty) => setQty(active.id, key, qty)}
          onClear={() => {
            carts = { ...carts, [active.id]: [] }
            customers = { ...customers, [active.id]: '' }
          }}
          onPost={() => post(active.id)}
          posting={postingId === active.id}
          customer={customers[active.id] ?? ''}
          onCustomer={(name) => (customers = { ...customers, [active.id]: name })}
          {skipPrint}
          onSkipPrint={(skip) => (skipPrint = skip)}
        />
      {/if}
    </main>
  {/if}

  {#if screen === 'billing'}
  <footer class="keys">
    <span>F1–F8 switch section</span>
    <span>Enter adds, selects, or applies +qty</span>
    <span>F12 posts the focused bill</span>
    <span>Esc clears it</span>
  </footer>
  {/if}
</div>

{#if attention}
  <AttentionDialog tone={attention.tone} message={attention.message} onClose={() => (attention = null)} />
{/if}

{#if measureChoice && snapshot}
  <MeasureDialog
    item={measureChoice.item}
    currency={snapshot.currency}
    locale={snapshot.locale}
    onClose={() => (measure = null)}
    onAdd={confirmMeasure}
  />
{/if}

{#if pendingChoice && snapshot}
  <VariantDialog
    item={pendingChoice.item}
    available={pendingChoice.available}
    currency={snapshot.currency}
    locale={snapshot.locale}
    onClose={() => (pending = null)}
    onAdd={confirmVariants}
  />
{/if}

