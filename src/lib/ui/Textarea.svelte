<script lang="ts">
  import { getContext } from 'svelte'
  import type { HTMLTextareaAttributes } from 'svelte/elements'
  import { FIELD, type FieldContext } from './field'

  type Props = Omit<HTMLTextareaAttributes, 'value' | 'class'> & {
    value?: string
    invalid?: boolean
    describedBy?: string
    class?: string
  }

  let {
    value = $bindable(),
    invalid = false,
    describedBy,
    id,
    class: className = '',
    ...rest
  }: Props = $props()

  const field = getContext<FieldContext | undefined>(FIELD)
  const controlId = $derived(id ?? field?.id)
  const isInvalid = $derived(invalid || !!field?.invalid)
  const described = $derived(describedBy ?? field?.describedBy)
</script>

<textarea
  class={['textarea', className].filter(Boolean).join(' ')}
  id={controlId}
  bind:value
  aria-invalid={isInvalid ? 'true' : undefined}
  aria-describedby={described}
  {...rest}
></textarea>
