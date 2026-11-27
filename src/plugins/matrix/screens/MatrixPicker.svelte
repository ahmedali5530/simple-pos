<script lang="ts">
  import { tick } from 'svelte'
  import { formatCost, formatMoney } from '../../../lib/money'
  import type { PosItemOptionsContext } from '../../../platform/slots'
  import { Input } from '../../../lib/ui'
  import { getRecipe, type MatrixSku } from '../api'
  import { hasMatrixItemOptions } from '../options'

  let { item, addLine, close, currency, locale }: PosItemOptionsContext = $props()

  const isMatrix = $derived(hasMatrixItemOptions(item.id))

  let skus = $state<MatrixSku[]>([])
  let query = $state('')
  let highlight = $state(0)
  let loading = $state(false)
  let failed = $state(false)
  let searchEl = $state<HTMLInputElement | null>(null)
  const rowEls = new Map<number, HTMLButtonElement>()

  const filtered = $derived.by(() => {
    const needle = query.trim().toLowerCase()
    const rows = skus.filter((sku) => {
      if (!needle) return true
      return `${sku.name} ${sku.barcode ?? ''}`.toLowerCase().includes(needle)
    })
    return rows
  })

  $effect(() => {
    const _ = filtered
    if (highlight >= filtered.length) highlight = Math.max(0, filtered.length - 1)
  })

  $effect(() => {
    const index = highlight
    const rows = filtered
    const sku = rows[index]
    if (!sku) return
    void tick().then(() => {
      rowEls.get(sku.variant_id)?.scrollIntoView({ block: 'nearest' })
    })
  })

  function bindRow(node: HTMLButtonElement, variantId: number) {
    rowEls.set(variantId, node)
    return {
      destroy() {
        if (rowEls.get(variantId) === node) rowEls.delete(variantId)
      },
    }
  }

  async function load() {
    loading = true
    failed = false
    try {
      const body = await getRecipe(item.id)
      skus = body.skus
      highlight = 0
      queueMicrotask(() => searchEl?.focus())
    } catch {
      failed = true
      skus = []
    } finally {
      loading = false
    }
  }

  function pick(sku: MatrixSku) {
    if (sku.stock < 1) return
    const ok = addLine({
      item_id: item.id,
      variant_id: sku.variant_id,
      rate: sku.rate,
      qty: 1,
      source: 'matrix.sku',
      reference: `sku:${sku.variant_id}`,
      label: sku.name,
      details: { stock: sku.stock },
    })
    if (ok) close()
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      if (filtered.length === 0) return
      highlight = (highlight + 1) % filtered.length
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      if (filtered.length === 0) return
      highlight = (highlight - 1 + filtered.length) % filtered.length
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      const sku = filtered[highlight]
      if (sku) pick(sku)
    }
  }

  $effect(() => {
    if (isMatrix) load()
  })
</script>

{#if isMatrix}
  <div class="flex flex-col gap-2" onkeydown={onKey}>
    <p class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">
      Variants
      <span class="ml-1 rounded-full bg-slate-800 px-1.5 py-0.5 text-[10px] text-white">{filtered.length}</span>
    </p>
    <Input
      bind:value={query}
      bind:element={searchEl}
      placeholder="Search name or barcode"
      aria-label="Search variants"
      oninput={() => (highlight = 0)}
    />
    {#if loading}
      <p class="hint">Loading variants…</p>
    {:else if failed}
      <p class="field-error">Could not load variants.</p>
    {:else if filtered.length === 0}
      <p class="hint">No variants match.</p>
    {:else}
      <div class="max-h-72 overflow-auto rounded-xl border border-slate-200" role="listbox">
        {#each filtered as sku, index (sku.variant_id)}
          <button
            type="button"
            role="option"
            class="flex w-full items-center justify-between gap-3 border-b border-slate-100 px-3 py-2.5 text-left last:border-b-0 {index === highlight ? 'bg-slate-100' : 'bg-white'} {sku.stock < 1 ? 'opacity-50' : ''}"
            aria-selected={index === highlight}
            disabled={sku.stock < 1}
            use:bindRow={sku.variant_id}
            onmouseenter={() => (highlight = index)}
            onclick={() => pick(sku)}
          >
            <span>
              <strong class="block text-sm font-extrabold text-slate-900">{sku.name}</strong>
              <span class="text-xs font-semibold text-slate-500">{sku.barcode || 'no barcode'} · {sku.stock} on hand · cost {formatCost(sku.avg_cost, currency, locale)}</span>
            </span>
            <strong class="text-sm font-extrabold text-slate-900">{formatMoney(sku.rate, currency, locale)}</strong>
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/if}
