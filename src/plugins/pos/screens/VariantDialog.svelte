<script lang="ts">
  import { formatMoney } from '../../../lib/money'
  import type { CartLine, ItemVariant, PosItem, PosSection } from '../../../lib/types'
  import { Button } from '../../../lib/ui'
  import { portal } from '../../../lib/ui/portal'
  import Slot from '../../../platform/Slot.svelte'
  import type { AddPosLine } from '../../../platform/slots'

  interface Props {
    item: PosItem
    section: PosSection
    lines: CartLine[]
    available: number
    currency: string
    locale: string
    onClose: () => void
    onAdd: (picks: { variant: ItemVariant; qty: number }[]) => void
    onExternalLine: AddPosLine
  }

  let {
    item,
    section,
    lines,
    available,
    currency,
    locale,
    onClose,
    onAdd,
    onExternalLine,
  }: Props = $props()

  let counts = $state<Record<number, number>>({})

  const planned = $derived(
    item.variants.reduce((sum, variant) => sum + (counts[variant.id] ?? 0) * variant.units, 0),
  )

  function money(amount: number) {
    return formatMoney(amount, currency, locale)
  }

  function roomFor(variant: ItemVariant) {
    const current = counts[variant.id] ?? 0
    const others = planned - current * variant.units
    return Math.max(0, Math.floor((available - others) / variant.units))
  }

  function setCount(variant: ItemVariant, qty: number) {
    if (qty < 0) return counts[variant.id] ?? 0
    const accepted = Math.min(qty, roomFor(variant))
    counts = { ...counts, [variant.id]: accepted }
    return accepted
  }

  function onQtyInput(variant: ItemVariant, input: HTMLInputElement) {
    const raw = input.value.trim()
    if (raw === '') {
      setCount(variant, 0)
      return
    }
    if (!/^\d+$/.test(raw)) {
      input.value = String(counts[variant.id] ?? 0)
      return
    }
    input.value = String(setCount(variant, Number(raw)))
  }

  function focusFirst(node: HTMLInputElement, first: boolean) {
    if (!first) return
    node.focus()
    node.select()
  }

  function confirm() {
    const picks = item.variants
      .filter((variant) => (counts[variant.id] ?? 0) > 0)
      .map((variant) => ({ variant, qty: counts[variant.id] ?? 0 }))
    if (picks.length === 0) return
    onAdd(picks)
  }
</script>

<div
  class="ui-overlay"
  data-state="open"
  data-density="touch"
  role="presentation"
  use:portal
  onclick={onClose}
  onkeydown={(event) => {
    if (event.key === 'Escape') onClose()
  }}
>
  <div
    class="ui-modal"
    data-size="lg"
    data-state="open"
    role="dialog"
    aria-labelledby="variant-title"
    tabindex="-1"
    onclick={(event) => event.stopPropagation()}
    onkeydown={(event) => {
      event.stopPropagation()
      if (event.key === 'Escape') onClose()
    }}
  >
    <div class="ui-modal-header">
      <div>
        <h2 class="ui-modal-title" id="variant-title">{item.name}</h2>
        {#if item.variants.length > 0}
          <p class="ui-modal-desc">
            {available} in stock. Choose one option, or several. Each option has its own rate.
          </p>
        {:else}
          <p class="ui-modal-desc">Choose an IMEI for this item. Each device is one unit.</p>
        {/if}
      </div>
    </div>
    <div class="ui-modal-body">
      {#each item.variants as variant, index (variant.id)}
        <div class="variant-row">
          <div>
            <strong>{variant.name}</strong>
            <div class="ui-hint">
              {money(variant.rate)}
              {#if variant.units > 1} · takes {variant.units} from stock{/if}
            </div>
          </div>
          <span class="ui-stepper">
            <button
              type="button"
              aria-label="Decrease {variant.name}"
              disabled={(counts[variant.id] ?? 0) === 0}
              onclick={() => setCount(variant, (counts[variant.id] ?? 0) - 1)}
            >-</button>
            <input
              inputmode="numeric"
              aria-label="Quantity for {variant.name}"
              value={counts[variant.id] ?? 0}
              use:focusFirst={index === 0}
              oninput={(event) => onQtyInput(variant, event.currentTarget)}
              onfocus={(event) => event.currentTarget.select()}
              onkeydown={(event) => {
                if (event.key !== 'Enter') return
                event.preventDefault()
                confirm()
              }}
            />
            <button
              type="button"
              aria-label="Increase {variant.name}"
              disabled={planned + variant.units > available}
              onclick={() => setCount(variant, (counts[variant.id] ?? 0) + 1)}
            >+</button>
          </span>
        </div>
      {/each}
      <Slot
        name="pos.item-options"
        context={{
          item,
          section,
          lines,
          addLine: onExternalLine,
          close: onClose,
          currency,
          locale,
        }}
      />
    </div>
    <div class="ui-modal-footer">
      <Button variant="secondary" onclick={onClose}>
        {item.variants.length > 0 ? 'Cancel' : 'Close'}
      </Button>
      {#if item.variants.length > 0}
        <Button disabled={planned === 0} onclick={confirm}>Add to bill</Button>
      {/if}
    </div>
  </div>
</div>
