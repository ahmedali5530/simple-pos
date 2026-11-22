<script lang="ts">
  import { onMount } from 'svelte'
  import { formatMoney } from '../../../lib/money'
  import { toastError, toastSaved } from '../../../lib/toast'
  import { Button, Field, Input, Select, Textarea } from '../../../lib/ui'
  import {
    listClaims,
    listDevices,
    openClaim,
    updateClaim,
    type ClaimKind,
    type ClaimStatus,
    type MobileClaim,
    type MobileDevice,
  } from '../api'

  interface Props {
    currency: string
    locale: string
  }

  let { currency, locale }: Props = $props()

  let query = $state('')
  let results = $state<MobileDevice[]>([])
  let selected = $state<MobileDevice | null>(null)
  let claims = $state<MobileClaim[]>([])
  let openClaims = $state<MobileClaim[]>([])
  let loading = $state(false)
  let failed = $state(false)
  let saving = $state(false)
  let kind = $state<ClaimKind>('repair')
  let problem = $state('')
  let cost = $state('0')

  function localToday() {
    const now = new Date()
    const month = String(now.getMonth() + 1).padStart(2, '0')
    const day = String(now.getDate()).padStart(2, '0')
    return `${now.getFullYear()}-${month}-${day}`
  }

  function warrantyState(device: MobileDevice) {
    if (!device.ends_on) return 'none'
    return device.ends_on >= localToday() ? 'active' : 'expired'
  }

  function deviceName(device: MobileDevice) {
    return [device.brand, device.model].filter(Boolean).join(' ') || `Item #${device.item_id}`
  }

  async function refreshOpen() {
    try {
      const [open, progress] = await Promise.all([
        listClaims({ status: 'open' }),
        listClaims({ status: 'in_progress' }),
      ])
      openClaims = [...open.claims, ...progress.claims]
    } catch {
      openClaims = []
    }
  }

  async function search() {
    const term = query.trim()
    if (!term) return
    loading = true
    failed = false
    try {
      results = (await listDevices({ search: term })).devices
      if (results.length === 1) {
        await selectDevice(results[0])
      } else {
        selected = null
        claims = []
      }
    } catch (error) {
      failed = true
      toastError(error, 'Could not find that device.')
    } finally {
      loading = false
    }
  }

  async function selectDevice(device: MobileDevice) {
    selected = device
    results = []
    try {
      claims = (await listClaims({ device_id: device.id })).claims
    } catch {
      claims = []
    }
  }

  async function openFromClaim(claim: MobileClaim) {
    query = claim.imei
    try {
      const body = await listDevices({ search: claim.imei })
      const device = body.devices.find((row) => row.id === claim.device_id) ?? body.devices[0]
      if (device) await selectDevice(device)
    } catch (error) {
      toastError(error, 'Could not open that device.')
    }
  }

  async function submitClaim() {
    if (!selected) return
    if (!problem.trim()) {
      toastError(null, 'Describe the problem.')
      return
    }
    const value = Number(cost || '0')
    if (!Number.isInteger(value) || value < 0) {
      toastError(null, 'Enter a valid cost.')
      return
    }
    saving = true
    try {
      await openClaim({
        device_id: selected.id,
        kind,
        problem: problem.trim(),
        cost: value,
      })
      problem = ''
      cost = '0'
      claims = (await listClaims({ device_id: selected.id })).claims
      await refreshOpen()
      toastSaved('Claim recorded.')
    } catch (error) {
      toastError(error, 'Could not record the claim.')
    } finally {
      saving = false
    }
  }

  async function saveClaim(
    claim: MobileClaim,
    patch: { status?: ClaimStatus; resolution?: string; cost?: number },
  ) {
    try {
      const result = await updateClaim(claim.id, {
        status: patch.status ?? claim.status,
        resolution: patch.resolution ?? claim.resolution,
        cost: patch.cost ?? claim.cost,
      })
      claims = claims.map((row) => (row.id === claim.id ? result.claim : row))
      openClaims = openClaims
        .map((row) => (row.id === claim.id ? result.claim : row))
        .filter((row) => row.status === 'open' || row.status === 'in_progress')
      toastSaved('Claim updated.')
    } catch (error) {
      toastError(error, 'Could not update the claim.')
    }
  }

  function warrantyLabel(device: MobileDevice) {
    const state = warrantyState(device)
    if (state === 'none') return 'No warranty record yet — the sale may still be syncing.'
    if (state === 'active') return `In warranty until ${device.ends_on}`
    return `Out of warranty since ${device.ends_on}`
  }

  onMount(refreshOpen)
</script>

