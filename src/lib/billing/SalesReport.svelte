<script lang="ts">
  import { loadReports } from '../api'
  import { formatCost, formatMoney } from '../money'
  import type { ReportSpan } from '../range'
  import type { Reports } from '../types'
  import ReportRange from './ReportRange.svelte'
  import { toast, toastError } from '../toast'
  import VirtualTable from './VirtualTable.svelte'

  interface Props {
    currency: string
    locale: string
  }

  let { currency, locale }: Props = $props()

  let report = $state<Reports | null>(null)
  let failed = $state(false)
  let headId = $state('all')
  let query = $state('')
  let openBill = $state<number | null>(null)

  function money(amount: number) {
    return formatMoney(amount, currency, locale)
  }

  function costMoney(cents: number) {
    return formatCost(cents, currency, locale)
  }

  function clock(value: string) {
    const date = new Date(value)
    if (Number.isNaN(date.getTime())) return value
    return date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
  }

  function summary(lines: { name: string; qty: number }[]) {
    return lines.map((line) => `${line.name} ×${line.qty}`).join(', ')
  }

  function billsLabel(count: number) {
    return count === 1 ? '1 bill recorded' : `${count} bills recorded`
  }

  function hit(needle: string, parts: Array<string | null | undefined>) {
    const text = needle.trim().toLowerCase()
    if (!text) return true
    return parts.some((part) => (part ?? '').toLowerCase().includes(text))
  }

  const bills = $derived(
    report?.bills.filter(
      (bill) =>
        (headId === 'all' || bill.short_name === headId) &&
        hit(query, [bill.bill_no, bill.short_name, bill.section_name, bill.cashier_name, bill.customer, summary(bill.lines)]),
    ) ?? [],
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
          <span>Total sales</span>
          <i class="kpi-icon purple" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M4 7h16v3H4zm0 5h16v3H4zm2 5h12v2H6z" /></svg>
          </i>
        </header>
        <strong>{money(report.total)}</strong>
        <p>{report.sections.length === 2 ? 'Combined both heads' : 'Combined all heads'}</p>
      </article>
      <article class="kpi">
        <header>
          <span>Profit</span>
          <i class="kpi-icon green" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M5 16l5-5 3 3 6-7v4h2V5h-6v2h4l-5 6-3-3-6 6z" /></svg>
          </i>
        </header>
        <strong>{costMoney(report.profit)}</strong>
        <p>Selling price minus cost. No cost counts as zero.</p>
      </article>
      {#each report.sections as section (section.id)}
        <article class="kpi tinted" style:--accent={section.accent}>
          <header>
            <span>{section.short_name} sales</span>
            <i class="kpi-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24"><path d="M6 7h12l-1 12H7zm3-3h6l1 3H8z" /></svg>
            </i>
          </header>
          <strong>{money(section.total)}</strong>
          <p>{billsLabel(section.bills)}</p>
        </article>
      {/each}
      <article class="kpi">
        <header>
          <span>Total bill entries</span>
          <i class="kpi-icon slate" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M7 3h8l4 4v14H7zm8 1.5V8h3.5zM9 12h6v2H9zm0 4h6v2H9z" /></svg>
          </i>
        </header>
        <strong>{report.bill_count}</strong>
        <p>*Add Bill = One Entry*</p>
      </article>
    </div>

    <section class="report-card">
      <div class="summary-head">
        <div>
          <h2>Sales Report — “One Entry” Bill Log</h2>
          <p class="hint">Profit is the bill total minus the cost of those items</p>
        </div>
        <label class="head-filter">
          <span class="sr-only">Business head</span>
          <select bind:value={headId}>
            <option value="all">All Business Heads</option>
            {#each report.sections as section (section.id)}
              <option value={section.short_name}>{section.short_name}</option>
            {/each}
          </select>
        </label>
      </div>
      <input class="mb-2 h-11 w-full rounded-xl border border-slate-300 bg-white px-3 text-sm font-bold text-slate-900" placeholder="Search bills" bind:value={query} />
      <VirtualTable class="report-scroll" tableClass="bill-log" items={bills} estimate={58} columns={7} reset="{headId}:{query}" key={(bill) => bill.id}>
        {#snippet header()}
          <tr>
            <th>Bill #</th>
            <th>Time</th>
            <th>Head</th>
            <th>Items summary</th>
            <th>Total</th>
            <th>Profit</th>
            <th>Receipt</th>
          </tr>
        {/snippet}
        {#snippet row(bill)}
          <tr>
            <td class="bill-no">
              {bill.bill_no}
              {#if bill.customer}<span class="block text-xs text-slate-500">{bill.customer}</span>{/if}
            </td>
            <td>{clock(bill.created_at)}</td>
            <td><span class="head-pill" style:--accent={bill.accent}>{bill.short_name}</span></td>
            <td class="summary-cell" title={summary(bill.lines)}>{summary(bill.lines)}</td>
            <td class="bill-total">{money(bill.total)}</td>
            <td class="bill-total">{costMoney(bill.total * 100 - bill.cost)}</td>
            <td>
              <button
                type="button"
                class="receipt-btn"
                aria-label="Receipt for {bill.bill_no}"
                aria-expanded={openBill === bill.id}
                onclick={() => (openBill = openBill === bill.id ? null : bill.id)}
              >
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 6c4.5 0 8 3.2 9.2 6-1.2 2.8-4.7 6-9.2 6S4 14.8 2.8 12C4 9.2 7.5 6 12 6zm0 2.2A3.8 3.8 0 1 0 12 15.8 3.8 3.8 0 0 0 12 8.2zm0 1.6a2.2 2.2 0 1 1 0 4.4 2.2 2.2 0 0 1 0-4.4z" /></svg>
              </button>
            </td>
          </tr>
          {#if openBill === bill.id}
            <tr class="receipt-lines">
              <td colspan="7">
                {#each bill.lines as line, index (`${bill.id}-${index}`)}
                  <span>{line.name} ×{line.qty}</span>
                  <strong>{money(line.amount)}</strong>
                {/each}
              </td>
            </tr>
          {/if}
        {/snippet}
        {#snippet footer()}
          {#if bills.length === 0}
            <p class="hint">{query.trim() ? 'Nothing matches that search.' : 'No bills for this range.'}</p>
          {:else if report?.bills_truncated}
            <p class="hint">Showing the latest 200 bills.</p>
          {/if}
        {/snippet}
      </VirtualTable>
    </section>
  {/if}
</div>
