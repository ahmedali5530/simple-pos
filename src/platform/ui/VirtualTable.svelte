<script lang="ts" generics="T">
  import type { Snippet } from 'svelte'
  import { createVirtual } from '../virtual.svelte'

  let {
    items,
    estimate,
    key,
    reset = '',
    columns,
    class: className = '',
    tableClass = '',
    header,
    row,
    footer,
  }: {
    items: T[]
    estimate: number
    key: (item: T, index: number) => string | number
    reset?: string
    columns: number
    class?: string
    tableClass?: string
    header: Snippet
    row: Snippet<[T, number]>
    footer?: Snippet
  } = $props()

  const frame = createVirtual()
  let seen = ''

  $effect.pre(() => {
    frame.configure(items, key, estimate, 0)
    if (seen !== reset) {
      seen = reset
      frame.resetScroll()
    }
  })
</script>

<div class="virtual-scroll {className}" use:frame.attach>
  <table class={tableClass}>
    <thead>
      {@render header()}
    </thead>
    {#if frame.before > 0}
      <tbody>
        <tr class="virtual-pad"><td colspan={columns}><div style:height="{frame.before}px"></div></td></tr>
      </tbody>
    {/if}
    {#each items.slice(frame.start, frame.end) as item, offset (key(item, frame.start + offset))}
      <tbody use:frame.measure={key(item, frame.start + offset)}>
        {@render row(item, frame.start + offset)}
      </tbody>
    {/each}
    {#if frame.after > 0}
      <tbody>
        <tr class="virtual-pad"><td colspan={columns}><div style:height="{frame.after}px"></div></td></tr>
      </tbody>
    {/if}
  </table>
  {#if footer}
    {@render footer()}
  {/if}
</div>
