<script lang="ts">
  import { onMount } from 'svelte'
  import { formatMoney } from '../../../lib/money'
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

<main class="stage">
  <header>
    <h1>Gift liability</h1>
    <p>Outstanding balances on active gift cards.</p>
  </header>
  {#if error}<p class="error">{error}</p>{/if}
  {#if report}
    <p>
      <strong>{report.cards}</strong> cards ·
      <strong>{formatMoney(report.outstanding, 'PKR', 'en-PK')}</strong> outstanding
    </p>
    <table>
      <thead>
        <tr>
          <th>Code</th>
          <th>Balance</th>
          <th>Issued on</th>
        </tr>
      </thead>
      <tbody>
        {#each report.rows as row}
          <tr>
            <td>{row.code}</td>
            <td>{formatMoney(row.balance, 'PKR', 'en-PK')}</td>
            <td>{row.issued_bill_no || '—'}</td>
          </tr>
        {:else}
          <tr><td colspan="3">No outstanding liability.</td></tr>
        {/each}
      </tbody>
    </table>
  {/if}
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
