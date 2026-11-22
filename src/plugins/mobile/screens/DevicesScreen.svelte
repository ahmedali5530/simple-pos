<script lang="ts">
  import { onMount } from 'svelte'
  import { Button, Field, Input, Select } from '../../../lib/ui'
  import { toastError, toastSaved } from '../../../lib/toast'
  import {
    addDevice,
    listDevices,
    listModels,
    type MobileDevice,
    type MobileModel,
  } from '../api'

  let models = $state<MobileModel[]>([])
  let devices = $state<MobileDevice[]>([])
  let loading = $state(true)
  let failed = $state(false)
  let saving = $state(false)
  let search = $state('')
  let status = $state('')
  let modelItemId = $state('')
  let imei = $state('')
  let serial = $state('')
  let months = $state('12')
  let searchTimer: ReturnType<typeof setTimeout> | undefined

  const selectedModel = $derived(
    models.find((model) => String(model.item_id) === modelItemId) ?? null,
  )
  const inStock = $derived(devices.filter((device) => device.status === 'in_stock').length)

  $effect(() => {
    if (selectedModel) months = String(selectedModel.warranty_months)
  })

  async function load(showSkeleton = devices.length === 0) {
    if (showSkeleton) loading = true
    failed = false
    try {
      const [deviceBody, modelBody] = await Promise.all([
        listDevices({ status, search }),
        listModels(),
      ])
      devices = deviceBody.devices
      models = modelBody.models
    } catch (error) {
      failed = true
      toastError(error, 'Could not load devices.')
    } finally {
      loading = false
    }
  }

  function scheduleLoad() {
    clearTimeout(searchTimer)
    searchTimer = setTimeout(() => load(false), 250)
  }

  function setStatus(value: string) {
    status = value
    load(false)
  }

  async function submit() {
    if (!selectedModel) {
      toastError(null, 'Choose a phone model.')
      return
    }
    if (!imei.trim()) {
      toastError(null, 'Enter the IMEI.')
      return
    }
    const value = Number(months)
    if (!Number.isInteger(value) || value < 1 || value > 120) {
      toastError(null, 'Warranty months must be between 1 and 120.')
      return
    }
    saving = true
    try {
      await addDevice({
        item_id: selectedModel.item_id,
        imei: imei.trim(),
        serial: serial.trim(),
        warranty_months: value,
      })
      imei = ''
      serial = ''
      await load(false)
      toastSaved('Device added.')
    } catch (error) {
      toastError(error, 'Could not add the device.')
    } finally {
      saving = false
    }
  }

  function deviceName(device: MobileDevice) {
    return [device.brand, device.model].filter(Boolean).join(' ') || `Item #${device.item_id}`
  }

  function statusClass(status: string) {
    return status === 'in_stock'
      ? 'bg-emerald-100 text-emerald-700'
      : 'bg-slate-100 text-slate-600'
  }

  onMount(() => {
    load()
    return () => clearTimeout(searchTimer)
  })
</script>

