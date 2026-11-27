<script lang="ts">
  import type { Snippet } from 'svelte'
  import { portal } from './portal'

  interface Props {
    title: string
    description?: string
    size?: 'sm' | 'md' | 'lg'
    closeOnOverlay?: boolean
    onClose: () => void
    children?: Snippet
    footer?: Snippet
  }

  let {
    title,
    description = '',
    size = 'md',
    closeOnOverlay = true,
    onClose,
    children,
    footer,
  }: Props = $props()

  const titleId = `modal-title-${Math.random().toString(36).slice(2, 9)}`
  const descId = description ? `modal-desc-${Math.random().toString(36).slice(2, 9)}` : undefined
</script>

<div
  class="ui-overlay"
  data-state="open"
  role="presentation"
  use:portal
  onclick={() => {
    if (closeOnOverlay) onClose()
  }}
  onkeydown={(event) => {
    if (event.key === 'Escape') onClose()
  }}
>
  <div
    class="ui-modal"
    data-size={size === 'md' ? undefined : size}
    data-state="open"
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    aria-describedby={descId}
    tabindex="-1"
    onclick={(event) => event.stopPropagation()}
  >
    <div class="ui-modal-header">
      <div>
        <h2 class="ui-modal-title" id={titleId}>{title}</h2>
        {#if description}
          <p class="ui-modal-desc" id={descId}>{description}</p>
        {/if}
      </div>
      <button type="button" class="ui-btn close" data-variant="ghost" data-size="sm" aria-label="Close" onclick={onClose}>
        ×
      </button>
    </div>
    {#if children}
      <div class="ui-modal-body">
        {@render children()}
      </div>
    {/if}
    {#if footer}
      <div class="ui-modal-footer">
        {@render footer()}
      </div>
    {/if}
  </div>
</div>

<style>
  .ui-modal {
    position: relative;
    z-index: 1;
  }
  .close {
    flex: none;
    min-width: 2.25rem;
    font-size: 1.35rem;
    line-height: 1;
  }
</style>