<section class="flex w-full min-w-0 flex-1 flex-col gap-4 pb-5">
  <header class="min-w-0">
    <span class="flex items-center gap-2 text-[11px] font-extrabold uppercase tracking-[0.09em] text-teal-700">
      <span class="h-0.5 w-5 rounded-full bg-current"></span>
      Mobile
    </span>
    <h1 class="mt-1.5 text-3xl font-extrabold tracking-tight text-ink">Warranty</h1>
    <p class="mt-2 text-[13px] font-semibold text-slate-500">
      Find a phone by IMEI or serial, check its warranty and record repairs or replacements.
    </p>
  </header>

  <section class="rounded-2xl border border-slate-200 bg-white p-4">
    <div class="flex flex-wrap items-end gap-2.5">
      <div class="min-w-[220px] flex-1 max-w-[520px]">
        <Field label="Find a device">
          <Input
            bind:value={query}
            placeholder="IMEI, serial, brand or model"
            onkeydown={(event) => {
              if (event.key === 'Enter') search()
            }}
          />
        </Field>
      </div>
      <Button onclick={search} disabled={loading}>{loading ? 'Searching…' : 'Search'}</Button>
    </div>

    {#if failed}
      <p class="mt-3 text-[12px] font-semibold text-rose-700">Could not find that device.</p>
    {/if}

    {#if results.length > 1}
      <div class="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {#each results as device (device.id)}
          <button
            type="button"
            class="rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-left transition-colors hover:border-teal-300 hover:bg-teal-50"
            onclick={() => selectDevice(device)}
          >
            <strong class="block truncate text-xs text-slate-800">{deviceName(device)}</strong>
            <span class="block truncate text-[11px] font-semibold text-slate-500">{device.imei}</span>
            <span class="mt-0.5 block text-[10px] font-bold text-slate-400">
              {device.status === 'sold' ? 'Sold' : 'In stock'}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </section>

  {#if openClaims.length > 0}
    <section class="flex flex-col gap-2 rounded-2xl border border-slate-200 bg-white p-4">
      <h2 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Open claims</h2>
      <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {#each openClaims as claim (claim.id)}
          <button
            type="button"
            class="rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-left transition-colors hover:bg-amber-100"
            onclick={() => openFromClaim(claim)}
          >
            <strong class="block truncate text-xs text-slate-800">
              {claim.brand} {claim.model} · {claim.imei}
            </strong>
            <span class="mt-0.5 block truncate text-[11px] font-semibold text-slate-600">{claim.problem}</span>
            <span class="mt-0.5 block text-[10px] font-bold text-amber-700">
              {claim.kind === 'replacement' ? 'Replacement' : 'Repair'} · opened {claim.opened_on}
            </span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  {#if selected}
    <section class="flex flex-col gap-3 rounded-2xl border border-slate-200 bg-white p-4">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <h2 class="truncate text-lg font-extrabold tracking-tight text-ink">{deviceName(selected)}</h2>
          <p class="mt-0.5 text-[11px] font-semibold text-slate-500">{selected.imei}{selected.serial ? ` · ${selected.serial}` : ''}</p>
        </div>
        <span class="inline-flex h-7 items-center rounded-full px-2.5 text-[10px] font-extrabold {warrantyState(selected) === 'active'
          ? 'bg-emerald-100 text-emerald-700'
          : warrantyState(selected) === 'expired'
            ? 'bg-rose-100 text-rose-700'
            : 'bg-slate-100 text-slate-600'}">
          {warrantyState(selected) === 'active' ? 'In warranty' : warrantyState(selected) === 'expired' ? 'Out of warranty' : 'No warranty'}
        </span>
      </div>

      <p class="text-[12px] font-semibold text-slate-600">{warrantyLabel(selected)}</p>
      {#if selected.status === 'sold'}
        <p class="text-[11px] font-semibold text-slate-500">
          Sold {selected.sold_bill_no ? `on ${selected.sold_bill_no}` : ''}
          {selected.sold_at ? ` · ${selected.sold_at.slice(0, 10)}` : ''}
          {selected.customer_name ? ` · ${selected.customer_name}` : ''}
        </p>
      {:else}
        <p class="text-[11px] font-semibold text-amber-700">This device is still in stock.</p>
      {/if}

      <div class="rounded-xl border border-slate-200 p-3">
        <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Record a claim</h3>
        <div class="mt-2 grid gap-2.5 sm:grid-cols-3 [&_.field]:mt-0">
          <Field label="Kind">
            <Select bind:value={kind}>
              <option value="repair">Repair</option>
              <option value="replacement">Replacement</option>
            </Select>
          </Field>
          <Field label="Cost">
            <Input inputmode="numeric" bind:value={cost} placeholder="0" />
          </Field>
          <div class="sm:col-span-3">
            <Field label="Problem">
              <Textarea bind:value={problem} rows={2} placeholder="What is wrong?" />
            </Field>
          </div>
        </div>
        <Button class="mt-3" onclick={submitClaim} disabled={saving}>
          {saving ? 'Saving…' : 'Record claim'}
        </Button>
      </div>

      <div class="flex flex-col gap-2">
        {#if claims.length === 0}
          <p class="text-xs font-semibold text-slate-400">No claims for this device.</p>
        {:else}
          {#each claims as claim (claim.id)}
            <div class="rounded-xl border border-slate-200 p-3">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <strong class="text-sm text-ink">
                  {claim.kind === 'repair' ? 'Repair' : 'Replacement'} #{claim.id}
                </strong>
                <span class="text-[11px] font-bold text-slate-500">{formatMoney(claim.cost, currency, locale)}</span>
              </div>
              <p class="mt-1 text-[12px] font-semibold text-slate-600">{claim.problem}</p>
              <p class="mt-0.5 text-[10px] font-bold text-slate-400">
                Opened {claim.opened_on}{claim.closed_on ? ` · closed ${claim.closed_on}` : ''}
              </p>
              <div class="mt-2 grid gap-2 sm:grid-cols-3">
                <Select
                  value={claim.status}
                  onchange={(event) => saveClaim(claim, { status: event.currentTarget.value as ClaimStatus })}
                >
                  <option value="open">Open</option>
                  <option value="in_progress">In progress</option>
                  <option value="resolved">Resolved</option>
                  <option value="rejected">Rejected</option>
                </Select>
                <Input
                  value={claim.resolution}
                  placeholder="Resolution"
                  onchange={(event) => saveClaim(claim, { resolution: event.currentTarget.value })}
                />
                <Input
                  value={String(claim.cost)}
                  inputmode="numeric"
                  placeholder="Cost"
                  onchange={(event) => saveClaim(claim, { cost: Number(event.currentTarget.value) || 0 })}
                />
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </section>
  {/if}
</section>
