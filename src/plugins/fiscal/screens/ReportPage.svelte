<script lang="ts">
  import type { ReportSpan } from '../../../lib/range'
  import ReportRange from '../../reports/screens/ReportRange.svelte'
  import { toast, toastError } from '../../../lib/toast'
  import { Button } from '../../../lib/ui'
  import {
    listFiscalSubmissions,
    retryFiscalSubmission,
    type FiscalSubmission,
  } from '../api'

  let rows = $state<FiscalSubmission[]>([])
  let failed = $state(false)
  let loading = $state(true)
  let pending = $state('')
  let query = $state('')
  let span = $state<Extract<ReportSpan, { kind: 'ready' }>>({ kind: 'ready' })
  let rangeLabel = $state('All dates')

  const okCount = $derived(rows.filter((row) => row.status === 'ok').length)
  const failedCount = $derived(rows.filter((row) => row.status === 'failed').length)

  function inSpan(row: FiscalSubmission) {
    if (!span.from && !span.to) return true
    const when = Date.parse(row.submitted_at)
    if (Number.isNaN(when)) return true
    if (span.from && when < Date.parse(span.from)) return false
    if (span.to && when >= Date.parse(span.to)) return false
    return true
  }

  const visible = $derived(
    rows.filter((row) => {
      if (!inSpan(row)) return false
      const needle = query.trim().toLowerCase()
      if (!needle) return true
      return [row.bill_no, row.invoice_no, row.authority, row.status, row.error_message].some((part) =>
        part.toLowerCase().includes(needle),
      )
    }),
  )

  const visibleOk = $derived(visible.filter((row) => row.status === 'ok').length)
  const visibleFailed = $derived(visible.filter((row) => row.status === 'failed').length)

  async function refresh() {
    failed = false
    loading = true
    try {
      rows = await listFiscalSubmissions()
    } catch (reason) {
      rows = []
      failed = true
      toastError(reason, 'Could not load fiscal submissions.')
    } finally {
      loading = false
    }
  }

  async function reload(next: Extract<ReportSpan, { kind: 'ready' }>) {
    span = next
    rangeLabel = next.from || next.to ? 'Selected range' : 'All dates'
    if (rows.length === 0 && !failed) await refresh()
  }

  async function retry(billNo: string) {
    pending = billNo
    try {
      await retryFiscalSubmission(billNo)
      toast.success(`Retried ${billNo}.`)
      await refresh()
    } catch (reason) {
      toastError(reason, 'Could not retry that submission.')
    } finally {
      pending = ''
    }
  }
</script>

<div class="report-board">
  <ReportRange
    label={rangeLabel}
    onchange={reload}
    oninvalid={(message) => toast.error(message)}
  />

  {#if loading && rows.length === 0}
    {#if !failed}<p class="hint">Opening the report…</p>{/if}
  {:else}
    <div class="kpi-row">
      <article class="kpi">
        <header>
          <span>Submitted</span>
          <i class="kpi-icon green" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z" /></svg>
          </i>
        </header>
        <strong>{visibleOk}</strong>
        <p>{okCount === visibleOk ? 'Accepted by the authority' : `Of ${okCount} overall`}</p>
      </article>
      <article class="kpi">
        <header>
          <span>Failed</span>
          <i class="kpi-icon purple" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 15h-2v-2h2zm0-4h-2V7h2z" /></svg>
          </i>
        </header>
        <strong>{visibleFailed}</strong>
        <p>{failedCount === visibleFailed ? 'Need a retry' : `Of ${failedCount} overall`}</p>
      </article>
      <article class="kpi">
        <header>
          <span>In view</span>
          <i class="kpi-icon slate" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M4 6h16v2H4zm0 5h16v2H4zm0 5h10v2H4z" /></svg>
          </i>
        </header>
        <strong>{visible.length}</strong>
        <p>{visible.length === 1 ? '1 submission' : `${visible.length} submissions`}</p>
      </article>
    </div>

    <section class="report-card">
      <div class="summary-head">
        <div>
          <h2>Fiscal submissions</h2>
          <p class="hint">Invoices sent to FBR/PRA. Failed rows can be retried without posting the sale again.</p>
        </div>
      </div>
      <input class="input mb-2" placeholder="Search bills or invoices" bind:value={query} />
      <div class="report-scroll">
        <table class="bill-log">
          <thead>
            <tr>
              <th>Bill</th>
              <th>Authority</th>
              <th>Invoice</th>
              <th>Status</th>
              <th>When</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each visible as row}
              <tr>
                <td class="bill-no">{row.bill_no}</td>
                <td>{row.authority.toUpperCase()}</td>
                <td>{row.invoice_no || '—'}</td>
                <td>
                  <span class:qty-out={row.status === 'failed'} class:qty-in={row.status === 'ok'}>
                    {row.status}
                  </span>
                  {#if row.error_message}
                    <span class="block text-xs text-slate-500">{row.error_message}</span>
                  {/if}
                </td>
                <td>{row.submitted_at?.slice(0, 19) || '—'}</td>
                <td>
                  {#if row.status === 'failed'}
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={pending === row.bill_no}
                      onclick={() => void retry(row.bill_no)}
                    >
                      {pending === row.bill_no ? 'Retrying…' : 'Retry'}
                    </Button>
                  {/if}
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="6">
                  {query.trim() || span.from || span.to
                    ? 'Nothing matches that filter.'
                    : 'No fiscal submissions yet.'}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}
</div>
