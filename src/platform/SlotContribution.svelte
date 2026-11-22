<script lang="ts">
  import type { Component } from 'svelte'
  import type { ResolvedSlotContribution } from '../plugins/registry'

  interface Props {
    contribution: ResolvedSlotContribution
    context?: Record<string, unknown>
  }

  let { contribution, context = {} }: Props = $props()

  // Depend only on the loader identity: context changes must not reload the chunk.
  const loader = $derived(contribution.component)
  let loaded = $state.raw<Component | null>(null)

  $effect(() => {
    const load = loader
    if (!load) {
      loaded = null
      return
    }
    let cancelled = false
    loaded = null
    load()
      .then((module) => {
        if (!cancelled) loaded = module.default
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          console.error(`[slot] ${contribution.owner}:${contribution.id} failed to load`, error)
        }
      })
    return () => {
      cancelled = true
    }
  })
</script>

{#if loaded}
  {@const SlotComponent = loaded}
  <SlotComponent {...context} />
{/if}
