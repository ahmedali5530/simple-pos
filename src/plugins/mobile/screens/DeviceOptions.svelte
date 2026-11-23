<script lang="ts">
  import type { PosItemOptionsContext } from '../../../platform/slots'
  import { Button, Input } from '../../../lib/ui'
  import { listDevices, type MobileDevice } from '../api'
  import { hasMobileItemOptions } from '../options'

  let { item, addLine, lines }: PosItemOptionsContext = $props()

  // Contributed for every variant sheet; only phone models have IMEIs to show.
  const isMobile = $derived(hasMobileItemOptions(item.id))

  let devices = $state<MobileDevice[]>([])
  let query = $state('')
  let loading = $state(false)
  let failed = $state(false)
  let added = $state('')

  const used = $derived(new Set(lines.map((line) => line.reference).filter(Boolean)))

  const available = $derived.by(() => {
    const needle = query.trim().toLowerCase()
    return devices.filter((device) => {
      if (device.item_id !== item.id) return false
      if (used.has(`device:${device.id}`)) return false
      if (!needle) return true
      return `${device.imei} ${device.serial}`.toLowerCase().includes(needle)
    })
  })

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

  function add(device: MobileDevice) {
    const ok = addLine({
      item_id: item.id,
      qty: 1,
      source: 'mobile.device',
      reference: `device:${device.id}`,
      label: device.imei,
      details: { device_id: device.id, imei: device.imei },
    })
    if (ok) {
      added = device.imei
      query = ''
    }
  }

  $effect(() => {
    if (isMobile) load()
  })
</script>

{#if isMobile}
<div class="flex flex-col gap-2">
  <div class="flex items-center justify-between gap-2">
    <p class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">
      IMEIs in stock
      <span class="ml-1 rounded-full bg-teal-700 px-1.5 py-0.5 text-[10px] text-white">{available.length}</span>
    </p>
    <Button variant="secondary" size="sm" onclick={load} disabled={loading}>
      {loading ? '…' : 'Refresh'}
    </Button>
  </div>

  <Input bind:value={query} placeholder="Search IMEI or serial" aria-label="Search IMEIs" />

  {#if added}
    <p class="text-[11px] font-semibold text-emerald-700">Added IMEI {added}. Pick another, or close.</p>
  {/if}

  {#if failed}
    <p class="text-[11px] font-semibold text-rose-700">Could not load phones. Try Refresh.</p>
  {:else if loading && available.length === 0}
    <p class="text-[11px] font-semibold text-slate-400">Loading phones…</p>
  {:else if available.length === 0}
    <p class="text-[11px] font-semibold text-slate-400">
      No IMEIs in stock for this model. Add devices under Mobile → Devices.
    </p>
  {:else}
    <div class="flex max-h-56 flex-col gap-1 overflow-auto">
      {#each available as device (device.id)}
        <button
          type="button"
          class="flex items-center justify-between gap-2 rounded-lg border border-slate-200 bg-slate-50 px-2.5 py-2 text-left transition-colors hover:border-teal-300 hover:bg-teal-50"
          onclick={() => add(device)}
        >
          <span class="min-w-0">
            <strong class="block truncate text-xs text-slate-800">{device.imei}</strong>
            <span class="block truncate text-[11px] font-semibold text-slate-500">
              {device.serial ? `Serial ${device.serial}` : `${device.warranty_months} month warranty`}
            </span>
          </span>
          <span class="text-[10px] font-extrabold uppercase tracking-[0.05em] text-teal-700">Add</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
{/if}
