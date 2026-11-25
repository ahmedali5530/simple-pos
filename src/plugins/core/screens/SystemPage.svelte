<script lang="ts">
  import { onMount } from 'svelte'
  import { loadCoreSnapshot } from '../../../lib/api'
  import type { ShopSnapshot } from '../../../lib/types'
  import SystemSettings from './SystemSettings.svelte'

  let snapshot = $state<ShopSnapshot | null>(null)
  let failed = $state(false)

  onMount(() => {
    loadCoreSnapshot()
      .then((shop) => (snapshot = shop))
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
