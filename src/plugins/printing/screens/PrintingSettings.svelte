<script lang="ts">
  import { onMount } from 'svelte'
  import { loadPrinterSettings, savePrinter } from '../../../lib/api'
  import { toast, toastError, toastSaved } from '../../../lib/toast'
  import { Button, Input, Select, Switch } from '../../../lib/ui'

  let printEnabled = $state(false)
  let printerKind = $state('network')
  let printerTarget = $state('')
  let printerPort = $state('9100')
  let saving = $state(false)

  onMount(() => {
    loadPrinterSettings().then((settings) => {
      printEnabled = settings.enabled
      printerKind = settings.kind
      printerTarget = settings.target
      printerPort = String(settings.port)
    }).catch((error: unknown) => toastError(error, 'Could not load printer settings.'))
  })

  async function save() {
    if (!['network', 'usb', 'serial'].includes(printerKind)) {
      toast.error('The printer is network, usb, or serial.')
      return
    }
    if (!/^\d+$/.test(printerPort) || Number(printerPort) < 1) {
      toast.error('The printer port is not a usable number.')
      return
    }
    if (printEnabled && printerKind !== 'usb' && !printerTarget.trim()) {
      toast.error('Enter the printer address.')
      return
    }
    saving = true
    try {
      const settings = await savePrinter({
          print_enabled: printEnabled,
          printer_kind: printerKind,
          printer_target: printerTarget.trim(),
          printer_port: Number(printerPort),
      })
      printEnabled = settings.enabled
      printerKind = settings.kind
      printerTarget = settings.target
      printerPort = String(settings.port)
      toastSaved('Printer saved.')
    } catch (error) {
      toastError(error, 'Could not save the printer.')
    } finally {
      saving = false
    }
  }
</script>

<div class="settings-page">
  <h2>Printing</h2>
  <p class="hint">A posted sale prints a cash receipt on this computer. The sale is kept if the printer does not answer.</p>
  <div class="system-form">
    <Switch bind:checked={printEnabled} label="Print a receipt after each sale" />
    <label>
      Connection
      <Select bind:value={printerKind}>
        <option value="network">Network</option>
        <option value="usb">USB</option>
        <option value="serial">Serial</option>
      </Select>
    </label>
    {#if printerKind !== 'usb'}
      <label>
        {printerKind === 'serial' ? 'Serial port' : 'Address'}
        <Input bind:value={printerTarget} placeholder={printerKind === 'serial' ? '/dev/ttyUSB0' : '192.168.1.100'} />
      </label>
    {:else}
      <label>
        USB id
        <Input bind:value={printerTarget} placeholder="0x04b8:0x0e28" />
      </label>
    {/if}
    <label>
      {printerKind === 'serial' ? 'Baud' : 'Port'}
      <Input inputmode="numeric" bind:value={printerPort} />
    </label>
    <div class="sheet-actions">
      <Button disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save'}</Button>
    </div>
  </div>
</div>
