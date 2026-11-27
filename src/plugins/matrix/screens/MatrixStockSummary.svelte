<script lang="ts">
  import { formatCost, formatMoney } from '../../../lib/money'
  import type { InventoryStockSummaryContext } from '../../../platform/slots'
  import { loadSkuSummary, type MatrixSku } from '../api'

  let { currency, locale, headId, query, refreshKey }: InventoryStockSummaryContext = $props()

  let skus = $state<MatrixSku[]>([])
  let failed = $state(false)

  const visible = $derived.by(() => {
    const needle = query.trim().toLowerCase()
    return skus.filter((sku) => {
      if (headId !== 'all' && String(sku.section_id) !== headId) return false
      if (!needle) return true
      return (
        sku.name.toLowerCase().includes(needle) ||
        sku.parent_name.toLowerCase().includes(needle) ||
        (sku.barcode ?? '').toLowerCase().includes(needle) ||
        sku.section_name.toLowerCase().includes(needle)
      )
    })
  })

  async function load() {
    try {
      skus = (await loadSkuSummary()).skus
      failed = false
    } catch {
      skus = []
      failed = true
    }
  }

  $effect(() => {
    const _ = refreshKey
    void load()
  })
</script>

{#if failed}
  <p class="hint mt-3">Matrix SKU summary did not load.</p>
{:else if visible.length > 0}
  <div class="stock-matrix-panel">
    <h3 class="m-0 text-sm font-extrabold text-slate-900">Matrix variant stock</h3>
    <p class="hint">Per-SKU balances · {visible.length} shown</p>
    <div class="stock-table-wrap">
      <table class="stock-table">
        <thead>
          <tr>
            <th>Barcode</th>
            <th>Item · Variant</th>
            <th>Head</th>
            <th>Rate</th>
            <th>Cost</th>
            <th>Current stock</th>
            <th>Value</th>
          </tr>
        </thead>
        <tbody>
          {#each visible as sku (sku.variant_id)}
            <tr>
              <td>{sku.barcode ?? ''}</td>
              <td class="item-cell">{sku.parent_name} · {sku.name}</td>
              <td><span class="head-pill" style:--accent={sku.accent}>{sku.section_name}</span></td>
              <td>{formatMoney(sku.rate, currency, locale)}</td>
              <td>{formatCost(sku.avg_cost, currency, locale)}</td>
              <td class="now-stock">{sku.stock}</td>
              <td>{formatCost(sku.value, currency, locale)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}
