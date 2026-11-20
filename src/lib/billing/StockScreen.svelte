<script lang="ts">
  import { onMount, tick } from 'svelte'
  import VirtualTable from './VirtualTable.svelte'
  import { adjustStock, downloadWorkbook, importMovementsWorkbook, loadStockSummary, saveCatalogItem } from '../api'
  import { formatCost, formatMoney, nextAverageCost } from '../money'
  import type { Category, PosSection, PosSnapshot, StockBalance } from '../types'

  interface Props {
    sections: PosSection[]
    categories: Category[]
    currency: string
    locale: string
    onChange: (pos: PosSnapshot) => void
  }

  let { sections, categories, currency, locale, onChange }: Props = $props()

  const kinds = [
    { id: 'purchase', label: 'Purchase', direction: 'in' },
    { id: 'found', label: 'Found', direction: 'in' },
    { id: 'return', label: 'Customer return', direction: 'in' },
    { id: 'transfer-in', label: 'Transfer in', direction: 'in' },
    { id: 'waste', label: 'Waste', direction: 'out' },
    { id: 'lost', label: 'Lost', direction: 'out' },
    { id: 'damaged', label: 'Damaged', direction: 'out' },
    { id: 'expired', label: 'Expired', direction: 'out' },
    { id: 'transfer-out', label: 'Transfer out', direction: 'out' },
    { id: 'staff', label: 'Staff use', direction: 'out' },
    { id: 'correction', label: 'Count correction', direction: 'both' },
  ] as const

  let direction = $state<'in' | 'out'>('in')
  let kind = $state<(typeof kinds)[number]['id']>('purchase')
  let itemId = $state('')
  let itemQuery = $state('')
  let itemOpen = $state(false)
  let highlight = $state(0)
  let itemListEl = $state<HTMLDivElement | null>(null)
  let qty = $state('1')
  let amount = $state('')
  let note = $state('')
  let headId = $state('all')
  let query = $state('')
  let balances = $state<StockBalance[]>([])
  let formError = $state('')
  let itemError = $state('')
  let saving = $state(false)
  let savingItem = $state(false)
  let newSection = $state('')
  let newBarcode = $state('')
  let newName = $state('')
  let newRate = $state('')
  let newOpening = $state('0')
  let sheetBusy = $state(false)
  let sheetNote = $state('')

  const catalog = $derived(sections.flatMap((section) => section.items.map((item) => ({ item, section }))))
  const selected = $derived(catalog.find((row) => String(row.item.id) === itemId) ?? null)
  const itemHits = $derived.by(() => {
    const needle = itemQuery.trim().toLowerCase()
    const rows = needle
      ? catalog.filter((row) => {
          const codes = [row.item.barcode ?? '', ...row.item.variants.map((variant) => `${variant.name} ${variant.barcode ?? ''}`)]
          return row.item.name.toLowerCase().includes(needle) || codes.some((code) => code.toLowerCase().includes(needle))
        })
      : catalog
    return rows.slice(0, 8)
  })
  const visible = $derived(
    balances.filter((row) => {
      if (headId !== 'all' && String(row.section_id) !== headId) return false
      const needle = query.trim().toLowerCase()
      if (!needle) return true
      return row.name.toLowerCase().includes(needle) || (row.barcode ?? '').toLowerCase().includes(needle) || row.section_name.toLowerCase().includes(needle)
    }),
  )
  const onHand = $derived(visible.reduce((sum, row) => sum + row.stock, 0))
  const stockWorth = $derived(visible.reduce((sum, row) => sum + row.value, 0))

  $effect(() => {
    if (!newSection && sections[0]) newSection = String(sections[0].id)
  })

  onMount(() => {
    loadStockSummary()
      .then((rows) => {
        balances = rows
      })
      .catch(() => {
        formError = 'The stock summary did not open.'
      })
  })

  function stockOf(id: number, fallback: number) {
    return balances.find((row) => row.id === id)?.stock ?? fallback
  }

  function chooseKind(id: string) {
    const entry = kinds.find((item) => item.id === id)
    if (!entry) return
    kind = entry.id
    if (entry.direction !== 'both') direction = entry.direction
  }

  function chooseDirection(next: 'in' | 'out') {
    direction = next
    const current = kinds.find((item) => item.id === kind)
    if (current && current.direction !== 'both' && current.direction !== next) kind = next === 'in' ? 'purchase' : 'waste'
  }

  const unitPrice = $derived.by(() => {
    if (!/^\d+$/.test(amount.trim()) || !/^\d+$/.test(qty.trim())) return null
    const count = Number(qty)
    if (count < 1) return null
    return Number(amount) / count
  })
  const projected = $derived.by(() => {
    if (kind !== 'purchase' || !selected || unitPrice == null) return null
    const count = Number(qty)
    if (!Number.isInteger(count) || count < 1) return null
    return nextAverageCost(stockOf(selected.item.id, selected.item.stock), selected.item.cost, count, Number(amount))
  })

  function money(value: number) {
    try {
      return new Intl.NumberFormat(locale, {
        style: 'currency',
        currency,
        minimumFractionDigits: Number.isInteger(value) ? 0 : 2,
        maximumFractionDigits: 2,
      }).format(value)
    } catch {
      return `${Math.round(value * 100) / 100} ${currency}`
    }
  }

  function pickItem(row: (typeof catalog)[number]) {
    itemId = String(row.item.id)
    itemQuery = row.item.name
    itemOpen = false
  }

  async function moveHighlight(next: number) {
    highlight = next
    await tick()
    const list = itemListEl
    const row = list?.querySelectorAll('button').item(next)
    if (!list || !row) return
    const top = row.offsetTop
    const bottom = top + row.offsetHeight
    if (top < list.scrollTop) list.scrollTop = top
    else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight
  }

  function onItemKey(event: KeyboardEvent) {
    if (event.key === 'ArrowDown' && itemHits.length > 0) {
      event.preventDefault()
      const next = itemOpen ? (highlight + 1) % itemHits.length : 0
      itemOpen = true
      void moveHighlight(next)
      return
    }
    if (event.key === 'ArrowUp' && itemHits.length > 0) {
      event.preventDefault()
      const next = itemOpen ? (highlight - 1 + itemHits.length) % itemHits.length : 0
      itemOpen = true
      void moveHighlight(next)
      return
    }
    if (event.key === 'Enter' && itemOpen && itemHits[highlight]) {
      event.preventDefault()
      pickItem(itemHits[highlight])
    } else if (event.key === 'Escape') {
      itemOpen = false
    }
  }

  async function refresh(pos: PosSnapshot) {
    onChange(pos)
    balances = await loadStockSummary()
  }

  async function exportWorkbook() {
    formError = ''
    sheetNote = ''
    sheetBusy = true
    try {
      await downloadWorkbook('/api/stock/export', 'stock-movements.xlsx')
      sheetNote = 'The movement workbook downloaded.'
    } catch (error) {
      formError = error instanceof Error ? error.message : 'The workbook did not download.'
    } finally {
      sheetBusy = false
    }
  }

  async function importWorkbook(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    formError = ''
    sheetNote = ''
    sheetBusy = true
    try {
      const result = await importMovementsWorkbook(file)
      await refresh(result.pos)
      sheetNote = result.applied
        ? `Saved ${result.applied} ${result.applied === 1 ? 'movement' : 'movements'}.`
        : 'No new movements were in the workbook. Rows that already have an id stay as history.'
    } catch (error) {
      formError = error instanceof Error ? error.message : 'The workbook was not saved.'
    } finally {
      sheetBusy = false
    }
  }

  async function updateBalance() {
    if (!selected) {
      formError = 'Select an item.'
      return
    }
    if (!/^\d+$/.test(qty.trim()) || Number(qty) < 1) {
      formError = 'Enter a quantity of at least 1.'
      return
    }
    formError = ''
    saving = true
    try {
      const label = kinds.find((item) => item.id === kind)?.label ?? 'Count correction'
      const purchasing = kind === 'purchase' && unitPrice != null
      const detail = note.trim()
      const saved = await adjustStock({
        item_id: selected.item.id,
        qty: Number(qty),
        direction,
        note: detail ? `${label} · ${detail}` : label,
        update_cost: purchasing,
        total: purchasing ? Number(amount) : null,
      })
      await refresh(saved.pos)
      qty = '1'
      amount = ''
      note = ''
    } catch (error) {
      formError = error instanceof Error ? error.message : 'Could not update the stock.'
    } finally {
      saving = false
    }
  }

  async function saveItem() {
    const sectionId = Number(newSection)
    const category = categories.find((entry) => entry.name === 'Shop') ?? categories[0]
    if (!sectionId) {
      itemError = 'Choose a head.'
      return
    }
    if (!newName.trim()) {
      itemError = 'Enter an item name.'
      return
    }
    if (!/^\d+$/.test(newRate.trim()) || Number(newRate) < 1) {
      itemError = 'Enter a sale rate of at least 1.'
      return
    }
    if (!/^\d+$/.test(newOpening.trim())) {
      itemError = 'Opening stock is a whole number.'
      return
    }
    if (!category) {
      itemError = 'Add a category in Settings before saving an item.'
      return
    }
    itemError = ''
    savingItem = true
    try {
      const saved = await saveCatalogItem(null, {
        name: newName.trim(),
        barcode: newBarcode.trim() || null,
        rate: Number(newRate),
        cost: 0,
        section_id: sectionId,
        category_id: category.id,
        prompt: 'none',
        base_rate: null,
        scale: 1,
        variants: [],
      })
      let pos = saved.pos
      const opening = Number(newOpening)
      if (opening > 0) {
        pos = (
          await adjustStock({
            item_id: saved.item_id,
            qty: opening,
            direction: 'in',
            note: 'Opening stock',
          })
        ).pos
      }
      await refresh(pos)
      itemId = String(saved.item_id)
      itemQuery = newName.trim()
      newBarcode = ''
      newName = ''
      newRate = ''
      newOpening = '0'
    } catch (error) {
      itemError = error instanceof Error ? error.message : 'Could not save the item.'
    } finally {
      savingItem = false
    }
  }
