<script lang="ts">
  import { formatMoney } from '../../../lib/money'
  import type { ReportSpan } from '../../../lib/range'
  import ReportRange from '../../reports/screens/ReportRange.svelte'
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import { toast, toastError } from '../../../lib/toast'
  import { loadPaymentsReport, type PaymentsReport } from '../api'

  let report = $state<PaymentsReport | null>(null)
  let failed = $state(false)
  let query = $state('')
  let rangeLabel = $state('')

  const rows = $derived(
    report?.totals.filter((row) => {
      const needle = query.trim().toLowerCase()
      if (!needle) return true
      return [row.label, row.method].some((part) => part.toLowerCase().includes(needle))
    }) ?? [],
  )

  async function reload(span: Extract<ReportSpan, { kind: 'ready' }>) {
    failed = false
    rangeLabel = span.from || span.to ? 'Selected range' : 'All dates'
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
    <div class="report-board">
      <ReportRange
        label={rangeLabel}
        onchange={reload}
        oninvalid={(message) => toast.error(message)}
      />

      {#if !report}
        {#if !failed}<p class="hint">Opening the report…</p>{/if}
      {:else}
        <div class="kpi-row">
          <article class="kpi">
            <header>
              <span>Collected</span>
              <i class="kpi-icon green" aria-hidden="true">
                <svg viewBox="0 0 24 24"><path d="M4 7h16v3H4zm0 5h16v3H4zm2 5h12v2H6z" /></svg>
              </i>
            </header>
            <strong>{formatMoney(report.collected, shop.currency, shop.locale)}</strong>
            <p>Cash and configured payment types</p>
          </article>
          <article class="kpi">
            <header>
              <span>Methods</span>
              <i class="kpi-icon slate" aria-hidden="true">
                <svg viewBox="0 0 24 24"><path d="M4 6h16v2H4zm0 5h16v2H4zm0 5h10v2H4z" /></svg>
              </i>
            </header>
            <strong>{report.totals.length}</strong>
            <p>{report.totals.length === 1 ? '1 tender method' : `${report.totals.length} tender methods`}</p>
          </article>
        </div>

        <section class="report-card">
          <div class="summary-head">
            <div>
              <h2>Payment mix</h2>
              <p class="hint">Breakdown from mirrored bill tenders</p>
            </div>
          </div>
          <input class="input mb-2" placeholder="Search methods" bind:value={query} />
          <div class="report-scroll">
            <table class="bill-log">
              <thead>
                <tr>
                  <th>Method</th>
                  <th>Bills</th>
                  <th>Amount</th>
                </tr>
              </thead>
              <tbody>
                {#each rows as row}
                  <tr>
                    <td class="bill-no">{row.label || row.method}</td>
                    <td>{row.bills}</td>
                    <td class="bill-total">{formatMoney(row.amount, shop.currency, shop.locale)}</td>
                  </tr>
                {:else}
                  <tr>
                    <td colspan="3">
                      {query.trim() ? 'Nothing matches that search.' : 'No payments in this range.'}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </section>
      {/if}
    </div>
  {/snippet}
</ShopLocaleGate>
