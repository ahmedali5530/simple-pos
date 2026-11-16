<script lang="ts">
  import { tick } from 'svelte'
  import { formatMoney } from '../money'
  import type { CartLine, Category, ItemVariant, PosItem, PosSection } from '../types'

  interface Props {
    section: PosSection
    categories: Category[]
    lines: CartLine[]
    focused: boolean
    filter: number | 'all'
    posted: string
    currency: string
    locale: string
    onFocus: (scan: boolean) => void
    onFilter: (filter: number | 'all') => void
    onScan: (code: string, qty: number) => 'done' | 'rejected' | 'search'
    onPick: (item: PosItem, variant: ItemVariant | null, qty: number) => boolean
    onAlert: (message: string, tone: 'error' | 'warning') => void
    onAdd: (item: PosItem) => void
    onQty: (key: string, qty: number) => boolean
    onClear: () => void
    onPost: () => void
    posting: boolean
  }

  let {
    section,
    categories,
    lines,
    focused,
    filter,
    posted,
    currency,
    locale,
    onFocus,
    onFilter,
    onScan,
    onPick,
    onAlert,
    onAdd,
    onQty,
    onClear,
    onPost,
    posting,
  }: Props = $props()

  let scanCode = $state('')
  let scanQty = $state('1')
  let highlight = $state(0)
  let suggestEl = $state<HTMLDivElement | null>(null)

  interface Hit {
    item: PosItem
    variant: ItemVariant | null
  }

  const hits = $derived.by(() => {
    const query = scanCode.trim().toLowerCase()
    if (!query || query.startsWith('+')) return []
    const found: Hit[] = []
    for (const item of section.items) {
      const itemMatch =
        item.name.toLowerCase().includes(query) || (item.barcode ?? '').toLowerCase().includes(query)
      if (itemMatch) found.push({ item, variant: null })
      for (const variant of item.variants) {
        const variantMatch =
          variant.name.toLowerCase().includes(query) ||
          (variant.barcode ?? '').toLowerCase().includes(query)
        if (variantMatch) found.push({ item, variant })
      }
    }
    return found.slice(0, 8)
  })

  $effect(() => {
    scanCode
    highlight = 0
  })

  const reserved = (itemId: number) =>
    lines.filter((line) => line.item_id === itemId).reduce((sum, line) => sum + line.qty * line.units, 0)

  const minimum = (item: PosItem) =>
    item.variants.length ? Math.min(...item.variants.map((variant) => variant.units)) : 1

  const sellable = (item: PosItem) => item.stock - reserved(item.id) >= minimum(item)

  function priceLabel(item: PosItem) {
    if (item.prompt !== 'none' && item.base_rate) return money(item.base_rate)
    if (item.variants.length === 0) return money(item.rate)
    const rates = item.variants.map((variant) => variant.rate)
    const low = Math.min(...rates)
    const high = Math.max(...rates)
    return low === high ? money(low) : `from ${money(low)}`
  }

  const visibleCategories = $derived(
    categories.filter((category) => section.items.some((item) => item.category_id === category.id)),
  )

  const visibleItems = $derived(
    section.items.filter((item) => filter === 'all' || item.category_id === filter),
  )

  const total = $derived(lines.reduce((sum, line) => sum + line.rate * line.qty, 0))

  function money(amount: number) {
    return formatMoney(amount, currency, locale)
  }

  function clearScan() {
    scanCode = ''
    scanQty = '1'
    highlight = 0
  }

  function choose(hit: Hit, qty: number) {
    if (!onPick(hit.item, hit.variant, qty)) return
    clearScan()
  }

  function submitScan() {
    const code = scanCode.trim()
    const qty = Number(scanQty)
    if (!code) return
    if (!Number.isInteger(qty) || qty < 1) {
      onAlert('Enter a quantity of at least 1.', 'warning')
      return
    }
    const outcome = onScan(code, qty)
    if (outcome === 'rejected') return
    if (outcome === 'search') {
      if (hits.length === 0) {
        onAlert(`No item matches ${code}.`, 'warning')
        return
      }
      choose(hits[Math.min(highlight, hits.length - 1)], qty)
      return
    }
    clearScan()
  }

  async function moveHighlight(next: number) {
    highlight = next
    await tick()
    const list = suggestEl
    const row = list?.children.item(next) as HTMLElement | null
    if (!list || !row) return
    const top = row.offsetTop
    const bottom = top + row.offsetHeight
    if (top < list.scrollTop) list.scrollTop = top
    else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight
  }

  function onScanKey(event: KeyboardEvent) {
    if (event.key === 'ArrowDown' && hits.length > 0) {
      event.preventDefault()
      void moveHighlight((highlight + 1) % hits.length)
      return
    }
    if (event.key === 'ArrowUp' && hits.length > 0) {
      event.preventDefault()
      void moveHighlight((highlight - 1 + hits.length) % hits.length)
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      submitScan()
      return
    }
    if (event.key === 'Escape' && scanCode) {
      event.preventDefault()
      event.stopPropagation()
      scanCode = ''
    }
  }

  function commitQty(line: CartLine, raw: string, input: HTMLInputElement) {
    if (raw === '') return
    if (!/^\d+$/.test(raw)) {
      input.value = String(line.qty)
      return
    }
    const qty = Number(raw)
    if (qty < 1) return
    if (!onQty(line.key, qty)) input.value = String(line.qty)
  }
