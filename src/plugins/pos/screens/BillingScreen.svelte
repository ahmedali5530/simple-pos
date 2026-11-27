<script lang="ts">
  import { tick } from 'svelte'
  import {
    currentSession,
    loadPos,
    postBill,
    rememberSection,
  } from '../../../lib/api'
  import { embeddedSale, findEmbedded, parseEmbedded } from '../../../lib/barcode'
  import { formatMoney } from '../../../lib/money'
  import { printReceipt } from '../../../lib/print'
  import { pluginEnabled } from '../../../platform/enabled.svelte'
  import { hasItemOptions, prepareItemOptions } from '../../../platform/itemOptions'
  import { location, screenOf } from '../../../lib/route.svelte'
  import { session } from '../../../lib/session.svelte'
  import type { CartLine, ItemVariant, PosItem, PosSnapshot } from '../../../lib/types'
  import type { LineMetadataPatch, PosLineDraft, PosTender } from '../../../platform/slots'
  import { Switch } from '../../../lib/ui'
  import AttentionDialog from './AttentionDialog.svelte'
  import MeasureDialog from './MeasureDialog.svelte'
  import SectionPanel from './SectionPanel.svelte'
  import VariantDialog from './VariantDialog.svelte'

  const screen = 'billing'

  let snapshot = $state<PosSnapshot | null>(null)
  let loadFailed = $state(false)
  let attention = $state<{ tone: 'error' | 'warning'; message: string } | null>(null)
  let carts = $state<Record<number, CartLine[]>>({})
  let filters = $state<Record<number, number | 'all'>>({})
  let focusId = $state<number | null>(null)
  let measure = $state<{ sectionId: number; itemId: number } | null>(null)
  let postingId = $state<number | null>(null)
  let customers = $state<Record<number, string>>({})
  let tendersBySection = $state<Record<number, PosTender[]>>({})
  let skipPrint = $state(false)
  let now = $state(new Date())
  let pending = $state<{ sectionId: number; itemId: number } | null>(null)
  let lastKey = $state<Record<number, string>>({})
  let focusSave: Promise<void> = Promise.resolve()

  const sections = $derived(snapshot?.sections ?? [])
  const active = $derived(sections.find((section) => section.id === focusId) ?? sections[0] ?? null)

  $effect(() => {
    if (session.user) currentSession().catch(() => {})
    Promise.all([loadPos(), prepareItemOptions()])
      .then(async ([pos]) => {
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
    return {
      sectionId: section.id,
      section,
      item,
      available: item.stock - reserved(section.id, item.id),
    }
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

  function lineKey(
    itemId: number,
    variantId: number | null,
    rate: number,
    units: number,
    source = '',
    reference = '',
  ) {
    const base = `${itemId}:${variantId ?? 0}:${rate}:${units}`
    return source || reference ? `${base}|${source}|${reference}` : base
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
    const key = lineKey(item.id, variant?.id ?? null, rate, units)
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
    attention = null
    return true
  }

  /** Add a line contributed by a plugin (device, job, etc.). The metadata is kept on the line. */
  function addExternalLine(sectionId: number, draft: PosLineDraft) {
    focusId = sectionId
    const section = sections.find((entry) => entry.id === sectionId)
    const item = section?.items.find((entry) => entry.id === draft.item_id)
    if (!item) {
      attention = { tone: 'warning', message: 'That item is not in this section.' }
      return false
    }
    const variant =
      draft.variant_id != null
        ? (item.variants.find((entry) => entry.id === draft.variant_id) ?? null)
        : null
    if (draft.variant_id != null && !variant) {
      attention = { tone: 'warning', message: 'That option is not in the catalog.' }
      return false
    }
    const rate = draft.rate ?? variant?.rate ?? item.rate
    const qty = draft.qty ?? 1
    const units = variant?.units ?? 1
    if (!Number.isInteger(rate) || rate < 1 || !Number.isInteger(qty) || qty < 1) {
      attention = { tone: 'error', message: 'That line has an invalid price or quantity.' }
      return false
    }
    const source = draft.source.trim()
    if (!source) {
      attention = { tone: 'error', message: 'That line is missing its source.' }
      return false
    }
    const reference = draft.reference?.trim() ?? ''
    // A referenced line names an external unit the plugin has already checked
    // (an IMEI, a job). Do not block it on this snapshot's catalog quantity,
    // which may be stale; the server still enforces stock when the bill posts.
    if (
      !reference &&
      item.stock - reserved(sectionId, item.id) < units * qty
    ) {
      attention = { tone: 'warning', message: `Only ${item.stock} left for ${item.name}.` }
      return false
    }
    const lines = linesFor(sectionId)
    if (
      reference &&
      lines.some((line) => line.reference === reference && (line.source ?? '') === source)
    ) {
      attention = { tone: 'warning', message: 'That line is already on this bill.' }
      return false
    }
    const key = lineKey(item.id, variant?.id ?? null, rate, units, source, reference)
    let name = variant ? `${item.name} · ${variant.name}` : item.name
    if (draft.label) name = `${name} · ${draft.label}`
    const existing = lines.find((line) => line.key === key)
    carts = {
      ...carts,
      [sectionId]: existing
        ? lines.map((line) => (line.key === key ? { ...line, qty: line.qty + qty } : line))
        : [
            ...lines,
            {
              key,
              item_id: item.id,
              variant_id: variant?.id ?? null,
              name,
              rate,
              units,
              qty,
              code: null,
              entry: null,
              source,
              reference,
              details: draft.details ?? {},
            },
          ],
    }
    lastKey = { ...lastKey, [sectionId]: key }
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
    if (item.variants.length > 0 || hasItemOptions(item.id)) {
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
      if (item.variants.length > 0 || hasItemOptions(item.id)) {
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
      if (found.variant == null && hasItemOptions(found.item.id)) {
        focusId = sectionId
        pending = { sectionId, itemId: found.item.id }
        attention = null
        return 'done'
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
    if (item.variants.length > 0 || hasItemOptions(item.id)) {
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
    if (current.source === 'mobile.device' && qty > 1) {
      attention = {
        tone: 'warning',
        message: 'A device line is one phone. Remove it and add another device instead.',
      }
      return false
    }
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

  function patchLine(sectionId: number, key: string, patch: LineMetadataPatch) {
    carts = {
      ...carts,
      [sectionId]: linesFor(sectionId).map((line) =>
        line.key === key ? { ...line, ...patch } : line,
      ),
    }
  }

  async function post(sectionId: number) {
    const lines = linesFor(sectionId)
    if (lines.length === 0) return
    if (!session.user || !snapshot) {
      attention = { tone: 'warning', message: 'Sign in before posting the bill.' }
      return
    }
    const printThis = pluginEnabled('printing') && !skipPrint
    postingId = sectionId
    attention = null
    try {
      const customer = (customers[sectionId] ?? '').trim()
      const tenders = tendersBySection[sectionId] ?? []
      const result = await postBill(
        sectionId,
        lines.map((line) => ({
          item_id: line.item_id,
          variant_id: line.variant_id,
          rate: line.rate,
          qty: line.qty,
          code: line.code,
          entry: line.entry,
          source: line.source ?? null,
          reference: line.reference ?? null,
          details: line.details ?? null,
        })),
        customer || null,
        tenders.map((tender) => ({
          method: tender.method,
          amount: tender.amount,
          reference: tender.reference ?? null,
          details: tender.details ?? null,
        })),
      )
      snapshot = result.pos
      carts = { ...carts, [sectionId]: [] }
      customers = { ...customers, [sectionId]: '' }
      tendersBySection = { ...tendersBySection, [sectionId]: [] }
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

  $effect(() => {
    document.title = snapshot?.shop_name.trim() || 'Shop'
  })

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

{#if !snapshot && !loadFailed}
  <p class="loading">Opening the till…</p>
{:else if !snapshot}
  <p class="loading">The till did not open.</p>
{:else}
    <div class="section-tabs" data-density="touch" role="tablist" aria-label="Sections">
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
    <main class="stage" data-density="touch">
      {#if active}
        <SectionPanel
          section={active}
          categories={snapshot.categories}
          lines={linesFor(active.id)}
          focused={true}
          filter={filters[active.id] ?? 'all'}
          currency={snapshot.currency}
          locale={snapshot.locale}
          onFilter={(filter) => (filters = { ...filters, [active.id]: filter })}
          onScan={(code, qty) => scan(active.id, code, qty)}
          onPick={(item, variant, qty) => pick(active.id, item, variant, qty)}
          onAlert={(message, tone) => (attention = { message, tone })}
          onAdd={(item) => requestAdd(active.id, item)}
          onQty={(key, qty) => setQty(active.id, key, qty)}
          onExternalLine={(draft) => addExternalLine(active.id, draft)}
          onLinePatch={(key, patch) => patchLine(active.id, key, patch)}
          onClear={() => {
            carts = { ...carts, [active.id]: [] }
            customers = { ...customers, [active.id]: '' }
            tendersBySection = { ...tendersBySection, [active.id]: [] }
          }}
          onPost={() => post(active.id)}
          posting={postingId === active.id}
          customer={customers[active.id] ?? ''}
          onCustomer={(name) => (customers = { ...customers, [active.id]: name })}
          tenders={tendersBySection[active.id] ?? []}
          onTenders={(tenders) => (tendersBySection = { ...tendersBySection, [active.id]: tenders })}
        />
      {/if}
    </main>
  <footer class="keys" data-density="touch">
    <div class="keys-hints">
      <span>F1–F8 switch section</span>
      <span>Enter adds, selects, or applies +qty</span>
      <span>Ctrl ↑/↓ moves cart qty</span>
      <span>F12 posts the focused bill</span>
      <span>Esc clears it</span>
    </div>
    {#if pluginEnabled('printing')}
      <Switch
        class="keys-skip"
        label="Skip receipt"
        checked={skipPrint}
        onchange={(skip) => (skipPrint = skip)}
      />
    {/if}
  </footer>
{/if}

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
    section={pendingChoice.section}
    lines={linesFor(pendingChoice.sectionId)}
    available={pendingChoice.available}
    currency={snapshot.currency}
    locale={snapshot.locale}
    onClose={() => (pending = null)}
    onAdd={confirmVariants}
    onExternalLine={(draft) => addExternalLine(pendingChoice.sectionId, draft)}
  />
{/if}

