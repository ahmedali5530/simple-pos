<script lang="ts">
  import { formatMoney } from '../../../lib/money'
  import type { ReportSpan } from '../../../lib/range'
  import ReportRange from '../../reports/screens/ReportRange.svelte'
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import { toast, toastError } from '../../../lib/toast'
  import { loadPaymentsReport, type PaymentsReport } from '../api'

  let report = $state<PaymentsReport | null>(null)
  let failed = $state(false)

  async function reload(span: Extract<ReportSpan, { kind: 'ready' }>) {
    failed = false
    try {
      report = await loadPaymentsReport(span.from, span.to)
    } catch (reason) {
      report = null
      failed = true
      toastError(reason, 'Could not load payment mix.')
    }
  }
</script>

<ShopLocaleGate>
  {#snippet children(shop)}
    <main class="stage">
      <header>
        <h1>Payment mix</h1>
        <p>Breakdown from mirrored bill tenders (cash + configured payment types).</p>
      </header>

      <ReportRange
        onchange={reload}
        oninvalid={(message) => toast.error(message)}
      />

      {#if !report}
        {#if !failed}<p class="hint">Opening the report…</p>{/if}
      {:else}
        <p class="grand">
          Collected {formatMoney(report.collected, shop.currency, shop.locale)}
        </p>
        <table>
          <thead>
            <tr>
              <th>Method</th>
              <th>Bills</th>
              <th>Amount</th>
            </tr>
          </thead>
          <tbody>
            {#each report.totals as row}
              <tr>
                <td>{row.label || row.method}</td>
                <td>{row.bills}</td>
                <td>{formatMoney(row.amount, shop.currency, shop.locale)}</td>
              </tr>
            {:else}
              <tr><td colspan="3">No payments in this range.</td></tr>
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
  .grand {
    margin: 0;
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
  .hint {
    color: var(--ui-text-muted);
  }
</style>
