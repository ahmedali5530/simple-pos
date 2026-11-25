<script lang="ts">
  import type { Snippet } from 'svelte'
  import { loadPos } from '../lib/api'
  import type { PosSnapshot } from '../lib/types'
  import PluginNeed from './PluginNeed.svelte'

  interface Props {
    children: Snippet<[PosSnapshot, (pos: PosSnapshot) => void]>
  }

  let { children }: Props = $props()
  let snapshot = $state<PosSnapshot | null>(null)
  let failed = $state(false)

  $effect(() => {
    loadPos()
      .then((pos) => (snapshot = pos))
      .catch(() => (failed = true))
  })
</script>

{#if snapshot}
  {@render children(snapshot, (pos) => (snapshot = pos))}
{:else if failed}
  <PluginNeed
    plugin="Point of Sale"
    purpose="Enable Point of Sale in Settings → Plugins to open the till."
  />
{:else}
  <p class="loading">Opening the till…</p>
{/if}
