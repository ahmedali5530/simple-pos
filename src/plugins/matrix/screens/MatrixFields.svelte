<script lang="ts">
  import type { CatalogItemFormContext } from '../../../platform/slots'
  import { formatMoney } from '../../../lib/money'
  import { Button, Field, Input, Switch } from '../../../lib/ui'
  import { session } from '../../../lib/session.svelte'
  import {
    generateVariants,
    getRecipe,
    saveRecipe,
    updateSku,
    type MatrixGroup,
    type MatrixSku,
  } from '../api'
  import { invalidateMatrixItemOptions } from '../options'

  let { item, itemId, editing, registerExtraSave }: CatalogItemFormContext = $props()

  let enabled = $state(false)
  let barcodePrefix = $state('')
  let groups = $state<MatrixGroup[]>([])
  let skus = $state<MatrixSku[]>([])
  let loadedFor: number | null = null
  let loading = $state(false)
  let loadFailed = $state(false)
  let generating = $state(false)
  let genError = $state('')
  let genNote = $state('')

  const currency = $derived(session.shop?.currency ?? 'PKR')
  const locale = $derived(session.shop?.locale ?? 'en-PK')
  const resolvedItemId = $derived(
    itemId ?? item?.id ?? (typeof editing === 'number' ? editing : null),
  )
  const groupsReady = $derived(
    groups.some(
      (group) =>
        group.name.trim().length > 0 &&
        group.values.some((value) => value.value.trim().length > 0),
    ),
  )

  function blankGroup(): MatrixGroup {
    return { name: '', values: [{ value: '' }] }
  }

  function reset() {
    enabled = false
    barcodePrefix = ''
    groups = []
    skus = []
    loadedFor = null
    loading = false
    loadFailed = false
    generating = false
    genError = ''
    genNote = ''
  }

  function applyRecipe(body: Awaited<ReturnType<typeof getRecipe>>) {
    enabled = body.enabled
    barcodePrefix = body.barcode_prefix
    groups =
      body.groups.length > 0
        ? body.groups.map((group) => ({
            name: group.name,
            sort_order: group.sort_order,
            values:
              group.values.length > 0
                ? group.values.map((value) => ({ value: value.value, sort_order: value.sort_order }))
                : [{ value: '' }],
          }))
        : []
    skus = body.skus
  }

  $effect(() => {
    const id = resolvedItemId
    if (id == null) {
      if (loadedFor != null) reset()
      return
    }
    if (id === loadedFor) return
    loadedFor = id
    let cancelled = false
    loading = true
    loadFailed = false
    getRecipe(id)
      .then((body) => {
        if (cancelled) return
        applyRecipe(body)
      })
      .catch(() => {
        if (cancelled) return
        loadFailed = true
        enabled = false
        barcodePrefix = ''
        groups = []
        skus = []
      })
      .finally(() => {
        if (!cancelled) loading = false
      })
    return () => {
      cancelled = true
    }
  })

  $effect(() => {
    return registerExtraSave('matrix', async (id) => {
      if (loading) {
        throw new Error('Wait for the matrix details to load, then save again.')
      }
      if (itemId != null && loadFailed) {
        throw new Error('Could not load the matrix details. Reopen the item and try again.')
      }
      const cleaned = groups
        .map((group, index) => ({
          name: group.name.trim(),
          sort_order: index,
          values: group.values
            .map((value, valueIndex) => ({
              value: value.value.trim(),
              sort_order: valueIndex,
            }))
            .filter((value) => value.value),
        }))
        .filter((group) => group.name && group.values.length > 0)

      if (!enabled && cleaned.length === 0 && !barcodePrefix.trim()) {
        await saveRecipe(id, { enabled: false, barcode_prefix: '', groups: [] })
        invalidateMatrixItemOptions()
        skus = []
        return
      }

      // Persist the recipe, then expand SKUs so catalog item-save cannot leave
      // the item without its matrix variants.
      let body = await saveRecipe(id, {
        enabled: enabled || cleaned.length > 0,
        barcode_prefix: barcodePrefix.trim(),
        groups: cleaned,
      })
      if ((enabled || cleaned.length > 0) && cleaned.length > 0) {
        body = await generateVariants(id)
      }
      applyRecipe(body)
      invalidateMatrixItemOptions()
    })
  })

  async function onGenerate() {
    const id = resolvedItemId
    if (id == null) {
      genError = 'Save the item first, then generate variants.'
      genNote = ''
      return
    }
    if (!groupsReady) {
      genError = 'Add at least one group with a value.'
      genNote = ''
      return
    }
    generating = true
    genError = ''
    genNote = ''
    try {
      const cleaned = groups
        .map((group, index) => ({
          name: group.name.trim(),
          sort_order: index,
          values: group.values
            .map((value, valueIndex) => ({
              value: value.value.trim(),
              sort_order: valueIndex,
            }))
            .filter((value) => value.value),
        }))
        .filter((group) => group.name && group.values.length > 0)
      await saveRecipe(id, {
        enabled: true,
        barcode_prefix: barcodePrefix.trim(),
        groups: cleaned,
      })
      enabled = true
      const body = await generateVariants(id)
      applyRecipe(body)
      invalidateMatrixItemOptions()
      genNote = `Generated ${body.skus.length} variants.`
    } catch (error) {
      genError = error instanceof Error ? error.message : 'Could not generate variants.'
    } finally {
      generating = false
    }
  }

  async function saveSkuRow(sku: MatrixSku, field: 'barcode' | 'rate' | 'stock' | 'avg_cost', raw: string) {
    try {
      if (field === 'barcode') {
        const body = await updateSku(sku.variant_id, { barcode: raw.trim() || null })
        skus = skus.map((row) => (row.variant_id === sku.variant_id ? body.sku : row))
        return
      }
      if (field === 'rate') {
        const rate = Number(raw)
        if (!Number.isInteger(rate) || rate < 1) throw new Error('Rate must be at least 1.')
        const body = await updateSku(sku.variant_id, { rate })
        skus = skus.map((row) => (row.variant_id === sku.variant_id ? body.sku : row))
        return
      }
      if (field === 'stock') {
        const stock = Number(raw)
        if (!Number.isInteger(stock) || stock < 0) throw new Error('Stock must be 0 or more.')
        const body = await updateSku(sku.variant_id, { stock })
        skus = skus.map((row) => (row.variant_id === sku.variant_id ? body.sku : row))
        return
      }
      const major = Number(raw)
      if (!Number.isFinite(major) || major < 0) throw new Error('Cost must be 0 or more.')
      const body = await updateSku(sku.variant_id, { avg_cost: Math.round(major * 100) })
      skus = skus.map((row) => (row.variant_id === sku.variant_id ? body.sku : row))
    } catch (error) {
      genError = error instanceof Error ? error.message : 'Could not update that SKU.'
    }
  }
