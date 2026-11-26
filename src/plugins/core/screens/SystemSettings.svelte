<script lang="ts">
  import { saveShopProfile } from '../../../lib/api'
  import type { BarcodeRule, ShopSnapshot } from '../../../lib/types'
  import { toast, toastError, toastSaved } from '../../../lib/toast'
  import { Button, Input, Select } from '../../../lib/ui'

  export type SystemView = ShopSnapshot & {
    bill_prefix: string
    barcode_rules: BarcodeRule[]
  }

  interface Props {
    snapshot: SystemView
    onChange: (shop: SystemView) => void
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
  let rules = $state<RuleDraft[]>([])
  let loaded = $state('')
  let saving = $state(false)

  const key = $derived(
    `${snapshot.shop_name}|${snapshot.bill_prefix}|${snapshot.currency}|${snapshot.locale}|${snapshot.barcode_rules
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
    rules = snapshot.barcode_rules.map((rule) => ({
      prefix: rule.prefix,
      mode: rule.mode === 'weight' ? 'weight' : 'price',
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
        await saveShopProfile({
          name: name.trim(),
          bill_prefix: billPrefix.trim().toUpperCase(),
          currency: currency.trim().toUpperCase(),
          locale: locale.trim(),
          barcode_rules: barcodeRules,
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
  <p class="hint">Shop name and money are core settings. Bill prefix and barcode rules belong to the till.</p>
  <div class="system-form">
    <label>
      Shop name
      <Input bind:value={name} />
    </label>
    <label>
      Bill prefix
      <Input bind:value={billPrefix} maxlength="8" />
    </label>
    <label>
      Currency
      <Input bind:value={currency} maxlength="3" />
    </label>
    <label>
      Locale
      <Input bind:value={locale} />
    </label>
    <div class="option-block">
      <h3>Barcode rules</h3>
      <p class="hint">Prefix 21 reads a weight. Prefix 22 reads a price. Scale is how many stock units match a value of 1.</p>
      {#each rules as rule, index (index)}
        <div class="rule-edit">
          <label>
            Prefix
            <Input inputmode="numeric" bind:value={rule.prefix} />
          </label>
          <label>
            Mode
            <Select bind:value={rule.mode}>
              <option value="weight">Weight</option>
              <option value="price">Price</option>
            </Select>
          </label>
          <label>
            Scale
            <Input inputmode="numeric" bind:value={rule.scale} />
          </label>
          <Button variant="link" class="text-danger" onclick={() => removeRule(index)}>Remove</Button>
        </div>
      {/each}
      <Button variant="secondary" onclick={addRule}>Add barcode rule</Button>
    </div>
    <div class="sheet-actions">
      <Button disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save'}</Button>
    </div>
  </div>
</div>
