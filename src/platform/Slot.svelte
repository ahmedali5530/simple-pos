<script lang="ts">
  /**
   * Renders every enabled plugin contribution for a named outlet, in order.
   * The shell does not know what the contribution means.
   */
  import type { Snippet } from 'svelte'

  interface Contribution {
    id: string
    order: number
    render: Snippet
  }

  interface Props {
    name: string
    contributions?: Contribution[]
  }

  let { name, contributions = [] }: Props = $props()
  const ordered = $derived([...contributions].sort((a, b) => a.order - b.order))
</script>

<div class="slot" data-slot={name}>
  {#each ordered as contribution (contribution.id)}
    {@render contribution.render()}
  {/each}
</div>
