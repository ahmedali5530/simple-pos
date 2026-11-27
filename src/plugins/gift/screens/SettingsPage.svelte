<script lang="ts">
  import { onMount } from 'svelte'
  import { formatMoney } from '../../../lib/money'
  import { Button, ConfirmDialog, Input } from '../../../lib/ui'
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import {
    issueGiftCard,
    listGiftCards,
    voidGiftCard,
    type GiftCard,
  } from '../api'

  let cards = $state<GiftCard[]>([])
  let error = $state('')
  let saved = $state('')
  let busy = $state(false)
  let code = $state('')
  let amount = $state('')
  let pendingVoid = $state<GiftCard | null>(null)
  let voiding = $state(false)

  function generateCode() {
    const chunk = Math.random().toString(36).slice(2, 8).toUpperCase()
    code = `GC-${chunk}`
  }

  async function refresh() {
    error = ''
    try {
      cards = await listGiftCards()
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load gift cards.'
    }
  }

  async function createCard() {
    error = ''
    saved = ''
    const trimmed = code.trim().toUpperCase()
    const value = Number(amount)
    if (!trimmed) {
      error = 'Enter a card code, or generate one.'
      return
    }
    if (!Number.isFinite(value) || value < 1) {
      error = 'Enter the card balance amount.'
      return
    }
    busy = true
    try {
      await issueGiftCard(trimmed, Math.trunc(value))
      code = ''
      amount = ''
      saved = `Created ${trimmed}.`
      await refresh()
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not create that card.'
    } finally {
      busy = false
    }
  }

  async function confirmVoid() {
    if (!pendingVoid) return
    voiding = true
    error = ''
    try {
      await voidGiftCard(pendingVoid.code)
      pendingVoid = null
      await refresh()
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not void that card.'
    } finally {
      voiding = false
    }
  }

  onMount(() => {
    void refresh()
  })
</script>

<ShopLocaleGate>
  {#snippet children(shop)}
    <main class="stage">
      <header>
        <h1>Gift cards</h1>
        <p>
          Create cards here with a code and balance. On the till, use
          <strong>Pay with gift card</strong> to redeem. Voiding clears the balance permanently.
        </p>
      </header>

      <section class="create">
        <h2>Create card</h2>
        <div class="row">
          <label>
            <span>Code</span>
            <div class="code-row">
              <Input placeholder="e.g. GC-AB12CD" maxlength={64} bind:value={code} />
              <Button variant="secondary" onclick={generateCode}>Generate</Button>
            </div>
          </label>
          <label>
            <span>Balance ({shop.currency})</span>
            <Input inputmode="numeric" placeholder="Amount" bind:value={amount} />
          </label>
          <Button disabled={busy} onclick={createCard}>{busy ? 'Saving…' : 'Create card'}</Button>
        </div>
      </section>

      {#if error}<p class="error">{error}</p>{/if}
      {#if saved}<p class="ok">{saved}</p>{/if}

      <section class="list">
        <h2>Cards</h2>
        <table>
          <thead>
            <tr>
              <th>Code</th>
              <th>Balance</th>
              <th>Status</th>
              <th>Issued</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each cards as card}
              <tr>
                <td>{card.code}</td>
                <td>{formatMoney(card.balance, shop.currency, shop.locale)}</td>
                <td>{card.status}</td>
                <td>{card.issued_bill_no || 'Settings'}</td>
                <td class="actions">
                  {#if card.status === 'active'}
                    <Button variant="danger" size="sm" onclick={() => (pendingVoid = card)}>
                      Void
                    </Button>
                  {/if}
                </td>
              </tr>
            {:else}
              <tr><td colspan="5">No gift cards yet.</td></tr>
            {/each}
          </tbody>
        </table>
      </section>
    </main>

    {#if pendingVoid}
      <ConfirmDialog
        title="Void gift card?"
        message={`Void ${pendingVoid.code}? Its remaining ${formatMoney(pendingVoid.balance, shop.currency, shop.locale)} will be cleared and the card cannot be used again.`}
        confirmLabel="Void card"
        danger
        busy={voiding}
        onCancel={() => {
          if (!voiding) pendingVoid = null
        }}
        onConfirm={() => void confirmVoid()}
      />
    {/if}
  {/snippet}
</ShopLocaleGate>

<style>
  .stage {
    padding: 1rem 1.25rem;
    display: grid;
    gap: 1.25rem;
  }
  header h1,
  .create h2,
  .list h2 {
    margin: 0;
    font-size: 1.2rem;
    font-weight: 600;
  }
  header p {
    margin: 0.4rem 0 0;
    color: var(--ui-text-muted);
    max-width: 42rem;
    line-height: 1.4;
  }
  .create,
  .list {
    display: grid;
    gap: 0.75rem;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    align-items: end;
  }
  label {
    display: grid;
    gap: 0.3rem;
    font-size: 0.75rem;
    color: var(--ui-text-muted);
    min-width: 12rem;
    flex: 1;
  }
  .code-row {
    display: flex;
    gap: 0.4rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    padding: 0.55rem 0.5rem;
    border-bottom: 1px solid var(--ui-border);
  }
  .actions {
    text-align: right;
  }
  .error {
    margin: 0;
    color: var(--ui-danger);
  }
  .ok {
    margin: 0;
    color: var(--ui-success);
  }
</style>
