<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLAnchorAttributes, HTMLButtonAttributes } from 'svelte/elements'

  type Variant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'success' | 'link'
  type Size = 'sm' | 'md' | 'lg' | 'icon'

  type Props = {
    variant?: Variant
    size?: Size
    loading?: boolean
    href?: string
    class?: string
    children?: Snippet
  } & Omit<HTMLButtonAttributes, 'class' | 'children'> &
    Omit<HTMLAnchorAttributes, 'class' | 'href' | 'children'>

  let {
    variant = 'primary',
    size = 'md',
    loading = false,
    href,
    class: className = '',
    children,
    type = 'button',
    disabled = false,
    ...rest
  }: Props = $props()

  const classes = $derived(
    ['btn', `btn-${variant}`, size === 'md' ? '' : `btn-${size}`, className].filter(Boolean).join(' '),
  )
</script>

{#if href}
  <a class={classes} {href} aria-busy={loading || undefined} aria-disabled={disabled || loading || undefined} {...rest}>
    {@render children?.()}
  </a>
{:else}
  <button class={classes} {type} disabled={disabled || loading} aria-busy={loading || undefined} {...rest}>
    {@render children?.()}
  </button>
{/if}
