<script lang="ts">
  import { onMount } from 'svelte'
  import type { PosLineSourceContext } from '../../../platform/slots'
  import { Button, Input } from '../../../lib/ui'
  import { listDevices, type MobileDevice } from '../api'

  let { section, addLine, lines }: PosLineSourceContext = $props()

  let devices = $state<MobileDevice[]>([])
  let query = $state('')
  let open = $state(false)
  let loading = $state(false)
  let failed = $state(false)

  const available = $derived.by(() => {
    const itemIds = new Set(section.items.map((item) => item.id))
    const used = new Set(lines.map((line) => line.reference).filter(Boolean))
    const needle = query.trim().toLowerCase()
    return devices.filter((device) => {
      if (!itemIds.has(device.item_id)) return false
      if (used.has(`device:${device.id}`)) return false
      if (!needle) return true
      return `${device.brand} ${device.model} ${device.imei} ${device.serial}`
        .toLowerCase()
        .includes(needle)
    })
  })

  function deviceLabel(device: MobileDevice) {
    const name = [device.brand, device.model].filter(Boolean).join(' ')
    return name ? `${name} · ${device.imei}` : device.imei
  }

  async function load() {
    loading = true
    failed = false
    try {
      devices = (await listDevices({ status: 'in_stock' })).devices
    } catch {
      failed = true
      devices = []
    } finally {
      loading = false
    }
  }

  // A posted bill clears the cart: reload so its devices cannot be picked again.
  let lastLineCount = 0
  $effect(() => {
    const count = lines.length
    if (count === 0 && lastLineCount > 0) load()
    lastLineCount = count
  })

  function add(device: MobileDevice) {
    const ok = addLine({
      item_id: device.item_id,
      qty: 1,
      source: 'mobile.device',
      reference: `device:${device.id}`,
      label: deviceLabel(device),
      details: { device_id: device.id, imei: device.imei },
    })
    if (ok) query = ''
  }

  onMount(load)
</script>

<div class="rounded-xl border border-slate-200 bg-white p-2">
  <button
    type="button"
    class="flex w-full items-center justify-between gap-2 rounded-lg px-2 py-2 text-left transition-colors hover:bg-slate-50"
    onclick={() => (open = !open)}
  >
    <span class="flex items-center gap-2 text-xs font-extrabold text-slate-700">
      Sell a phone
      <span class="grid h-5 min-w-5 place-items-center rounded-full bg-teal-700 px-1.5 text-[10px] text-white">
        {available.length}
      </span>
    </span>
    <span class="text-[11px] font-bold text-slate-500">{open ? 'Hide' : 'Choose'}</span>
  </button>

  {#if open}
    <div class="mt-2 flex flex-col gap-2 border-t border-slate-100 pt-2">
      <div class="flex gap-2">
        <Input bind:value={query} placeholder="Search IMEI, brand or model" />
        <Button variant="secondary" size="sm" onclick={load} disabled={loading}>
          {loading ? '…' : 'Refresh'}
        </Button>
      </div>
      {#if failed}
        <p class="text-[11px] font-semibold text-rose-700">Could not load phones.</p>
      {:else if loading && available.length === 0}
        <p class="text-[11px] font-semibold text-slate-400">Loading phones…</p>
      {:else if available.length === 0}
        <p class="text-[11px] font-semibold text-slate-400">
          No phones in stock for this section. Add devices under Mobile → Devices.
        </p>
      {:else}
        <div class="flex max-h-56 flex-col gap-1 overflow-auto">
          {#each available as device (device.id)}
            <button
              type="button"
              class="flex items-center justify-between gap-2 rounded-lg border border-slate-100 bg-slate-50 px-2.5 py-2 text-left transition-colors hover:border-teal-300 hover:bg-teal-50"
              onclick={() => add(device)}
            >
              <span class="min-w-0">
                <strong class="block truncate text-xs text-slate-800">
                  {device.brand} {device.model}
                </strong>
                <span class="block truncate text-[11px] font-semibold text-slate-500">{device.imei}</span>
              </span>
              <span class="text-[10px] font-extrabold uppercase tracking-[0.05em] text-teal-700">Add</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
