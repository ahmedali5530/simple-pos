<script lang="ts" generics="T">
  import type { Snippet } from 'svelte'
  import { createVirtual } from '../virtual.svelte'

  let {
    items,
    estimate,
    min,
    gap = 12,
    key,
    reset = '',
    class: className = '',
    children,
  }: {
    items: T[]
    estimate: number
    min: number
    gap?: number
    key: (item: T) => string | number
    reset?: string
    class?: string
    children: Snippet<[T]>
  } = $props()

  const frame = createVirtual()
  let seen = ''
  const columns = $derived(
    frame.inner > 0 ? Math.max(1, Math.floor((frame.inner + gap) / (min + gap))) : Math.max(1, Math.floor((960 + gap) / (min + gap))),
  )
  const rows = $derived.by(() => {
    const grouped: T[][] = []
    for (let index = 0; index < items.length; index += columns) grouped.push(items.slice(index, index + columns))
    return grouped
  })

  $effect.pre(() => {
    frame.configure(rows, (row) => row.map((item) => key(item)).join('|'), estimate, gap)
    if (seen !== reset) {
      seen = reset
      frame.resetScroll()
    }
  })
</script>

<div class="virtual-scroll {className}" use:frame.attach>
  <div class="virtual-space" style:height="{frame.total}px">
    {#each rows.slice(frame.start, frame.end) as row, offset (row.map((item) => key(item)).join('|'))}
      <div
        class="virtual-row"
        style:top="{frame.placed[offset]}px"
        style:grid-template-columns="repeat({columns}, minmax(0, 1fr))"
        use:frame.measure={row.map((item) => key(item)).join('|')}
      >
        {#each row as item (key(item))}
          {@render children(item)}
        {/each}
      </div>
    {/each}
  </div>
</div>
