<script lang="ts">
  import { loadReports } from '../api'
  import { formatCost } from '../money'
  import type { ReportSpan } from '../range'
  import type { Reports, StockMovement } from '../types'
  import ReportRange from './ReportRange.svelte'
  import { toast, toastError } from '../toast'
  import VirtualList from './VirtualList.svelte'

  interface Props {
    currency: string
    locale: string
  }

  let { currency, locale }: Props = $props()

  let report = $state<Reports | null>(null)
  let failed = $state(false)
  let query = $state('')

  function money(cents: number) {
    return formatCost(cents, currency, locale)
  }

  function clock(value: string) {
    const date = new Date(value)
    if (Number.isNaN(date.getTime())) return value
    return date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
  }

  function hit(needle: string, parts: Array<string | null | undefined>) {
    const text = needle.trim().toLowerCase()
    if (!text) return true
    return parts.some((part) => (part ?? '').toLowerCase().includes(text))
  }

  function note(move: StockMovement) {
    if (move.bill_no) return `POS Bill #${move.bill_no}`
    const extra = move.reason.split(' · ').slice(1).join(' · ')
    return extra || move.reason
  }

  const movements = $derived(
    report?.movements.filter((move) => hit(query, [move.item_name, move.section_name, move.reason, move.bill_no])) ?? [],
  )

  async function reload(span: Extract<ReportSpan, { kind: 'ready' }>) {
    failed = false
    try {
      report = await loadReports(span.from, span.to)
    } catch (reason) {
      report = null
      failed = true
      toastError(reason, 'The report did not open.')
    }
  }
</script>

<div class="report-board">
  <ReportRange label={report?.range_label ?? ''} onchange={reload} oninvalid={(message) => toast.error(message)} />
  {#if !report}
    {#if !failed}<p class="hint">Opening the report…</p>{/if}
  {:else}
    <div class="kpi-row">
      <article class="kpi">
        <header>
          <span>Stock valuation</span>
          <i class="kpi-icon blue" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M3 7l9-4 9 4v10l-9 4-9-4zm9 2.2L6.5 7 12 4.8 17.5 7z" /></svg>
          </i>
        </header>
        <strong>{money(report.stock_value)}</strong>
        <p>{report.stock_units} on hand, priced at average cost</p>
      </article>
    </div>

    <section class="report-card">
      <div class="summary-head">
        <div>
          <h2>Stock In / Out Ledger</h2>
          <p class="hint">Restocks, manual adjustments, and POS deductions</p>
        </div>
      </div>
      <input class="input mb-2" placeholder="Search the ledger" bind:value={query} />
      <VirtualList class="report-scroll" items={movements} estimate={76} reset={query} key={(move) => move.id}>
        {#snippet children(move)}
          <article class="ledger-row">
            <div>
              <div class="ledger-meta">
                <span class="move-badge" class:out={move.qty < 0}>{move.qty < 0 ? 'Stock out' : 'Stock in'}</span>
                <time>{clock(move.created_at)}</time>
              </div>
              <strong>{move.item_name}</strong>
              <p>{note(move)}</p>
            </div>
            <strong class:qty-in={move.qty > 0} class:qty-out={move.qty < 0}>{move.qty > 0 ? `+${move.qty}` : move.qty}</strong>
          </article>
        {/snippet}
        {#snippet footer()}
          {#if movements.length === 0}
            <p class="hint">{query.trim() ? 'Nothing matches that search.' : 'No stock movements in this range.'}</p>
          {:else if report?.movements_truncated}
            <p class="hint">Showing the latest 200 movements.</p>
          {/if}
        {/snippet}
      </VirtualList>
    </section>
  {/if}
</div>
