<script lang="ts">
  import { setContext, type Snippet } from 'svelte'
  import { FIELD, type FieldContext } from './field'

  type Props = {
    label?: string
    hint?: string
    error?: string
    class?: string
    children?: Snippet
  }

  let { label = '', hint = '', error = '', class: className = '', children }: Props = $props()

  const uid = $props.id()
  const id = `field-${uid}`
  const field: FieldContext = $state({ id, invalid: false, describedBy: undefined })

  $effect.pre(() => {
    field.invalid = error.trim().length > 0
    const ids = [hint ? `${id}-hint` : '', error ? `${id}-error` : ''].filter(Boolean)
    field.describedBy = ids.length > 0 ? ids.join(' ') : undefined
  })

  setContext(FIELD, field)
</script>

<div class={['field', className].filter(Boolean).join(' ')}>
  {#if label}
    <label class="label" for={id}>{label}</label>
  {/if}
  {@render children?.()}
  {#if error}
    <p class="field-error" id="{id}-error">{error}</p>
  {/if}
  {#if hint}
    <p class="hint" id="{id}-hint">{hint}</p>
  {/if}
</div>
