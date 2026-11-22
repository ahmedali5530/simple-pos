<script lang="ts">
  type Props = {
    checked?: boolean
    label?: string
    ariaLabel?: string
    class?: string
    onchange?: (checked: boolean) => void
  }

  let { checked = $bindable(false), label = '', ariaLabel = '', class: className = '', onchange }: Props = $props()

  function toggle() {
    const next = !checked
    if (onchange) onchange(next)
    else checked = next
  }
</script>

{#if label}
  <label class={['switch-row', className].filter(Boolean).join(' ')}>
    <button type="button" class="switch" class:on={checked} role="switch" aria-checked={checked} aria-label={ariaLabel || label} onclick={toggle}><i></i></button>
    {label}
  </label>
{:else}
  <button
    type="button"
    class={['switch', className].filter(Boolean).join(' ')}
    class:on={checked}
    role="switch"
    aria-checked={checked}
    aria-label={ariaLabel || undefined}
    onclick={toggle}
  ><i></i></button>
{/if}
