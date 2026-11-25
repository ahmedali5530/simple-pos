<script lang="ts">
  import type { Snippet } from 'svelte'
  import { loadCoreSnapshot } from '../lib/api'
  import type { ShopSnapshot } from '../lib/types'

  interface Props {
    children: Snippet<[ShopSnapshot]>
  }

  let { children }: Props = $props()
  let shop = $state<ShopSnapshot | null>(null)
  let failed = $state(false)

  $effect(() => {
    loadCoreSnapshot()
      .then((snapshot) => (shop = snapshot))
      .catch(() => (failed = true))
  })
</script>

{#if shop}
  {@render children(shop)}
{:else if failed}
  <p class="loading">Shop settings could not be loaded.</p>
{:else}
  <p class="loading">Opening…</p>
{/if}
