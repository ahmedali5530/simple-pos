<script lang="ts">
  import { saveSettings } from '../api'
  import type { BarcodeRule, PosSnapshot } from '../types'
  import { toast, toastError, toastSaved } from '../toast'

  interface Props {
    snapshot: PosSnapshot
    onChange: (pos: PosSnapshot) => void
  }

  let { snapshot, onChange }: Props = $props()

  interface RuleDraft {
    prefix: string
    mode: 'weight' | 'price'
    scale: string
  }

  let name = $state('')
  let billPrefix = $state('')
  let currency = $state('')
  let locale = $state('')
  let printEnabled = $state(false)
  let printerKind = $state('network')
  let printerTarget = $state('')
  let printerPort = $state('9100')
  let rules = $state<RuleDraft[]>([])
  let loaded = $state('')
  let saving = $state(false)

  const key = $derived(
    `${snapshot.shop_name}|${snapshot.bill_prefix}|${snapshot.currency}|${snapshot.locale}|${snapshot.printer.enabled}|${snapshot.printer.kind}|${snapshot.printer.target}|${snapshot.printer.port}|${snapshot.barcode_rules
      .map((rule) => `${rule.prefix}:${rule.mode}:${rule.scale}`)
      .join(',')}`,
  )

  $effect(() => {
    if (loaded === key) return
    loaded = key
    name = snapshot.shop_name
    billPrefix = snapshot.bill_prefix
    currency = snapshot.currency
    locale = snapshot.locale
    printEnabled = snapshot.printer.enabled
    printerKind = snapshot.printer.kind
    printerTarget = snapshot.printer.target
    printerPort = String(snapshot.printer.port)
    rules = snapshot.barcode_rules.map((rule) => ({
      prefix: rule.prefix,
      mode: rule.mode,
      scale: String(rule.scale),
    }))
  })

  function addRule() {
    rules = [...rules, { prefix: '', mode: 'price', scale: '1' }]
  }

  function removeRule(index: number) {
    rules = rules.filter((_, ruleIndex) => ruleIndex !== index)
  }

  async function save() {
    if (!name.trim()) {
      toast.error('Enter a shop name.')
      return
    }
    if (!/^[A-Za-z0-9]{1,8}$/.test(billPrefix.trim())) {
      toast.error('The bill prefix is 1 to 8 letters or digits.')
      return
    }
    if (!/^[A-Za-z]{3}$/.test(currency.trim())) {
      toast.error('Currency is a 3-letter code, such as PKR.')
      return
    }
    if (!locale.trim()) {
      toast.error('Enter a locale, such as en-PK.')
      return
    }
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
    const barcodeRules: BarcodeRule[] = []
    for (const rule of rules) {
      const prefix = rule.prefix.trim()
      if (!/^\d{1,4}$/.test(prefix)) {
        toast.error('A barcode prefix is 1 to 4 digits.')
        return
      }
      if (!/^\d+$/.test(rule.scale) || Number(rule.scale) < 1) {
        toast.error('Barcode scale must be at least 1.')
        return
      }
      barcodeRules.push({ prefix, mode: rule.mode, scale: Number(rule.scale) })
    }
    saving = true
    try {
      onChange(
        await saveSettings({
          name: name.trim(),
          bill_prefix: billPrefix.trim().toUpperCase(),
          currency: currency.trim().toUpperCase(),
          locale: locale.trim(),
          barcode_rules: barcodeRules,
          print_enabled: printEnabled,
          printer_kind: printerKind,
          printer_target: printerTarget.trim(),
          printer_port: Number(printerPort),
        }),
      )
      toastSaved('Settings saved.')
    } catch (error) {
      toastError(error, 'Could not save the settings.')
    } finally {
      saving = false
    }
  }
</script>

<div class="settings-page">
  <h2>System</h2>
  <p class="hint">Shop name, bill prefix, and money. Item rates stay as stored whole units when the currency code changes.</p>
  <div class="system-form">
    <label>
      Shop name
      <input bind:value={name} />
    </label>
    <label>
      Bill prefix
      <input bind:value={billPrefix} maxlength="8" />
    </label>
    <label>
      Currency
      <input bind:value={currency} maxlength="3" />
    </label>
    <label>
      Locale
      <input bind:value={locale} />
    </label>
    <div class="option-block">
      <h3>Receipt printer</h3>
      <p class="hint">A posted sale prints a cash receipt on this computer. The sale is kept if the printer does not answer.</p>
      <div class="switch-row">
        <button
          type="button"
          class="switch"
          class:on={printEnabled}
          role="switch"
          aria-checked={printEnabled}
          aria-label="Print a receipt after each sale"
          onclick={() => (printEnabled = !printEnabled)}
        ><i></i></button>
        Print a receipt after each sale
      </div>
      <label>
        Connection
        <select bind:value={printerKind}>
          <option value="network">Network</option>
          <option value="usb">USB</option>
          <option value="serial">Serial</option>
        </select>
      </label>
      {#if printerKind !== 'usb'}
        <label>
          {printerKind === 'serial' ? 'Serial port' : 'Address'}
          <input bind:value={printerTarget} placeholder={printerKind === 'serial' ? '/dev/ttyUSB0' : '192.168.1.100'} />
        </label>
      {:else}
        <label>
          USB id
          <input bind:value={printerTarget} placeholder="0x04b8:0x0e28" />
        </label>
      {/if}
      <label>
        {printerKind === 'serial' ? 'Baud' : 'Port'}
        <input inputmode="numeric" bind:value={printerPort} />
      </label>
    </div>
    <div class="option-block">
      <h3>Barcode rules</h3>
      <p class="hint">Prefix 21 reads a weight. Prefix 22 reads a price. Scale is how many stock units match a value of 1.</p>
      {#each rules as rule, index (index)}
        <div class="rule-edit">
          <label>
            Prefix
            <input inputmode="numeric" bind:value={rule.prefix} />
          </label>
          <label>
            Mode
            <select bind:value={rule.mode}>
              <option value="weight">Weight</option>
              <option value="price">Price</option>
            </select>
          </label>
          <label>
            Scale
            <input inputmode="numeric" bind:value={rule.scale} />
          </label>
          <button type="button" class="link" onclick={() => removeRule(index)}>Remove</button>
        </div>
      {/each}
      <button type="button" class="ghost light" onclick={addRule}>Add barcode rule</button>
    </div>
    <div class="sheet-actions">
      <button type="button" class="save" disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save'}</button>
    </div>
  </div>
</div>
