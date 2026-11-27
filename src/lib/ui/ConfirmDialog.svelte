<script lang="ts">
  import Button from './Button.svelte'
  import Modal from './Modal.svelte'

  interface Props {
    title: string
    message: string
    confirmLabel?: string
    cancelLabel?: string
    danger?: boolean
    busy?: boolean
    onConfirm: () => void
    onCancel: () => void
  }

  let {
    title,
    message,
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    danger = false,
    busy = false,
    onConfirm,
    onCancel,
  }: Props = $props()
</script>

<Modal title={title} description={message} size="sm" closeOnOverlay={!busy} onClose={onCancel}>
  {#snippet footer()}
    <Button variant="secondary" disabled={busy} onclick={onCancel}>{cancelLabel}</Button>
    <Button variant={danger ? 'danger' : 'primary'} disabled={busy} onclick={onConfirm}>
      {busy ? '…' : confirmLabel}
    </Button>
  {/snippet}
</Modal>
