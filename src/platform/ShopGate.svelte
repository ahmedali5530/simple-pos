<script lang="ts">
  import type { Snippet } from 'svelte'
  import { loadPos } from '../lib/api'
  import type { PosSnapshot } from '../lib/types'

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
  <p class="loading">The till did not open.</p>
{:else}
  <p class="loading">Opening the till…</p>
{/if}
