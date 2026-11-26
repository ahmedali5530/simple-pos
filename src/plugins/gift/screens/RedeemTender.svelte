<script lang="ts">
  import { formatMoney } from '../../../lib/money'
  import type { PosTenderContext } from '../../../platform/slots'
  import { giftBalance } from '../api'

  let { total, currency, locale, tenders, setTenders }: PosTenderContext = $props()

  let code = $state('')
  let amount = $state('')
  let message = $state('')
  let looking = $state(false)

  const giftTenders = $derived(tenders.filter((tender) => tender.method === 'gift.card'))
  const giftTotal = $derived(giftTenders.reduce((sum, tender) => sum + tender.amount, 0))
  const remaining = $derived(Math.max(0, total - tenders.reduce((sum, tender) => sum + tender.amount, 0)))

  async function lookupAndAdd() {
    message = ''
    const trimmed = code.trim().toUpperCase()
    const value = Number(amount || remaining)
    if (!trimmed) {
      message = 'Enter a gift card code.'
      return
    }
    if (!Number.isFinite(value) || value < 1) {
      message = 'Enter an amount to redeem.'
      return
    }
    if (value > remaining) {
      message = `Only ${formatMoney(remaining, currency, locale)} left on this bill.`
      return
    }
    looking = true
    try {
      const card = await giftBalance(trimmed)
      if (card.status !== 'active') {
        message = `Gift card ${card.code} is void.`
        return
      }
      if (value > card.balance) {
        message = `Gift card ${card.code} only has ${formatMoney(card.balance, currency, locale)}.`
        return
      }
      const next = [
        ...tenders.filter((tender) => tender.method !== 'cash'),
        {
          method: 'gift.card',
          amount: Math.trunc(value),
          reference: card.code,
          details: { kind: 'gift.redeem' },
        },
      ]
      const paid = next.reduce((sum, tender) => sum + tender.amount, 0)
      if (paid < total) {
        next.push({ method: 'cash', amount: total - paid })
      }
      setTenders(next)
      code = ''
      amount = ''
      message = `Redeemed ${formatMoney(value, currency, locale)} from ${card.code}.`
    } catch (error) {
      message = error instanceof Error ? error.message : 'Could not look up that card.'
    } finally {
      looking = false
    }
  }

  function clearGift() {
    setTenders(tenders.filter((tender) => tender.method !== 'gift.card'))
    message = ''
  }
</script>

<div class="gift-redeem">
  <strong>Gift card tender</strong>
  <div class="row">
    <input class="input" placeholder="Card code" maxlength="64" bind:value={code} />
    <input
      class="input"
      placeholder={remaining > 0 ? String(remaining) : 'Amount'}
      inputmode="numeric"
      bind:value={amount}
    />
    <button type="button" class="btn" disabled={looking || total < 1} onclick={lookupAndAdd}>
      {looking ? '…' : 'Apply'}
    </button>
    {#if giftTotal > 0}
      <button type="button" class="btn ghost" onclick={clearGift}>Clear</button>
    {/if}
  </div>
  {#if giftTenders.length > 0}
    <ul>
      {#each giftTenders as tender}
        <li>
          {tender.reference}: {formatMoney(tender.amount, currency, locale)}
        </li>
      {/each}
    </ul>
  {/if}
  {#if message}<p class="hint">{message}</p>{/if}
</div>

<style>
  .gift-redeem {
    display: grid;
    gap: 0.35rem;
    margin: 0.4rem 0;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }
  .input {
    min-width: 7rem;
    flex: 1;
  }
  ul {
    margin: 0;
    padding-left: 1.1rem;
  }
  .hint {
    margin: 0;
    font-size: 0.85rem;
    opacity: 0.8;
  }
  .ghost {
    opacity: 0.75;
  }
</style>
