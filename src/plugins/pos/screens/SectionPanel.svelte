<script lang="ts">
  import { tick } from 'svelte'
  import ItemPicture from '../../../lib/ui/ItemPicture.svelte'
  import VirtualGrid from '../../../lib/ui/VirtualGrid.svelte'
  import { formatMoney } from '../../../lib/money'
  import type { CartLine, Category, ItemVariant, PosItem, PosSection } from '../../../lib/types'
  import { Button } from '../../../lib/ui'
  import Slot from '../../../platform/Slot.svelte'
  import { hasExclusiveItemOptions, hasItemOptions } from '../../../platform/itemOptions'
  import { slotsFor } from '../../../plugins/registry'
  import type { AddPosLine, LineMetadataPatch, PosTender } from '../../../platform/slots'

  interface Props {
    section: PosSection
    categories: Category[]
    lines: CartLine[]
    focused: boolean
    filter: number | 'all'
    currency: string
    locale: string
    onFilter: (filter: number | 'all') => void
    onScan: (code: string, qty: number) => 'done' | 'rejected' | 'search'
    onPick: (item: PosItem, variant: ItemVariant | null, qty: number) => boolean
    onAlert: (message: string, tone: 'error' | 'warning') => void
    onAdd: (item: PosItem) => void
    onQty: (key: string, qty: number) => boolean
    onExternalLine: AddPosLine
    onLinePatch: (key: string, patch: LineMetadataPatch) => void
    onClear: () => void
    onPost: () => void
    posting: boolean
    customer: string
    onCustomer: (name: string) => void
    tenders: PosTender[]
    onTenders: (tenders: PosTender[]) => void
  }

  let {
    section,
    categories,
    lines,
    focused,
    filter,
    currency,
    locale,
    onFilter,
    onScan,
    onPick,
    onAlert,
    onAdd,
    onQty,
    onExternalLine,
    onLinePatch,
    onClear,
    onPost,
    posting,
    customer,
    onCustomer,
    tenders,
    onTenders,
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
    lines
      .filter((line) => line.item_id === itemId && !(line.reference && line.source))
      .reduce((sum, line) => sum + line.qty * line.units, 0)

  const minimum = (item: PosItem) =>
    item.variants.length ? Math.min(...item.variants.map((variant) => variant.units)) : 1

  // An item with plugin options opens a chooser; the chooser decides availability.
  const sellable = (item: PosItem) =>
    hasItemOptions(item.id) || item.stock - reserved(item.id) >= minimum(item)

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
  const hasLineDetails = $derived(slotsFor('pos.line-detail').length > 0)

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
    if (!(event.ctrlKey || event.metaKey) && event.key === 'ArrowDown' && hits.length > 0) {
      event.preventDefault()
      void moveHighlight((highlight + 1) % hits.length)
      return
    }
    if (!(event.ctrlKey || event.metaKey) && event.key === 'ArrowUp' && hits.length > 0) {
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

  let billLinesEl = $state<HTMLDivElement | null>(null)

  function qtyFields() {
    return [
      ...(billLinesEl?.querySelectorAll<HTMLInputElement>('input[data-cart-qty]') ?? []),
    ]
  }

  function focusQty(index: number) {
    const fields = qtyFields()
    if (fields.length === 0) return
    const next = fields[((index % fields.length) + fields.length) % fields.length]
    next.focus()
    next.select()
  }

  function onCartKeydown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey)) return
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    const fields = qtyFields()
    if (fields.length === 0) return
    const active = document.activeElement
    const current = fields.findIndex((field) => field === active)
    event.preventDefault()
    event.stopPropagation()
    if (event.key === 'ArrowDown') focusQty(current < 0 ? 0 : current + 1)
    else focusQty(current < 0 ? fields.length - 1 : current - 1)
  }
</script>

<div
  class="panel"
  class:focused
  style="--accent: {section.accent}"
  role="group"
  aria-label={section.name}
  onkeydown={onCartKeydown}
