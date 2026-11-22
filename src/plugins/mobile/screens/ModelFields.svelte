<script lang="ts">
  import type { CatalogItemFormContext } from '../../../platform/slots'
  import { Field, Input } from '../../../lib/ui'
  import { clearModel, getModel, saveModel } from '../api'

  let { itemId, registerExtraSave }: CatalogItemFormContext = $props()

  let brand = $state('')
  let model = $state('')
  let warrantyMonths = $state('12')
  // Plain variable: the load effect must not re-run when it records the item it loaded.
  let loadedFor: number | null = null
  let loading = $state(false)
  let loadFailed = $state(false)

  function reset() {
    brand = ''
    model = ''
    warrantyMonths = '12'
    loadedFor = null
    loading = false
    loadFailed = false
  }

  $effect(() => {
    const id = itemId
    if (id == null) {
      reset()
      return
    }
    if (id === loadedFor) return
    loadedFor = id
    let cancelled = false
    loading = true
    loadFailed = false
    getModel(id)
      .then((body) => {
        if (cancelled) return
        brand = body.model?.brand ?? ''
        model = body.model?.model ?? ''
        warrantyMonths = String(body.model?.warranty_months ?? 12)
      })
      .catch(() => {
        if (cancelled) return
        loadFailed = true
        brand = ''
        model = ''
        warrantyMonths = '12'
      })
      .finally(() => {
        if (!cancelled) loading = false
      })
    return () => {
      cancelled = true
    }
  })

  $effect(() => {
    return registerExtraSave('mobile', async (id) => {
      if (loading) {
        throw new Error('Wait for the mobile details to load, then save again.')
      }
      if (itemId != null && loadFailed) {
        throw new Error('Could not load the mobile details. Reopen the item and try again.')
      }
      const months = Number(warrantyMonths)
      if (!Number.isInteger(months) || months < 1 || months > 120) {
        throw new Error('Warranty months must be between 1 and 120.')
      }
      const nextBrand = brand.trim()
      const nextModel = model.trim()
      if (!nextBrand && !nextModel) {
        await clearModel(id)
        return
      }
      await saveModel(id, {
        brand: nextBrand,
        model: nextModel,
        warranty_months: months,
      })
    })
  })
</script>

<div class="mt-4 flex flex-col gap-2.5 rounded-xl border border-slate-200 bg-slate-50 p-3">
  <div>
    <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Mobile details</h3>
    <p class="mt-0.5 text-[11px] font-semibold text-slate-400">
      Brand, model and default warranty for this phone. Devices are added under Mobile → Devices.
    </p>
  </div>
  <div class="grid gap-2.5 sm:grid-cols-3 [&_.field]:mt-0">
    <Field label="Brand"><Input bind:value={brand} placeholder="Apple" /></Field>
    <Field label="Model"><Input bind:value={model} placeholder="iPhone 13" /></Field>
    <Field label="Warranty months"><Input inputmode="numeric" bind:value={warrantyMonths} placeholder="12" /></Field>
  </div>
  {#if loading}
    <p class="text-[11px] font-semibold text-slate-400">Loading mobile details…</p>
  {/if}
</div>
