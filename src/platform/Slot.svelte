<script lang="ts">
  /**
   * Renders every enabled plugin contribution for a named outlet, in order.
   * The host supplies the context; the shell does not know what the contribution means.
   */
  import SlotContribution from './SlotContribution.svelte'
  import { slotsFor } from '../plugins/registry'

  interface Props {
    name: string
    context?: Record<string, unknown>
  }

  let { name, context = {} }: Props = $props()
  const contributions = $derived(slotsFor(name))
</script>

{#if contributions.length > 0}
  <div class="slot" data-slot={name}>
    {#each contributions as contribution (`${contribution.owner}:${contribution.id}`)}
      <SlotContribution {contribution} {context} />
    {/each}
  </div>
{/if}
