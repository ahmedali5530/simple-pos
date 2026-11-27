<script lang="ts">
  import { formatCost, formatMoney, nextAverageCost } from '../../../lib/money'
  import { toast, toastError, toastSaved } from '../../../lib/toast'
  import { Button, Input } from '../../../lib/ui'
  import type { InventoryStockFormContext } from '../../../platform/slots'
  import { adjustSku, getRecipe, type MatrixSku } from '../api'
  import { hasMatrixItemOptions } from '../options'

  let { item, direction, kind, currency, locale, onAdjusted }: InventoryStockFormContext = $props()

  const isMatrix = $derived(hasMatrixItemOptions(item.id))

  let skus = $state<MatrixSku[]>([])
  let skuId = $state('')
  let skuQuery = $state('')
  let skuOpen = $state(false)
  let highlight = $state(0)
  let qty = $state('1')
  let amount = $state('')
  let note = $state('')
  let saving = $state(false)
  let loading = $state(false)

  const selected = $derived(skus.find((sku) => String(sku.variant_id) === skuId) ?? null)
  const hits = $derived.by(() => {
    const needle = skuQuery.trim().toLowerCase()
    const rows = needle
      ? skus.filter((sku) => `${sku.name} ${sku.barcode ?? ''}`.toLowerCase().includes(needle))
      : skus
    return rows.slice(0, 8)
  })
  const unitPrice = $derived.by(() => {
    if (kind !== 'purchase' || !/^\d+$/.test(qty) || Number(qty) < 1) return null
    if (!/^\d+$/.test(amount.trim())) return null
    return Number(amount) / Number(qty)
  })
  const projected = $derived.by(() => {
    if (!selected || unitPrice == null || kind !== 'purchase') return null
    return nextAverageCost(selected.stock, selected.avg_cost, Number(qty), Number(amount))
  })

  async function load() {
    loading = true
    try {
      skus = (await getRecipe(item.id)).skus
    } catch {
      skus = []
      toast.error('Could not load matrix SKUs.')
    } finally {
      loading = false
    }
  }

  $effect(() => {
    if (isMatrix) {
      skuId = ''
      skuQuery = ''
      void load()
    }
  })

  function pickSku(sku: MatrixSku) {
    skuId = String(sku.variant_id)
    skuQuery = `${sku.name}${sku.barcode ? ` · ${sku.barcode}` : ''}`
    skuOpen = false
  }

  function onSkuKey(event: KeyboardEvent) {
    if (!skuOpen && (event.key === 'ArrowDown' || event.key === 'Enter')) {
      skuOpen = true
      return
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      highlight = Math.min(highlight + 1, Math.max(0, hits.length - 1))
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      highlight = Math.max(highlight - 1, 0)
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      const hit = hits[highlight]
      if (hit) pickSku(hit)
    }
  }

  async function submit() {
    if (!selected) {
      toast.error('Select a variant.')
      return
    }
    if (!/^\d+$/.test(qty.trim()) || Number(qty) < 1) {
      toast.error('Enter a quantity of at least 1.')
      return
    }
    saving = true
    try {
      const purchasing = kind === 'purchase' && unitPrice != null
      const label =
        kind === 'purchase'
          ? 'Purchase'
          : kind === 'found'
            ? 'Found'
            : kind === 'return'
              ? 'Customer return'
              : kind === 'transfer-in'
                ? 'Transfer in'
                : kind === 'waste'
                  ? 'Waste'
                  : kind === 'lost'
                    ? 'Lost'
                    : kind === 'damaged'
                      ? 'Damaged'
                      : kind === 'expired'
                        ? 'Expired'
                        : kind === 'transfer-out'
                          ? 'Transfer out'
                          : kind === 'staff'
                            ? 'Staff use'
                            : 'Count correction'
      const detail = note.trim()
      await adjustSku(selected.variant_id, {
        direction,
        qty: Number(qty),
        update_cost: purchasing,
        total: purchasing ? Number(amount) : null,
        note: detail ? `${label} · ${detail}` : label,
      })
      await load()
      await onAdjusted()
      qty = '1'
      amount = ''
      note = ''
      toastSaved('Variant stock updated.')
    } catch (error) {
      toastError(error, 'Could not update the variant stock.')
    } finally {
      saving = false
    }
  }
</script>

{#if isMatrix}
  <div class="mt-2">
    <p class="hint">This item uses matrix variants. Adjust stock on one SKU at a time.</p>
    {#if loading}
      <p class="hint">Loading variants…</p>
    {:else}
      <div class="stock-item">
        <label>
          Select variant
          <Input
            role="combobox"
            aria-expanded={skuOpen}
            aria-autocomplete="list"
            placeholder="Search variant name or barcode"
            bind:value={skuQuery}
            onfocus={() => (skuOpen = true)}
            oninput={() => {
              skuOpen = true
              skuId = ''
              highlight = 0
            }}
            onkeydown={onSkuKey}
            onblur={() => (skuOpen = false)}
          />
        </label>
        {#if skuOpen}
          <div class="stock-suggest" role="listbox">
            {#if hits.length === 0}
              <p class="hint">No variant matches.</p>
            {:else}
              {#each hits as sku, index (sku.variant_id)}
                <button
                  type="button"
                  role="option"
                  class:active={index === highlight}
                  aria-selected={index === highlight}
                  onpointerdown={(event) => event.preventDefault()}
                  onmouseenter={() => (highlight = index)}
                  onclick={() => pickSku(sku)}
                >
                  <strong>{sku.name}</strong>
                  <span>{sku.barcode || 'no barcode'} · {sku.stock} on hand · {formatMoney(sku.rate, currency, locale)}</span>
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>

      <div class="stock-pair">
        <label>
          Quantity
          <Input inputmode="numeric" bind:value={qty} />
        </label>
        <label>
          Total amount ({currency})
          <Input inputmode="numeric" bind:value={amount} placeholder="Optional" />
        </label>
      </div>
      <div class="stock-pair">
        <label>
          Unit price
          <Input readonly tabindex="-1" value={unitPrice == null ? '' : formatMoney(unitPrice, currency, locale)} />
        </label>
        <label>
          Reason / Note
          <Input bind:value={note} placeholder="e.g. Supplier / Damage" />
        </label>
      </div>

      {#if selected}
        <p class="hint">
          Average cost {formatCost(selected.avg_cost, currency, locale)}.
          {selected.stock} on hand is worth {formatCost(selected.value, currency, locale)}.
        </p>
      {/if}
      {#if projected != null}
        <p class="hint">This purchase sets the average cost to {formatCost(projected, currency, locale)}.</p>
      {:else if kind === 'purchase' && selected}
        <p class="hint">Enter the total you paid and this purchase will update the average cost.</p>
      {/if}
      <Button size="sm" class="mt-4 w-full" disabled={saving} onclick={submit}>
        {saving ? 'Saving…' : 'Update Variant Stock'}
      </Button>
    {/if}
  </div>
{/if}
