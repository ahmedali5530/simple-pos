<script lang="ts">
  import { onMount } from 'svelte'
  import { loadCoreSnapshot, loadPos } from '../../../lib/api'
  import type { BarcodeRule, ShopSnapshot } from '../../../lib/types'
  import SystemSettings, { type SystemView } from './SystemSettings.svelte'

  let snapshot = $state<SystemView | null>(null)
  let failed = $state(false)

  async function load() {
    const shop: ShopSnapshot = await loadCoreSnapshot()
    let bill_prefix = 'BILL'
    let barcode_rules: BarcodeRule[] = []
    try {
      const pos = await loadPos()
      bill_prefix = pos.bill_prefix
      barcode_rules = pos.barcode_rules
    } catch {
      // Till plugin may be off.
    }
    snapshot = { ...shop, bill_prefix, barcode_rules }
  }

  onMount(() => {
    load()
      .then(() => {})
      .catch(() => (failed = true))
  })
</script>

{#if snapshot}
  <SystemSettings {snapshot} onChange={(shop) => (snapshot = shop)} />
{:else if failed}
  <p class="loading">System settings could not be loaded.</p>
{:else}
  <p class="loading">Loading system settings…</p>
{/if}
