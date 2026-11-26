<script lang="ts">
  import { portal } from '../../../lib/ui/portal'

  interface Props {
    tone: 'error' | 'warning'
    message: string
    onClose: () => void
  }

  let { tone, message, onClose }: Props = $props()
</script>

<div class="ui-overlay" data-state="open" role="presentation" use:portal>
  <button type="button" class="attention-hit" aria-label="Close" onclick={onClose}></button>
  <div
    class="ui-modal"
    data-size="sm"
    data-state="open"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="attention-title"
    tabindex="-1"
  >
    <div class="ui-modal-header">
      <div>
        <p class="ui-modal-title" id="attention-title">{tone === 'error' ? 'Error' : 'Warning'}</p>
        <p class="ui-modal-desc">{message}</p>
      </div>
    </div>
    <div class="ui-modal-footer">
      <button type="button" class="ui-btn" data-variant="secondary" onclick={onClose}>OK</button>
    </div>
  </div>
</div>

<style>
  .attention-hit {
    position: absolute;
    inset: 0;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: pointer;
  }

  .ui-modal {
    position: relative;
    z-index: 1;
  }

  .ui-modal[data-size='sm'] .ui-modal-title {
    color: var(--ui-danger);
  }
</style>