</script>

<div
  class="panel"
  class:focused
  style="--accent: {section.accent}"
  role="group"
  aria-label={section.name}
  tabindex="0"
  onclick={() => onFocus(false)}
  onkeydown={(event) => {
    if (event.key === 'Enter' && event.target === event.currentTarget) onFocus(true)
  }}
>
  <div class="catalog">
  <div class="scan">
    <label for="scan-{section.id}">Scan, search, or +quantity</label>
    <div class="scan-row">
      <input
        id="scan-{section.id}"
        data-scan={section.id}
        role="combobox"
        aria-expanded={hits.length > 0}
        aria-controls="suggest-{section.id}"
        aria-autocomplete="list"
        placeholder="Name, barcode, or +3"
        bind:value={scanCode}
        onkeydown={onScanKey}
        type="search"
      />
      <input
        class="scan-qty"
        inputmode="numeric"
        aria-label="Quantity to add"
        bind:value={scanQty}
        onkeydown={onScanKey}
      />
      <button type="button" class="add" onclick={submitScan}>Scan / Add</button>
    </div>
    {#if hits.length > 0}
      <div class="suggest" id="suggest-{section.id}" role="listbox" bind:this={suggestEl}>
        {#each hits as hit, index (`${hit.item.id}:${hit.variant?.id ?? 0}`)}
          <button
            type="button"
            role="option"
            class:active={index === highlight}
            aria-selected={index === highlight}
            onpointerdown={(event) => event.preventDefault()}
            onmouseenter={() => (highlight = index)}
            onclick={() => choose(hit, Number(scanQty) || 1)}
          >
            <strong>{hit.variant ? `${hit.item.name} · ${hit.variant.name}` : hit.item.name}</strong>
            <span>
              {hit.variant?.barcode ?? hit.item.barcode ?? ''}
              {money(hit.variant?.rate ?? hit.item.rate)}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <div class="filters" aria-label="Categories in {section.short_name}">
    <button type="button" class="chip" class:active={filter === 'all'} onclick={() => onFilter('all')}>All</button>
    {#each visibleCategories as category (category.id)}
      <button
        type="button"
        class="chip"
        class:active={filter === category.id}
        onclick={() => onFilter(category.id)}
      >{category.name}</button>
    {/each}
  </div>

  <div class="items">
    <div class="grid">
      {#each visibleItems as item (item.id)}
        <button
          type="button"
          class="card"
          class:out={!sellable(item)}
          disabled={!sellable(item)}
          aria-label={
            item.variants.length
              ? `Choose ${item.name}`
              : item.prompt === 'quantity'
                ? `Enter quantity for ${item.name}`
                : item.prompt === 'price'
                  ? `Enter price for ${item.name}`
                  : `Add ${item.name}`
          }
          onclick={() => onAdd(item)}
        >
          <span class="meta">
            {#if item.barcode}#{item.barcode} · {/if}Stock {item.stock}
          </span>
          <strong>{item.name}</strong>
          {#if item.variants.length}
            <span class="options">{item.variants.map((variant) => variant.name).join(' · ')}</span>
          {:else if item.prompt === 'quantity'}
            <span class="options">Asks quantity / weight</span>
          {:else if item.prompt === 'price'}
            <span class="options">Asks price</span>
          {/if}
          <span class="price">{priceLabel(item)}</span>
          <span class="category">{item.category_name}</span>
        </button>
      {/each}
    </div>
  </div>
  </div>

  <footer class="bill">
    <div class="bill-head">
      <h3>Current {section.short_name} bill</h3>
      <button type="button" class="link" onclick={onClear} disabled={lines.length === 0}>Clear Items</button>
    </div>
    <div class="bill-lines">
      {#if lines.length === 0}
        <p class="empty">No items in this bill yet. Scan a barcode or tap an item.</p>
      {:else}
        {#each lines as line (line.key)}
          <div class="line">
            <div class="line-main">
              <strong>{line.name}</strong>
              <span>{money(line.rate * line.qty)}</span>
            </div>
            <div class="line-meta">
              <span class="rate">{money(line.rate)}</span>
              <span class="qty">
                <button type="button" aria-label="Decrease {line.name}" onclick={() => onQty(line.key, line.qty - 1)}>-</button>
                <input
                  inputmode="numeric"
                  aria-label="Quantity for {line.name}"
                  value={line.qty}
                  oninput={(event) => commitQty(line, event.currentTarget.value, event.currentTarget)}
                  onblur={(event) => {
                    const qty = Number(event.currentTarget.value)
                    if (!Number.isInteger(qty) || qty < 1) onQty(line.key, 0)
                  }}
                />
                <button type="button" aria-label="Increase {line.name}" onclick={() => onQty(line.key, line.qty + 1)}>+</button>
                <button type="button" class="remove" aria-label="Remove {line.name}" onclick={() => onQty(line.key, 0)}>×</button>
              </span>
            </div>
          </div>
        {/each}
      {/if}
    </div>
    <div class="total-row">
      <div>
        <span>Total ({section.short_name})</span>
        <strong>{money(total)}</strong>
      </div>
      <button type="button" class="post" disabled={lines.length === 0 || posting} onclick={onPost}>
        {posting ? 'Posting…' : 'Add Bill (One Entry)'}
      </button>
    </div>
    {#if posted}<p class="posted">{posted}</p>{/if}
  </footer>
</div>
