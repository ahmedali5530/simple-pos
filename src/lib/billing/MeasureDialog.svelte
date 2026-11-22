<script lang="ts">
  import { promptSale } from '../barcode'
  import { formatMoney } from '../money'
  import type { PosItem } from '../types'
  import { toast } from '../toast'

  interface Props {
    item: PosItem
    currency: string
    locale: string
    onClose: () => void
    onAdd: (quote: { rate: number; units: number; qty: number; label: string; entry: string }) => boolean
  }

  let { item, currency, locale, onClose, onAdd }: Props = $props()

  let value = $state('')
  let field = $state<HTMLInputElement | null>(null)

  const askingPrice = $derived(item.prompt === 'price')
  const quote = $derived(
    item.prompt === 'quantity' || item.prompt === 'price'
      ? promptSale(item.prompt, item.base_rate ?? 0, item.scale, value)
      : null,
  )

  $effect(() => {
    field?.focus()
  })

  function confirm() {
    const entry = value.trim()
    if (!quote) {
      toast.error(askingPrice ? 'Enter a whole price above 0.' : 'Enter a quantity like 1 or 1.25.')
      return
    }
    onAdd({ ...quote, entry })
  }
</script>

<div class="sheet-backdrop" role="presentation" onclick={onClose}>
  <div
    class="sheet measure-sheet"
    role="dialog"
    aria-labelledby="measure-title"
    tabindex="-1"
    onclick={(event) => event.stopPropagation()}
    onkeydown={(event) => {
      event.stopPropagation()
      if (event.key === 'Escape') onClose()
    }}
  >
    <h2 id="measure-title">{item.name}</h2>
    {#if askingPrice}
      <p class="hint">
        Enter the price to charge. Base rate {formatMoney(item.base_rate ?? 0, currency, locale)}.
        Stock removed is that price × {item.scale} ÷ the base rate.
      </p>
    {:else}
      <p class="hint">
        Enter the quantity or weight. {formatMoney(item.base_rate ?? 0, currency, locale)} for 1,
        and 1 removes {item.scale} from stock.
      </p>
    {/if}
    <input
      bind:this={field}
      class="measure-input"
      inputmode={askingPrice ? 'numeric' : 'decimal'}
      autocomplete="off"
      aria-label={askingPrice ? `Price for ${item.name}` : `Quantity or weight for ${item.name}`}
      placeholder={askingPrice ? 'Price' : 'Quantity or weight'}
      bind:value
      onkeydown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault()
          confirm()
        }
      }}
    />
    {#if quote}
      <p class="hint">
        {formatMoney(quote.rate, currency, locale)}
        {#if quote.qty > 1}× {quote.qty}{/if}
        · takes {quote.units * quote.qty} from stock
      </p>
    {/if}
    <div class="sheet-actions">
      <button type="button" class="ghost" onclick={onClose}>Cancel</button>
      <button type="button" class="save" onclick={confirm}>Add to bill</button>
    </div>
  </div>
</div>