>
  <div class="catalog">
  <div class="scan">
    <label class="ui-label" for="scan-{section.id}">Scan, search, or +quantity</label>
    <div class="scan-row">
      <input
        id="scan-{section.id}"
        class="ui-input input"
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
        class="ui-input input scan-qty"
        inputmode="numeric"
        aria-label="Quantity to add"
        bind:value={scanQty}
        onkeydown={onScanKey}
      />
      <Button onclick={submitScan}>Scan / Add</Button>
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
    <button
      type="button"
      class="ui-pill"
      aria-pressed={filter === 'all'}
      onclick={() => onFilter('all')}
    >All</button>
    {#each visibleCategories as category (category.id)}
      <button
        type="button"
        class="ui-pill"
        aria-pressed={filter === category.id}
        onclick={() => onFilter(category.id)}
      >{category.name}</button>
    {/each}
  </div>

  <VirtualGrid class="items" items={visibleItems} estimate={128} min={160} gap={10} reset="{section.id}:{filter}" key={(item) => item.id}>
    {#snippet children(item)}
      <button
        type="button"
        class="ui-tile"
        data-out-of-stock={!sellable(item) ? 'true' : undefined}
        disabled={!sellable(item)}
        aria-label={
          item.variants.length || hasItemOptions(item.id)
            ? `Choose ${item.name}`
            : item.prompt === 'quantity'
              ? `Enter quantity for ${item.name}`
              : item.prompt === 'price'
                ? `Enter price for ${item.name}`
                : `Add ${item.name}`
        }
        style:--accent={section.accent}
        onclick={() => onAdd(item)}
      >
        <span class="card-body" class:has-photo={!!item.image_url}>
          <span class="card-name">{item.name}</span>
          {#if item.image_url}
            <ItemPicture class="card-thumb" src={item.image_url} />
          {/if}
          <span class="card-sub">
            <span>{item.category_name}</span>
            {#if item.barcode}<span class="ui-badge">{item.barcode}</span>{/if}
          </span>
          {#if item.variants.length}
            {#if hasExclusiveItemOptions(item.id) || item.variants.length > 4}
              <span class="ui-badge" data-tone="info">{item.variants.length} variants</span>
            {:else}
              <span class="card-options">{item.variants.map((variant) => variant.name).join(' · ')}</span>
            {/if}
          {:else if hasItemOptions(item.id)}
            <span class="ui-badge" data-tone="info">Choose variant</span>
          {:else if item.prompt === 'quantity'}
            <span class="ui-badge" data-tone="info">Asks quantity / weight</span>
          {:else if item.prompt === 'price'}
            <span class="ui-badge" data-tone="warning">Asks price</span>
          {/if}
          <span class="card-foot">
            <span class="price">{priceLabel(item)}</span>
            <span class="card-stock">
              {#if hasExclusiveItemOptions(item.id)}
                {item.variants.length ? `${item.variants.length} variants` : 'Variants'}
              {:else if hasItemOptions(item.id)}
                Variants
              {:else}
                {item.stock} on hand
              {/if}
            </span>
          </span>
        </span>
      </button>
    {/snippet}
  </VirtualGrid>
  </div>

  <footer class="bill">
    <div class="bill-head">
      <h3>Current {section.short_name} bill</h3>
      <Button variant="link" class="text-danger" onclick={onClear} disabled={lines.length === 0}>Clear Items</Button>
    </div>
    <Slot
      name="pos.line-source"
      context={{ section, addLine: onExternalLine, currency, locale, lines }}
    />
    <div class="bill-lines" bind:this={billLinesEl}>
      {#if lines.length === 0}
        <p class="ui-hint empty">No items in this bill yet. Scan a barcode or tap an item.</p>
      {:else}
        <table class="cart-table">
          <thead>
            <tr>
              <th scope="col">Item</th>
              <th scope="col">Qty</th>
              <th scope="col">Amount</th>
              <th scope="col"><span class="sr-only">Remove</span></th>
            </tr>
          </thead>
          <tbody>
            {#each lines as line (line.key)}
              <tr class="cart-row">
                <td class="cart-item">
                  <strong>{line.name}</strong>
                  <span class="cart-rate">{money(line.rate)}</span>
                </td>
                <td class="cart-qty">
                  <input
                    class="cart-qty-input"
                    data-cart-qty=""
                    inputmode="numeric"
                    aria-label="Quantity for {line.name}"
                    readonly={line.source === 'mobile.device'}
                    value={line.qty}
                    oninput={(event) => commitQty(line, event.currentTarget.value, event.currentTarget)}
                    onblur={(event) => {
                      const qty = Number(event.currentTarget.value)
                      if (!Number.isInteger(qty) || qty < 1) onQty(line.key, 0)
                    }}
                    onfocus={(event) => event.currentTarget.select()}
                  />
                </td>
                <td class="cart-amount">{money(line.rate * line.qty)}</td>
                <td class="cart-remove-cell">
                  <button
                    type="button"
                    class="cart-remove"
                    aria-label="Remove {line.name}"
                    onclick={() => onQty(line.key, 0)}
                  >×</button>
                </td>
              </tr>
              {#if hasLineDetails}
                <tr class="cart-detail-row">
                  <td colspan="4">
                    <Slot
                      name="pos.line-detail"
                      context={{
                        line,
                        patch: (patch: LineMetadataPatch) => onLinePatch(line.key, patch),
                        currency,
                        locale,
                      }}
                    />
                  </td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
    <label class="bill-customer ui-field">
      <span class="ui-label">Customer</span>
      <input
        class="ui-input input"
        maxlength="80"
        placeholder="Optional"
        value={customer}
        oninput={(event) => onCustomer(event.currentTarget.value)}
      />
    </label>
    <Slot
      name="pos.tender"
      context={{
        total,
        currency,
        locale,
        tenders,
        setTenders: onTenders,
      }}
    />
    <div class="ui-totals">
      <div data-grand>
        <span>Total ({section.short_name})</span>
        <strong>{money(total)}</strong>
      </div>
      <Button
        class="post-bill"
        size="xl"
        variant="success"
        disabled={lines.length === 0 || posting}
        loading={posting}
        onclick={onPost}
      >
        {posting ? 'Posting…' : 'Add Bill (One Entry)'}
      </Button>
    </div>
  </footer>
</div>