</script>

<div class="stock-board">
  <aside class="stock-form">
    <h2>Record Stock In / Out</h2>
    <p class="hint">Update physical stock or record wastage</p>
    <p class="stock-label">Movement Type</p>
    <div class="move-type" role="radiogroup" aria-label="Movement type">
      <button type="button" role="radio" class:on={direction === 'in'} aria-checked={direction === 'in'} onclick={() => chooseDirection('in')}>Stock IN (Add)</button>
      <button type="button" role="radio" class="out" class:on={direction === 'out'} aria-checked={direction === 'out'} onclick={() => chooseDirection('out')}>Stock OUT (Deduct)</button>
    </div>
    <label>
      Type
      <select value={kind} onchange={(event) => chooseKind(event.currentTarget.value)}>
        {#each kinds as entry (entry.id)}
          <option value={entry.id}>{entry.label}</option>
        {/each}
      </select>
    </label>
    <div class="stock-item">
      <label>
        Select item
        <input
          role="combobox"
          aria-expanded={itemOpen}
          aria-autocomplete="list"
          aria-controls="stock-item-list"
          placeholder="Search name or barcode"
          bind:value={itemQuery}
          onfocus={() => {
            itemOpen = true
            highlight = 0
          }}
          oninput={() => {
            itemOpen = true
            itemId = ''
            highlight = 0
          }}
          onkeydown={onItemKey}
          onblur={() => (itemOpen = false)}
        />
      </label>
      {#if itemOpen}
        <div class="stock-suggest" id="stock-item-list" role="listbox" bind:this={itemListEl}>
          {#if itemHits.length === 0}
            <p class="hint">No item matches that search.</p>
          {:else}
            {#each itemHits as row, index (row.item.id)}
              <button
                type="button"
                role="option"
                class:active={index === highlight}
                aria-selected={index === highlight}
                onpointerdown={(event) => event.preventDefault()}
                onmouseenter={() => (highlight = index)}
                onclick={() => pickItem(row)}
              >
                <strong>{row.item.name}</strong>
                <span>{row.section.short_name} · {row.item.barcode || 'no barcode'} · {stockOf(row.item.id, row.item.stock)} on hand</span>
              </button>
            {/each}
          {/if}
        </div>
      {/if}
    </div>
    <div class="stock-pair">
      <label>
        Quantity
        <input inputmode="numeric" bind:value={qty} />
      </label>
      <label>
        Total amount ({currency})
        <input inputmode="numeric" bind:value={amount} placeholder="Optional" />
      </label>
    </div>
    <div class="stock-pair">
      <label>
        Unit price
        <input readonly tabindex="-1" value={unitPrice == null ? '' : money(unitPrice)} />
      </label>
      <label>
        Reason / Note
        <input bind:value={note} placeholder="e.g. Supplier / Damage" />
      </label>
    </div>
    {#if selected}
      <p class="hint">
        Average cost {formatCost(selected.item.cost, currency, locale)}.
        {stockOf(selected.item.id, selected.item.stock)} on hand is worth {formatCost(stockOf(selected.item.id, selected.item.stock) * selected.item.cost, currency, locale)}.
      </p>
    {/if}
    {#if projected != null}
      <p class="hint">This purchase sets the average cost to {formatCost(projected, currency, locale)}.</p>
    {:else if kind === 'purchase' && selected}
      <p class="hint">Enter the total you paid and this purchase will update the average cost.</p>
    {/if}
    {#if formError}<p class="field-error">{formError}</p>{/if}
    <button type="button" class="stock-update" disabled={saving} onclick={updateBalance}>{saving ? 'Saving…' : 'Update Stock Balance'}</button>
    <p class="stock-label">Excel</p>
    <div class="mt-2.5 grid grid-cols-2 gap-2">
      <button type="button" class="inline-flex h-[46px] items-center justify-center rounded-xl border border-slate-300 bg-white px-3 text-center text-sm font-extrabold leading-none whitespace-nowrap text-slate-900" disabled={sheetBusy} onclick={exportWorkbook}>Export movements</button>
      <label class="mt-0 inline-flex h-[46px] cursor-pointer items-center justify-center rounded-xl border border-slate-300 bg-white px-3 text-center text-sm font-extrabold leading-none whitespace-nowrap text-slate-900">
        Import movements
        <input class="hidden" type="file" accept=".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" onchange={importWorkbook} />
      </label>
    </div>
    <p class="hint">Add rows with a blank id. Direction in adds stock and out deducts it.</p>
    {#if sheetNote}<p class="mt-2 text-[12px] font-bold text-emerald-700">{sheetNote}</p>{/if}

    <div class="new-item">
      <p class="stock-label add">Add new item to inventory</p>
      <div class="stock-pair">
        <label>
          Business Head
          <select bind:value={newSection}>
            {#each sections as section (section.id)}
              <option value={String(section.id)}>{section.short_name}</option>
            {/each}
          </select>
        </label>
        <label>
          Barcode (Optional)
          <input bind:value={newBarcode} placeholder="e.g. 1005" />
        </label>
      </div>
      <label>
        Item Name
        <input bind:value={newName} placeholder="e.g. Cold Drink 500ml / Chicken Roll" />
      </label>
      <div class="stock-pair">
        <label>
          Sale Rate ({currency})
          <input inputmode="numeric" bind:value={newRate} placeholder="100" />
        </label>
        <label>
          Opening Stock
          <input inputmode="numeric" bind:value={newOpening} />
        </label>
      </div>
      {#if itemError}<p class="field-error">{itemError}</p>{/if}
      <button type="button" class="stock-create" disabled={savingItem} onclick={saveItem}>{savingItem ? 'Saving…' : 'Save New Item'}</button>
    </div>
  </aside>

  <section class="stock-summary">
    <div class="summary-head">
      <div>
        <h2>Live Stock Summary (In / Out / Balance)</h2>
        <p class="hint">{onHand} on hand · {formatCost(stockWorth, currency, locale)} at average cost</p>
      </div>
      <label class="head-filter">
        <select bind:value={headId}>
          <option value="all">All Heads</option>
          {#each sections as section (section.id)}
            <option value={String(section.id)}>{section.short_name}</option>
          {/each}
        </select>
      </label>
    </div>
    <input class="mt-3 h-11 w-full rounded-xl border border-slate-300 bg-white px-3 text-sm font-bold text-slate-900" placeholder="Search items, barcodes, or heads" bind:value={query} />
    <VirtualTable
      class="stock-table-wrap"
      tableClass="stock-table"
      items={visible}
      estimate={49}
      columns={9}
      reset="{headId}:{query}"
      key={(row) => row.id}
    >
      {#snippet header()}
        <tr>
          <th>Barcode</th>
          <th>Item name</th>
          <th>Head</th>
          <th>Rate</th>
          <th>Cost</th>
          <th>Stock in</th>
          <th>Stock out</th>
          <th>Current stock</th>
          <th>Value</th>
        </tr>
      {/snippet}
      {#snippet row(row)}
        <tr class:picked={String(row.id) === itemId}>
          <td>{row.barcode ?? ''}</td>
          <td class="item-cell">{row.name}</td>
          <td><span class="head-pill" style:--accent={row.accent}>{row.section_name}</span></td>
          <td>{formatMoney(row.rate, currency, locale)}</td>
          <td>{formatCost(row.cost, currency, locale)}</td>
          <td class="qty-in">+{row.stock_in}</td>
          <td class="qty-out">{row.stock_out > 0 ? `-${row.stock_out}` : '0'}</td>
          <td class="now-stock">{row.stock}</td>
          <td>{formatCost(row.value, currency, locale)}</td>
        </tr>
      {/snippet}
      {#snippet footer()}
        {#if visible.length === 0}
          <p class="hint">Nothing matches that search.</p>
        {/if}
      {/snippet}
    </VirtualTable>
  </section>
</div>
