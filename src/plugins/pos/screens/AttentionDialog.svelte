<script lang="ts">
  interface Props {
    tone: 'error' | 'warning'
    message: string
    onClose: () => void
  }

  let { tone, message, onClose }: Props = $props()
</script>

<div class="attention-backdrop">
  <button type="button" class="attention-overlay" aria-label="Close" onclick={onClose}></button>
  <div
    class="attention {tone}"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="attention-title"
    tabindex="-1"
  >
    <p class="kicker" id="attention-title">{tone === 'error' ? 'Error' : 'Warning'}</p>
    <p>{message}</p>
  </div>
</div>
