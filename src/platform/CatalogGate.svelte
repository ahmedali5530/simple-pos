<script lang="ts">
  import type { Snippet } from 'svelte'
  import { loadCatalog } from '../lib/api'
  import type { PosSnapshot } from '../lib/types'
  import PluginNeed from './PluginNeed.svelte'

  interface Props {
    children: Snippet<[PosSnapshot, (pos: PosSnapshot) => void]>
  }

  let { children }: Props = $props()
  let snapshot = $state<PosSnapshot | null>(null)
  let failed = $state(false)

  $effect(() => {
    loadCatalog()
      .then((pos) => (snapshot = pos))
      .catch(() => (failed = true))
  })
</script>

{#if snapshot}
  {@render children(snapshot, (pos) => (snapshot = pos))}
{:else if failed}
  <PluginNeed
    plugin="Catalog"
    purpose="Enable the Catalog plugin in Settings → Plugins to manage items and stock on this page."
  />
{:else}
  <p class="loading">Opening the catalog…</p>
{/if}
