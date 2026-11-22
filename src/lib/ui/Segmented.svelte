<script lang="ts">
  export interface Segment {
    value: string
    label: string
    tone?: 'default' | 'danger'
  }

  type Props = {
    value?: string
    options: Segment[]
    label: string
    class?: string
    onchange?: (value: string) => void
  }

  let { value = $bindable(''), options, label, class: className = '', onchange }: Props = $props()

  function choose(next: string) {
    if (onchange) onchange(next)
    else value = next
  }
</script>

<div class={['segmented', className].filter(Boolean).join(' ')} role="radiogroup" aria-label={label}>
  {#each options as option (option.value)}
    <button type="button" role="radio" class:out={option.tone === 'danger'} aria-checked={value === option.value} onclick={() => choose(option.value)}>
      {option.label}
    </button>
  {/each}
</div>
