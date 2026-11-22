<script lang="ts">
  import { getContext, type Snippet } from 'svelte'
  import type { HTMLSelectAttributes } from 'svelte/elements'
  import { FIELD, type FieldContext } from './field'

  type Props = Omit<HTMLSelectAttributes, 'value' | 'class' | 'size'> & {
    value?: string | number
    invalid?: boolean
    describedBy?: string
    class?: string
    children?: Snippet
  }

  let {
    value = $bindable(),
    invalid = false,
    describedBy,
    id,
    class: className = '',
    children,
    ...rest
  }: Props = $props()

  const field = getContext<FieldContext | undefined>(FIELD)
  const controlId = $derived(id ?? field?.id)
  const isInvalid = $derived(invalid || !!field?.invalid)
  const described = $derived(describedBy ?? field?.describedBy)
</script>

<select
  class={['select', className].filter(Boolean).join(' ')}
  id={controlId}
  bind:value
  aria-invalid={isInvalid ? 'true' : undefined}
  aria-describedby={described}
  {...rest}
>
  {@render children?.()}
</select>
