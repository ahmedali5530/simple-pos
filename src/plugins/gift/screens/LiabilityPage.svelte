<script lang="ts">
  import { onMount } from 'svelte'
  import { formatMoney } from '../../../lib/money'
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import { giftLiability, type GiftLiability } from '../api'

  let report = $state<GiftLiability | null>(null)
  let error = $state('')

  onMount(() => {
    giftLiability()
      .then((value) => (report = value))
      .catch((err: unknown) => {
        error = err instanceof Error ? err.message : 'Could not load gift liability.'
      })
  })
</script>

<ShopLocaleGate>
  {#snippet children(shop)}
    <main class="stage">
      <header>
        <h1>Gift liability</h1>
        <p>Outstanding balances on active gift cards.</p>
      </header>

      {#if error}<p class="error">{error}</p>{/if}

      {#if report}
        <div class="totals">
          <p>
            <span>Total issued</span>
            <strong>{formatMoney(report.issued, shop.currency, shop.locale)}</strong>
          </p>
          <p>
            <span>Balance outstanding</span>
            <strong>{formatMoney(report.outstanding, shop.currency, shop.locale)}</strong>
          </p>
          <p>
            <span>Cards</span>
            <strong>{report.cards}</strong>
          </p>
        </div>

        <table>
          <thead>
            <tr>
              <th>Code</th>
              <th>Issued</th>
              <th>Balance</th>
              <th>Issued on</th>
            </tr>
          </thead>
          <tbody>
            {#each report.rows as row}
              <tr>
                <td>{row.code}</td>
                <td>{formatMoney(row.issued, shop.currency, shop.locale)}</td>
                <td>{formatMoney(row.balance, shop.currency, shop.locale)}</td>
                <td>{row.issued_at ? row.issued_at.slice(0, 10) : row.issued_bill_no || '—'}</td>
              </tr>
            {:else}
              <tr><td colspan="4">No outstanding liability.</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </main>
  {/snippet}
</ShopLocaleGate>

<style>
  .stage {
    padding: 1rem 1.25rem;
    display: grid;
    gap: 0.85rem;
  }
  header h1 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 600;
  }
  header p {
    margin: 0.35rem 0 0;
    color: var(--ui-text-muted);
  }
  .totals {
    display: flex;
    flex-wrap: wrap;
    gap: 1.25rem 2rem;
  }
  .totals p {
    margin: 0;
    display: grid;
    gap: 0.2rem;
  }
  .totals span {
    color: var(--ui-text-muted);
    font-size: 0.75rem;
  }
  .totals strong {
    font-size: 1.1rem;
    font-weight: 600;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    padding: 0.45rem 0.5rem;
    border-bottom: 1px solid var(--ui-border);
  }
  .error {
    margin: 0;
    color: var(--ui-danger, tomato);
  }
</style>
