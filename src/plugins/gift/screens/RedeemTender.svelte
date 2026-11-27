<script lang="ts">
  import { formatMoney } from '../../../lib/money'
  import { Button, ConfirmDialog, Input, Modal } from '../../../lib/ui'
  import type { PosTenderContext } from '../../../platform/slots'
  import { giftBalance, type GiftBalance } from '../api'

  let { total, currency, locale, tenders, setTenders }: PosTenderContext = $props()

  let open = $state(false)
  let confirmClear = $state(false)
  let code = $state('')
  let amount = $state('')
  let message = $state('')
  let looking = $state(false)
  let card = $state<GiftBalance | null>(null)

  const giftTenders = $derived(tenders.filter((tender) => tender.method === 'gift.card'))
  const giftTotal = $derived(giftTenders.reduce((sum, tender) => sum + tender.amount, 0))
  const giftCode = $derived(giftTenders[0]?.reference ?? '')
  const remaining = $derived(
    Math.max(0, total - tenders.reduce((sum, tender) => sum + tender.amount, 0)),
  )

  function money(value: number) {
    return formatMoney(value, currency, locale)
  }

  function openModal() {
    message = ''
    if (giftCode) code = giftCode
    open = true
  }

  function closeModal() {
    open = false
    message = ''
  }

  async function lookup() {
    message = ''
    card = null
    const trimmed = code.trim().toUpperCase()
    if (!trimmed) {
      message = 'Enter the gift card code.'
      return
    }
    looking = true
    try {
      const found = await giftBalance(trimmed)
      card = found
      code = found.code
      if (found.status !== 'active') {
        message = `Card ${found.code} is void.`
        return
      }
      const usable = Math.min(found.balance, remaining + giftTotal || found.balance)
      if (!amount) amount = String(usable)
      message = `Balance ${money(found.balance)}.`
    } catch (error) {
      message = error instanceof Error ? error.message : 'Could not look up that card.'
    } finally {
      looking = false
    }
  }

  async function payWithCard() {
    message = ''
    if (!card || card.status !== 'active') {
      await lookup()
      if (!card || card.status !== 'active') return
    }
    const value = Number(amount || remaining + giftTotal)
    if (!Number.isFinite(value) || value < 1) {
      message = 'Enter how much to take from the card.'
      return
    }
    if (value > remaining + giftTotal) {
      message = `Only ${money(remaining + giftTotal)} left on this bill.`
      return
    }
    if (value > card.balance) {
      message = `Card only has ${money(card.balance)}.`
      return
    }
    const next = [
      ...tenders.filter((tender) => tender.method !== 'cash' && tender.method !== 'gift.card'),
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
    closeModal()
  }

  function clearGift() {
    setTenders(tenders.filter((tender) => tender.method !== 'gift.card'))
    card = null
    amount = ''
    code = ''
    confirmClear = false
  }
</script>

<div class="gift-summary">
  {#if giftTotal > 0}
    <div class="applied">
      <div>
        <strong>Gift card</strong>
        <p>{giftCode} · {money(giftTotal)}</p>
      </div>
      <div class="actions">
        <Button variant="secondary" size="sm" onclick={openModal}>Change</Button>
        <Button variant="ghost" size="sm" class="text-danger" onclick={() => (confirmClear = true)}>
          Clear
        </Button>
      </div>
    </div>
  {:else}
    <Button variant="secondary" data-block="" disabled={total < 1} onclick={openModal}>
      Pay with gift card
    </Button>
  {/if}
</div>

{#if open}
  <Modal
    title="Pay with gift card"
    description="Look up the code, then apply some or all of its balance to this bill."
    size="sm"
    onClose={closeModal}
  >
    <div class="modal-form">
      <label class="field">
        <span>Card code</span>
        <div class="row">
          <Input
            placeholder="Scan or type code"
            maxlength={64}
            bind:value={code}
            onkeydown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault()
                void lookup()
              }
            }}
          />
          <Button variant="secondary" disabled={looking} onclick={() => void lookup()}>
            {looking ? '…' : 'Check'}
          </Button>
        </div>
      </label>

      {#if card && card.status === 'active'}
        <p class="balance">Available {money(card.balance)}</p>
        <label class="field">
          <span>Amount to use</span>
          <Input
            inputmode="numeric"
            placeholder={String(Math.min(card.balance, remaining + giftTotal || card.balance))}
            bind:value={amount}
          />
        </label>
      {/if}

      {#if message}<p class="hint">{message}</p>{/if}
    </div>

    {#snippet footer()}
      <Button variant="secondary" onclick={closeModal}>Cancel</Button>
      <Button disabled={total < 1 || looking} onclick={() => void payWithCard()}>
        Use on bill
      </Button>
    {/snippet}
  </Modal>
{/if}

{#if confirmClear}
  <ConfirmDialog
    title="Clear gift card payment?"
    message={`Remove ${money(giftTotal)} from ${giftCode || 'this gift card'} on the bill?`}
    confirmLabel="Clear"
    danger
    onCancel={() => (confirmClear = false)}
    onConfirm={clearGift}
  />
{/if}

<style>
  .gift-summary {
    margin: 0.5rem 0;
  }
  .applied {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.65rem 0.75rem;
    border: 1px solid var(--ui-border);
    border-radius: var(--ui-radius);
    background: var(--ui-surface);
  }
  .applied p {
    margin: 0.15rem 0 0;
    color: var(--ui-text-muted);
    font-size: 0.85rem;
  }
  .actions {
    display: flex;
    gap: 0.35rem;
    flex: none;
  }
  .modal-form {
    display: grid;
    gap: 0.75rem;
  }
  .field {
    display: grid;
    gap: 0.3rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--ui-text-muted);
  }
  .row {
    display: flex;
    gap: 0.4rem;
  }
  .balance {
    margin: 0;
    font-weight: 600;
  }
  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--ui-text-muted);
  }
</style>
