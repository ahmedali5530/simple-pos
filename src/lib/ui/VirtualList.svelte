<script lang="ts" generics="T">
  import type { Snippet } from 'svelte'
  import { createVirtual } from '../virtual.svelte'

  let {
    items,
    estimate,
    key,
    gap = 0,
    reset = '',
    class: className = '',
    children,
    footer,
  }: {
    items: T[]
    estimate: number
    key: (item: T, index: number) => string | number
    gap?: number
    reset?: string
    class?: string
    children: Snippet<[T, number]>
    footer?: Snippet
  } = $props()

  const frame = createVirtual()
  let seen = ''

  $effect.pre(() => {
    frame.configure(items, key, estimate, gap)
    if (seen !== reset) {
      seen = reset
      frame.resetScroll()
    }
  })
</script>

<div class="virtual-scroll {className}" use:frame.attach>
  <div class="virtual-space" style:height="{frame.total}px">
    {#each items.slice(frame.start, frame.end) as item, offset (key(item, frame.start + offset))}
      <div class="virtual-item" style:top="{frame.placed[offset]}px" use:frame.measure={key(item, frame.start + offset)}>
        {@render children(item, frame.start + offset)}
      </div>
    {/each}
  </div>
  {#if footer}
    {@render footer()}
  {/if}
</div>
