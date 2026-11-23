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
  let inStock = $state(0)
  let sold = $state(0)
  let loading = $state(true)
  let failed = $state(false)
  let saving = $state(false)
  let adding = $state(false)
  let selected = $state<MobileDevice | null>(null)
  let search = $state('')
  let status = $state('')
  let typeItemId = $state('')
  let modelItemId = $state('')
  let imei = $state('')
  let serial = $state('')
  let months = $state('12')
  let searchTimer: ReturnType<typeof setTimeout> | undefined
  let loadSeq = 0

  // Shared column track so the header and body rows line up.
  const rowGrid =
    'grid-cols-[minmax(180px,1.5fr)_minmax(130px,1.1fr)_minmax(90px,0.7fr)_96px_minmax(130px,1fr)_minmax(150px,1.2fr)_22px]'

  const selectedModel = $derived(
    models.find((model) => String(model.item_id) === modelItemId) ?? null,
  )

  const hasFilters = $derived(Boolean(search.trim() || status || typeItemId))

  $effect(() => {
    if (selectedModel) months = String(selectedModel.warranty_months)
  })

  function localToday() {
    const now = new Date()
    const month = String(now.getMonth() + 1).padStart(2, '0')
    const day = String(now.getDate()).padStart(2, '0')
    return `${now.getFullYear()}-${month}-${day}`
  }

  async function load(showSkeleton = devices.length === 0) {
    const seq = ++loadSeq
    if (showSkeleton) loading = true
    failed = false
    try {
      const [deviceBody, modelBody] = await Promise.all([
        listDevices({
          status,
          search: search.trim(),
          item_id: typeItemId ? Number(typeItemId) : undefined,
        }),
        listModels(),
      ])
      if (seq !== loadSeq) return
      devices = deviceBody.devices
      inStock = deviceBody.in_stock
      sold = deviceBody.sold
      models = modelBody.models
    } catch (error) {
      if (seq !== loadSeq) return
      failed = true
      toastError(error, 'Could not load devices.')
    } finally {
      if (seq === loadSeq) loading = false
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

  function setType(value: string) {
    typeItemId = value
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
    const nextImei = imei.trim()
    saving = true
    try {
      // The API also enforces this, but checking here gives the answer before
      // the form clears or the list refreshes.
      const existing = await listDevices({ search: nextImei })
      if (
        existing.devices.some(
          (device) => device.imei.trim().toLowerCase() === nextImei.toLowerCase(),
        )
      ) {
        toastError(null, 'That IMEI is already registered.')
        return
      }
      await addDevice({
        item_id: selectedModel.item_id,
        imei: nextImei,
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

  function typeName(model: MobileModel) {
    return [model.brand, model.model].filter(Boolean).join(' ') || `Item #${model.item_id}`
  }

  function statusClass(value: string) {
    return value === 'in_stock'
      ? 'bg-emerald-100 text-emerald-700'
      : 'bg-slate-100 text-slate-600'
  }

  function warrantyState(device: MobileDevice) {
    if (!device.ends_on) return 'none'
    return device.ends_on >= localToday() ? 'active' : 'expired'
  }

  function warrantyTitle(device: MobileDevice) {
    if (device.status !== 'sold') return `${device.warranty_months} months`
    if (!device.ends_on) return 'Syncing…'
    return warrantyState(device) === 'active' ? `Until ${device.ends_on}` : `Ended ${device.ends_on}`
  }

  function warrantySub(device: MobileDevice) {
    if (device.status !== 'sold') return 'Starts on sale'
    if (!device.starts_on) return 'Sale syncing'
    return `From ${device.starts_on}`
  }

  function warrantyLabel(device: MobileDevice) {
    if (device.status !== 'sold') return 'This device has not been sold yet.'
    const state = warrantyState(device)
    if (state === 'none') return 'Warranty record is still syncing from the sale.'
    if (state === 'active') return `In warranty until ${device.ends_on}.`
    return `Out of warranty since ${device.ends_on}.`
  }

  function soldOn(device: MobileDevice) {
    return device.sold_at ? device.sold_at.slice(0, 10) : '—'
  }

  function closeDevice() {
    selected = null
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && selected) closeDevice()
  }

  onMount(() => {
    load()
    return () => clearTimeout(searchTimer)
  })
</script>

<svelte:window onkeydown={onKeydown} />

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
    <div class="flex flex-wrap items-center justify-end gap-2">
      <div class="flex overflow-hidden rounded-xl border border-slate-200 bg-white" aria-label="Device totals">
        <div class="px-3.5 py-2 text-right">
          <span class="block text-[10px] font-extrabold uppercase tracking-[0.06em] text-slate-500">In stock</span>
          <strong class="mt-0.5 block text-xl leading-none tracking-tight text-ink">{inStock}</strong>
        </div>
        <div class="border-l border-slate-200 px-3.5 py-2 text-right">
          <span class="block text-[10px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Sold</span>
          <strong class="mt-0.5 block text-xl leading-none tracking-tight text-slate-500">{sold}</strong>
        </div>
      </div>
      <Button
        variant="secondary"
        size="sm"
        onclick={() => load(false)}
        disabled={loading}
        aria-label="Refresh devices"
      >
        <svg class="h-4 w-4 fill-none stroke-current stroke-[1.9]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M20 11a8 8 0 1 0-2.3 5.7" /><path d="M20 5v6h-6" />
        </svg>
        Refresh
      </Button>
      <Button size="sm" onclick={() => (adding = !adding)}>
        <svg class="h-4 w-4 fill-none stroke-current stroke-[1.9]" viewBox="0 0 24 24" stroke-linecap="round" aria-hidden="true">
          {#if adding}<path d="M6 6l12 12M18 6L6 18" />{:else}<path d="M12 5v14M5 12h14" />{/if}
        </svg>
        {adding ? 'Close' : 'Register IMEI'}
      </Button>
    </div>
  </header>

  {#if adding}
    <section class="job-pop rounded-2xl border border-slate-200 bg-white p-4">
      <h2 class="text-sm font-extrabold tracking-tight text-ink">Register a device</h2>
      {#if loading && models.length === 0}
        <p class="mt-2 text-[12px] font-semibold text-slate-400">Loading phone models…</p>
      {:else if models.length === 0}
        <p class="mt-2 text-[12px] font-semibold text-slate-500">
          Add mobile brand and model to a catalog item first: Items → open the phone → Mobile details.
        </p>
      {:else}
        <form
          class="mt-3 grid gap-2.5 sm:grid-cols-2 lg:grid-cols-5 [&_.field]:mt-0"
          onsubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <Field label="Phone model">
            <Select bind:value={modelItemId}>
              <option value="">Choose model</option>
              {#each models as model (model.item_id)}
                <option value={String(model.item_id)}>
                  {typeName(model)} (#{model.item_id})
                </option>
              {/each}
            </Select>
          </Field>
          <Field label="IMEI"><Input bind:value={imei} placeholder="356…" /></Field>
          <Field label="Serial"><Input bind:value={serial} placeholder="Optional" /></Field>
          <Field label="Warranty months"><Input inputmode="numeric" bind:value={months} /></Field>
          <div class="flex items-end">
            <Button class="w-full" type="submit" disabled={saving}>
              {saving ? 'Saving…' : 'Add device'}
            </Button>
          </div>
        </form>
      {/if}
    </section>
  {/if}

  <section class="flex min-w-0 flex-wrap items-center gap-2" aria-label="Filter devices">
    <div class="relative flex min-w-[200px] flex-1 items-center">
      <svg class="pointer-events-none absolute left-3.5 h-[18px] w-[18px] fill-none stroke-current stroke-[1.8] text-slate-400" viewBox="0 0 24 24" stroke-linecap="round" aria-hidden="true">
        <circle cx="11" cy="11" r="6.5" /><path d="M16 16l4.5 4.5" />
      </svg>
      <Input
        class="pl-10 pr-10"
        bind:value={search}
        oninput={scheduleLoad}
        placeholder="Search IMEI, serial or model"
        aria-label="Search devices"
      />
      {#if search}
        <button
          class="absolute right-2 grid h-7 w-7 place-items-center rounded-lg bg-slate-100 text-lg leading-none text-slate-500 transition-colors hover:bg-slate-200 hover:text-slate-900"
          type="button"
          onclick={() => {
            search = ''
            load(false)
          }}
          aria-label="Clear search"
        >×</button>
      {/if}
    </div>
    <Select
      class="w-auto min-w-[160px] max-sm:w-full"
      value={typeItemId}
      onchange={(event) => setType(event.currentTarget.value)}
      aria-label="Filter by device type"
    >
      <option value="">All types</option>
      {#each models as model (model.item_id)}
        <option value={String(model.item_id)}>{typeName(model)}</option>
      {/each}
    </Select>
    <Select
      class="w-auto min-w-[140px] max-sm:w-full"
      value={status}
      onchange={(event) => setStatus(event.currentTarget.value)}
      aria-label="Filter by status"
    >
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
    <div class="overflow-hidden rounded-2xl border border-slate-200 bg-white" aria-hidden="true">
      {#each [0, 1, 2, 3, 4] as row (row)}
        <div class="flex items-center gap-4 border-b border-slate-100 px-4 py-4 last:border-0">
          <span class="job-skeleton h-3 w-[22%] rounded-lg"></span>
          <span class="job-skeleton h-3 w-[18%] rounded-lg"></span>
          <span class="job-skeleton h-3 w-[12%] rounded-lg"></span>
          <span class="job-skeleton h-3 flex-1 rounded-lg"></span>
        </div>
      {/each}
    </div>
  {:else if devices.length === 0}
    <section class="rounded-2xl border border-dashed border-slate-300 bg-white px-6 py-14 text-center">
      <h2 class="text-lg font-extrabold tracking-tight text-ink">
        {hasFilters ? 'No devices match' : 'No devices yet'}
      </h2>
      <p class="mt-1 text-[13px] font-semibold text-slate-500">
        {#if hasFilters}
          Try another type or status, or clear the search to see the whole list.
        {:else}
          Add a phone model to an item, then register its first IMEI.
        {/if}
      </p>
      {#if hasFilters}
        <Button
          class="mt-4"
          variant="secondary"
          onclick={() => {
            search = ''
            status = ''
            typeItemId = ''
            load(false)
          }}
        >Clear filters</Button>
      {:else}
        <Button class="mt-4" onclick={() => (adding = true)}>Register the first IMEI</Button>
      {/if}
    </section>
  {:else}
    <div class="overflow-hidden rounded-2xl border border-slate-200 bg-white">
      <div class="overflow-x-auto">
        <div class="min-w-[900px]">
          <div
            class="grid items-center gap-3 border-b border-slate-200 bg-slate-50/80 px-4 py-2.5 text-[10px] font-extrabold uppercase tracking-[0.06em] text-slate-500 {rowGrid}"
            aria-hidden="true"
          >
            <span>Device</span>
            <span>IMEI</span>
            <span>Serial</span>
            <span>Status</span>
            <span>Warranty</span>
            <span>Sold</span>
            <span></span>
          </div>
          {#each devices as device (device.id)}
            <button
              type="button"
              class="grid w-full cursor-pointer items-center gap-3 border-b border-slate-100 px-4 py-3 text-left text-ink transition-colors last:border-0 hover:bg-teal-50/60 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-focus/30 focus-visible:ring-inset {rowGrid}"
              onclick={() => (selected = device)}
              aria-label={`Open ${deviceName(device)}, IMEI ${device.imei}`}
            >
              <span class="flex min-w-0 flex-col gap-0.5">
                <strong class="truncate text-[13px] font-bold">{deviceName(device)}</strong>
                <span class="truncate text-[11px] font-semibold text-slate-400">
                  Received {device.received_on} · Item #{device.item_id}
                </span>
              </span>
              <span class="truncate font-mono text-[12px] font-semibold text-slate-700">{device.imei}</span>
              <span class="truncate text-[12px] font-semibold text-slate-500">{device.serial || '—'}</span>
              <span>
                <span class="inline-flex h-6 items-center rounded-full px-2 text-[10px] font-extrabold {statusClass(device.status)}">
                  {device.status === 'in_stock' ? 'In stock' : 'Sold'}
                </span>
              </span>
              <span class="flex min-w-0 flex-col gap-0.5">
                <span class="truncate text-[12px] font-semibold {device.status === 'sold' && warrantyState(device) === 'expired' ? 'text-rose-600' : 'text-slate-700'}">
                  {warrantyTitle(device)}
                </span>
                <span class="truncate text-[10px] font-bold text-slate-400">{warrantySub(device)}</span>
              </span>
              {#if device.status === 'sold'}
                <span class="flex min-w-0 flex-col gap-0.5">
                  <strong class="truncate text-[12px] font-bold text-slate-700">{device.sold_bill_no || 'Sold'}</strong>
                  <span class="truncate text-[11px] font-semibold text-slate-400">{device.customer_name || 'No customer'}</span>
                </span>
              {:else}
                <span class="text-[12px] font-semibold text-slate-300">—</span>
              {/if}
              <svg class="h-4 w-4 justify-self-end fill-none stroke-slate-400 stroke-[2]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M9 6l6 6-6 6" />
              </svg>
            </button>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</section>

{#if selected}
  <div class="fixed inset-0 z-50 flex justify-end">
    <button
      class="job-fade absolute inset-0 cursor-pointer bg-slate-900/40 backdrop-blur-sm"
      type="button"
      aria-label="Close device details"
      onclick={closeDevice}
    ></button>
    <div
      class="job-drawer relative z-10 flex h-dvh w-full max-w-[460px] flex-col bg-slate-50 shadow-2xl"
      role="dialog"
      aria-modal="true"
      aria-labelledby="device-drawer-title"
    >
      <header class="flex items-center gap-3.5 border-b-4 border-teal-600 bg-slate-900 p-5 text-white">
        <span class="grid h-12 w-12 flex-none place-items-center rounded-2xl bg-white/15" aria-hidden="true">
          <svg class="h-6 w-6 fill-none stroke-current stroke-[1.7]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
            <rect x="7" y="2.5" width="10" height="19" rx="2.5" /><path d="M11 18.5h2" />
          </svg>
        </span>
        <div class="min-w-0 flex-1">
          <h2 id="device-drawer-title" class="truncate text-xl font-extrabold tracking-tight">{deviceName(selected)}</h2>
          <p class="mt-0.5 truncate font-mono text-xs font-semibold text-white/75">{selected.imei}</p>
        </div>
        <button
          class="grid h-9 w-9 flex-none place-items-center rounded-xl bg-white/10 text-white transition-colors hover:bg-white/20"
          type="button"
          onclick={closeDevice}
          aria-label="Close"
        >
          <svg class="h-[18px] w-[18px] fill-none stroke-current stroke-[1.9]" viewBox="0 0 24 24" stroke-linecap="round" aria-hidden="true">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
      </header>

      <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-5 pb-8 pt-4">
        <div class="flex flex-wrap gap-2">
          <span class="inline-flex h-[26px] items-center gap-1.5 rounded-full px-2.5 text-[11px] font-extrabold {statusClass(selected.status)}">
            <span class="h-1.5 w-1.5 rounded-full bg-current opacity-80"></span>
            {selected.status === 'in_stock' ? 'In stock' : 'Sold'}
          </span>
          {#if selected.status === 'sold'}
            <span class="inline-flex h-[26px] items-center gap-1.5 rounded-full px-2.5 text-[11px] font-extrabold {warrantyState(selected) === 'active'
              ? 'bg-emerald-100 text-emerald-700'
              : warrantyState(selected) === 'expired'
                ? 'bg-rose-100 text-rose-700'
                : 'bg-slate-100 text-slate-600'}">
              <span class="h-1.5 w-1.5 rounded-full bg-current opacity-80"></span>
              {warrantyState(selected) === 'active' ? 'In warranty' : warrantyState(selected) === 'expired' ? 'Out of warranty' : 'Warranty syncing'}
            </span>
          {/if}
        </div>

        <section class="rounded-2xl border border-slate-200 bg-white p-4">
          <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Device</h3>
          <div class="mt-3 grid grid-cols-2 gap-2">
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">IMEI</span>
              <strong class="mt-0.5 block break-all font-mono text-[12px] text-slate-900">{selected.imei}</strong>
            </div>
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Serial</span>
              <strong class="mt-0.5 block truncate text-[13px] text-slate-900">{selected.serial || '—'}</strong>
            </div>
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Catalog item</span>
              <strong class="mt-0.5 block truncate text-[13px] text-slate-900">#{selected.item_id}</strong>
            </div>
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Received</span>
              <strong class="mt-0.5 block truncate text-[13px] text-slate-900">{selected.received_on}</strong>
            </div>
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Warranty</span>
              <strong class="mt-0.5 block truncate text-[13px] text-slate-900">{selected.warranty_months} months</strong>
            </div>
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">State</span>
              <strong class="mt-0.5 block truncate text-[13px] text-slate-900">{selected.status === 'in_stock' ? 'In stock' : 'Sold'}</strong>
            </div>
          </div>
        </section>

        <section class="rounded-2xl border border-slate-200 bg-white p-4">
          <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Sale</h3>
          {#if selected.status === 'sold'}
            <div class="mt-3 grid grid-cols-2 gap-2">
              <div class="rounded-xl bg-slate-50 px-2.5 py-2">
                <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Bill</span>
                <strong class="mt-0.5 block truncate text-[13px] text-slate-900">{selected.sold_bill_no || '—'}</strong>
              </div>
              <div class="rounded-xl bg-slate-50 px-2.5 py-2">
                <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Sold on</span>
                <strong class="mt-0.5 block truncate text-[13px] text-slate-900">{soldOn(selected)}</strong>
              </div>
              <div class="rounded-xl bg-slate-50 px-2.5 py-2">
                <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Customer</span>
                <strong class="mt-0.5 block truncate text-[13px] text-slate-900">{selected.customer_name || '—'}</strong>
              </div>
              <div class="rounded-xl bg-slate-50 px-2.5 py-2">
                <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Warranty ends</span>
                <strong class="mt-0.5 block truncate text-[13px] text-slate-900">{selected.ends_on || 'Syncing…'}</strong>
              </div>
            </div>
            <p class="mt-3 text-[12px] font-semibold text-slate-600">{warrantyLabel(selected)}</p>
          {:else}
            <p class="mt-2 text-[13px] font-semibold text-slate-600">In stock — ready to sell at the till.</p>
            <p class="mt-1 text-[12px] font-semibold text-slate-500">
              The warranty starts on the sale date and runs {selected.warranty_months} months.
            </p>
          {/if}
        </section>

        <p class="px-1 text-[11px] font-semibold text-slate-400">
          Repairs and replacements are recorded from Mobile → Warranty.
        </p>
      </div>
    </div>
  </div>
{/if}