</script>

<div class="mt-4 rounded-xl border border-slate-200 p-3">
  <div class="mb-3 flex items-center justify-between gap-3">
    <div>
      <p class="text-sm font-extrabold text-slate-900">Variant matrix</p>
      <p class="text-xs font-semibold text-slate-500">Dynamic groups with per-SKU barcode, price, cost, and stock.</p>
    </div>
    <Switch bind:checked={enabled} label="Enable matrix" />
  </div>

  {#if loading}
    <p class="hint">Loading matrix…</p>
  {:else if loadFailed}
    <p class="field-error">Could not load the matrix details.</p>
  {:else}
    <Field label="Barcode prefix">
      <Input bind:value={barcodePrefix} placeholder="e.g. SH" />
    </Field>

    <div class="mt-3 flex flex-col gap-3">
      {#each groups as group, groupIndex (groupIndex)}
        <div class="rounded-lg border border-slate-200 p-2">
          <div class="mb-2 flex items-center gap-2">
            <Input class="flex-1" bind:value={group.name} placeholder="Group (Color, Size…)" />
            <Button
              variant="secondary"
              size="sm"
              onclick={() => {
                groups = groups.filter((_, index) => index !== groupIndex)
              }}
            >Remove</Button>
          </div>
          <div class="flex flex-wrap gap-2">
            {#each group.values as value, valueIndex (valueIndex)}
              <Input
                class="w-28"
                bind:value={value.value}
                placeholder="Value"
              />
            {/each}
            <Button
              variant="secondary"
              size="sm"
              onclick={() => {
                groups = groups.map((entry, index) =>
                  index === groupIndex
                    ? { ...entry, values: [...entry.values, { value: '' }] }
                    : entry,
                )
              }}
            >Add value</Button>
          </div>
        </div>
      {/each}
      <Button variant="secondary" size="sm" onclick={() => (groups = [...groups, blankGroup()])}>
        Add group
      </Button>
    </div>

    <div class="mt-3 flex flex-wrap items-center gap-2">
      <Button disabled={generating || !groupsReady} onclick={onGenerate}>
        {generating ? 'Generating…' : 'Generate variants'}
      </Button>
      {#if resolvedItemId == null}
        <p class="hint">Save the item once, then generate variants.</p>
      {/if}
      {#if genNote}<p class="text-xs font-bold text-emerald-700">{genNote}</p>{/if}
      {#if genError}<p class="field-error">{genError}</p>{/if}
    </div>

    {#if skus.length > 0}
      <div class="mt-3 overflow-auto">
        <table class="bill-log w-full min-w-[640px]">
          <thead>
            <tr>
              <th>Variant</th>
              <th>Barcode</th>
              <th>Price</th>
              <th>Cost</th>
              <th>Stock</th>
            </tr>
          </thead>
          <tbody>
            {#each skus as sku (sku.variant_id)}
              <tr>
                <td class="bill-no">{sku.name}</td>
                <td>
                  <input
                    class="h-9 w-28 rounded-lg border border-slate-300 px-2 text-sm font-bold"
                    value={sku.barcode ?? ''}
                    onchange={(event) => saveSkuRow(sku, 'barcode', event.currentTarget.value)}
                  />
                </td>
                <td>
                  <input
                    class="h-9 w-20 rounded-lg border border-slate-300 px-2 text-sm font-bold"
                    value={String(sku.rate)}
                    onchange={(event) => saveSkuRow(sku, 'rate', event.currentTarget.value)}
                  />
                  <span class="ml-1 text-[11px] text-slate-500">{formatMoney(sku.rate, currency, locale)}</span>
                </td>
                <td>
                  <input
                    class="h-9 w-24 rounded-lg border border-slate-300 px-2 text-sm font-bold"
                    value={(sku.avg_cost / 100).toFixed(2)}
                    onchange={(event) => saveSkuRow(sku, 'avg_cost', event.currentTarget.value)}
                  />
                </td>
                <td>
                  <input
                    class="h-9 w-16 rounded-lg border border-slate-300 px-2 text-sm font-bold"
                    value={String(sku.stock)}
                    onchange={(event) => saveSkuRow(sku, 'stock', event.currentTarget.value)}
                  />
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="hint mt-2">Cost shown as major units. Stock can also be adjusted on Stock In / Out.</p>
      </div>
    {/if}
  {/if}
</div>