<section class="flex w-full min-w-0 flex-1 flex-col gap-4 pb-5">
  <header class="flex flex-wrap items-end justify-between gap-4">
    <div class="min-w-0">
      <span class="flex items-center gap-2 text-[11px] font-extrabold uppercase tracking-[0.09em] text-teal-700">
        <span class="h-0.5 w-5 rounded-full bg-current"></span>
        Mobile
      </span>
      <h1 class="mt-1.5 text-3xl font-extrabold tracking-tight text-ink">Devices</h1>
      <p class="mt-2 text-[13px] font-semibold text-slate-500">
        Register each IMEI, then pick it from the till when it is sold.
      </p>
    </div>
    <div class="rounded-xl border border-slate-200 bg-white px-4 py-2.5 text-right">
      <span class="block text-[10px] font-extrabold uppercase tracking-[0.06em] text-slate-500">In stock</span>
      <strong class="mt-0.5 block text-2xl leading-none tracking-tight text-ink">{inStock}</strong>
    </div>
  </header>

  <section class="rounded-2xl border border-slate-200 bg-white p-4">
    <h2 class="text-sm font-extrabold tracking-tight text-ink">Add a device</h2>
    {#if models.length === 0}
      <p class="mt-2 text-[12px] font-semibold text-slate-500">
        Add mobile brand and model to a catalog item first: Items → open the phone → Mobile details.
      </p>
    {:else}
      <div class="mt-3 grid gap-2.5 sm:grid-cols-2 lg:grid-cols-5 [&_.field]:mt-0">
        <Field label="Phone model">
          <Select bind:value={modelItemId}>
            <option value="">Choose model</option>
            {#each models as model (model.item_id)}
              <option value={String(model.item_id)}>
                {model.brand} {model.model} (#{model.item_id})
              </option>
            {/each}
          </Select>
        </Field>
        <Field label="IMEI"><Input bind:value={imei} placeholder="356…" /></Field>
        <Field label="Serial"><Input bind:value={serial} placeholder="Optional" /></Field>
        <Field label="Warranty months"><Input inputmode="numeric" bind:value={months} /></Field>
        <div class="flex items-end">
          <Button class="w-full" onclick={submit} disabled={saving}>
            {saving ? 'Saving…' : 'Add device'}
          </Button>
        </div>
      </div>
    {/if}
  </section>

  <section class="flex min-w-0 flex-wrap items-center gap-2">
    <div class="relative flex min-w-[220px] max-w-[420px] flex-1 items-center">
      <Input
        bind:value={search}
        oninput={scheduleLoad}
        placeholder="Search IMEI, serial or model"
        aria-label="Search devices"
      />
    </div>
    <Select value={status} onchange={(event) => setStatus(event.currentTarget.value)} aria-label="Filter devices">
      <option value="">All devices</option>
      <option value="in_stock">In stock</option>
      <option value="sold">Sold</option>
    </Select>
  </section>

  {#if failed}
    <section class="rounded-2xl border border-dashed border-rose-200 bg-rose-50/50 px-6 py-12 text-center">
      <h2 class="text-lg font-extrabold tracking-tight text-ink">The device list did not open</h2>
      <p class="mt-1 text-[13px] font-semibold text-slate-500">Check the till connection, then try again.</p>
      <Button class="mt-4" variant="secondary" onclick={() => load(true)}>Try again</Button>
    </section>
  {:else if loading && devices.length === 0}
    <p class="py-10 text-center text-sm font-semibold text-slate-400">Loading devices…</p>
  {:else if devices.length === 0}
    <section class="rounded-2xl border border-dashed border-slate-300 bg-white px-6 py-14 text-center">
      <h2 class="text-lg font-extrabold tracking-tight text-ink">No devices yet</h2>
      <p class="mt-1 text-[13px] font-semibold text-slate-500">
        Add a phone model to an item, then register its first IMEI above.
      </p>
    </section>
  {:else}
    <div class="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] items-start gap-3">
      {#each devices as device (device.id)}
        <article class="flex flex-col gap-2 rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0">
              <strong class="block truncate text-sm text-ink">{deviceName(device)}</strong>
              <span class="block truncate text-[11px] font-semibold text-slate-500">{device.imei}</span>
            </div>
            <span class="inline-flex h-6 items-center rounded-full px-2 text-[10px] font-extrabold {statusClass(device.status)}">
              {device.status === 'in_stock' ? 'In stock' : 'Sold'}
            </span>
          </div>
          {#if device.status === 'sold'}
            <p class="text-[11px] font-semibold text-slate-500">
              {device.sold_bill_no ? `Sold on ${device.sold_bill_no}` : 'Sold'}
              {device.customer_name ? ` · ${device.customer_name}` : ''}
            </p>
            <p class="text-[11px] font-bold text-slate-600">
              {device.ends_on ? `Warranty until ${device.ends_on}` : 'Warranty starting…'}
            </p>
          {:else}
            <p class="text-[11px] font-semibold text-slate-500">
              {device.warranty_months} month warranty starts on sale
            </p>
          {/if}
          {#if device.serial}
            <p class="text-[10px] font-bold text-slate-400">Serial {device.serial}</p>
          {/if}
        </article>
      {/each}
    </div>
  {/if}
</section>
