<script lang="ts">
  import { getContext } from 'svelte'
  import type { HTMLInputAttributes } from 'svelte/elements'
  import { FIELD, type FieldContext } from './field'

  type Props = Omit<HTMLInputAttributes, 'value' | 'class' | 'size'> & {
    value?: string | number
    invalid?: boolean
    describedBy?: string
    element?: HTMLInputElement | null
    class?: string
  }

  let {
    value = $bindable(),
    invalid = false,
    describedBy,
    element = $bindable<HTMLInputElement | null>(null),
    id,
    class: className = '',
    ...rest
  }: Props = $props()

  const field = getContext<FieldContext | undefined>(FIELD)
  const controlId = $derived(id ?? field?.id)
  const isInvalid = $derived(invalid || !!field?.invalid)
  const described = $derived(describedBy ?? field?.describedBy)
</script>

<input
  bind:this={element}
  class={['input', className].filter(Boolean).join(' ')}
  id={controlId}
  bind:value
  aria-invalid={isInvalid ? 'true' : undefined}
  aria-describedby={described}
  {...rest}
/>
