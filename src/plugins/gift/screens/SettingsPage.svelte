<script lang="ts">
  import { onMount } from 'svelte'
  import { formatMoney } from '../../../lib/money'
  import { listGiftCards, voidGiftCard, type GiftCard } from '../api'

  let cards = $state<GiftCard[]>([])
  let error = $state('')
  let busy = $state<string | null>(null)

  async function refresh() {
    error = ''
    try {
      cards = await listGiftCards()
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load gift cards.'
    }
  }

  async function voidCard(code: string) {
    busy = code
    error = ''
    try {
      await voidGiftCard(code)
      await refresh()
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not void that card.'
    } finally {
      busy = null
    }
  }

  onMount(() => {
    void refresh()
  })
</script>

<main class="stage">
  <header>
    <h1>Gift cards</h1>
    <p>Balances and voids. Issue cards from the till as stock-neutral lines.</p>
  </header>
  {#if error}<p class="error">{error}</p>{/if}
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
          <td>{formatMoney(card.balance, 'PKR', 'en-PK')}</td>
          <td>{card.status}</td>
          <td>{card.issued_bill_no || '—'}</td>
          <td>
            {#if card.status === 'active'}
              <button type="button" disabled={busy === card.code} onclick={() => voidCard(card.code)}>
                Void
              </button>
            {/if}
          </td>
        </tr>
      {:else}
        <tr><td colspan="5">No gift cards yet.</td></tr>
      {/each}
    </tbody>
  </table>
</main>

<style>
  .stage {
    padding: 1rem 1.25rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    padding: 0.4rem 0.5rem;
    border-bottom: 1px solid color-mix(in oklab, currentColor 12%, transparent);
  }
  .error {
    color: tomato;
  }
</style>
