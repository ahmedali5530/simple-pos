<script lang="ts">
  import { formatMoney } from '../money'
  import type { PosItem, PosSection } from '../types'

  interface Props {
    sections: PosSection[]
    currency: string
    locale: string
    saving: boolean
    onClose: () => void
    onSave: (itemId: number, body: { prompt: 'none' | 'quantity' | 'price'; base_rate: number | null; scale: number }) => void
  }

  let { sections, currency, locale, saving, onClose, onSave }: Props = $props()

  let query = $state('')
  let selectedId = $state<number | null>(null)
  let prompt = $state<'none' | 'quantity' | 'price'>('none')
  let baseRate = $state('')
  let scale = $state('1')
  let formError = $state('')

  const items = $derived(sections.flatMap((section) => section.items.map((item) => ({ item, section }))))
  const visible = $derived.by(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return items
    return items.filter(
      (row) =>
        row.item.name.toLowerCase().includes(needle) ||
        (row.item.barcode ?? '').toLowerCase().includes(needle),
    )
  })
  const selected = $derived(items.find((row) => row.item.id === selectedId) ?? null)

  $effect(() => {
    const item = selected?.item
    if (!item) return
    prompt = item.prompt
    baseRate = item.base_rate == null ? '' : String(item.base_rate)
    scale = String(item.scale || 1)
    formError = ''
  })

  function open(item: PosItem) {
    selectedId = item.id
  }

  function save() {
    if (!selected) return
    if (prompt === 'none') {
      onSave(selected.item.id, { prompt, base_rate: null, scale: 1 })
      return
    }
    if (!/^\d+$/.test(baseRate) || Number(baseRate) < 1) {
      formError = 'Enter a base rate above 0.'
      return
    }
    if (!/^\d+$/.test(scale) || Number(scale) < 1) {
      formError = 'Stock per 1 must be at least 1.'
      return
    }
    formError = ''
    onSave(selected.item.id, { prompt, base_rate: Number(baseRate), scale: Number(scale) })
  }

  function promptLabel(item: PosItem) {
    if (item.prompt === 'quantity') return 'Quantity / weight'
    if (item.prompt === 'price') return 'Price'
    return 'Off'
  }
</script>

<div
  class="sheet-backdrop"
  role="presentation"
  onclick={onClose}
  onkeydown={(event) => {
    if (event.key === 'Escape') onClose()
  }}
>
  <div
    class="sheet item-setup"
    role="dialog"
    aria-labelledby="item-setup-title"
    tabindex="-1"
    onclick={(event) => event.stopPropagation()}
    onkeydown={(event) => {
      event.stopPropagation()
      if (event.key === 'Escape') {
        if (selected) selectedId = null
        else onClose()
      }
    }}
  >
    <h2 id="item-setup-title">{selected ? selected.item.name : 'Item setup'}</h2>
    {#if selected}
      <p class="hint">{selected.section.short_name}. Shelf rate {formatMoney(selected.item.rate, currency, locale)}.</p>
      {#if selected.item.variants.length}
        <p class="field-error">This item sells by its options, so asking stays off.</p>
      {:else}
        <div class="prompt-choices" role="radiogroup" aria-label="What the cashier enters">
          <button type="button" class:active={prompt === 'none'} aria-checked={prompt === 'none'} role="radio" onclick={() => (prompt = 'none')}>Off</button>
          <button type="button" class:active={prompt === 'quantity'} aria-checked={prompt === 'quantity'} role="radio" onclick={() => (prompt = 'quantity')}>Quantity / weight</button>
          <button type="button" class:active={prompt === 'price'} aria-checked={prompt === 'price'} role="radio" onclick={() => (prompt = 'price')}>Price</button>
        </div>
        {#if prompt !== 'none'}
          <label>
            Base rate
            <input inputmode="numeric" bind:value={baseRate} />
          </label>
          <label>
            Stock per 1
            <input inputmode="numeric" bind:value={scale} />
          </label>
          <p class="hint">
            {#if prompt === 'quantity'}
              Amount is the base rate times what they type. One of that value removes this many from stock.
            {:else}
              They type the price. Stock removed is that price times stock-per-1, divided by the base rate.
            {/if}
          </p>
        {/if}
        {#if formError}<p class="field-error">{formError}</p>{/if}
      {/if}
      <div class="sheet-actions">
        <button type="button" class="ghost" onclick={() => (selectedId = null)}>Back</button>
        <button type="button" class="save" disabled={saving || selected.item.variants.length > 0} onclick={save}>
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    {:else}
      <p class="hint">Choose whether a tap asks for a quantity, a weight, or a price, and set the base rate used for that sum.</p>
      <input class="setup-search" placeholder="Find an item" bind:value={query} />
      <div class="setup-list">
        {#each visible as row (row.item.id)}
          <button type="button" onclick={() => open(row.item)}>
            <strong>{row.item.name}</strong>
            <span>{row.section.short_name} · {promptLabel(row.item)}</span>
          </button>
        {/each}
      </div>
      <div class="sheet-actions">
        <button type="button" class="ghost" onclick={onClose}>Close</button>
      </div>
    {/if}
  </div>
</div>
