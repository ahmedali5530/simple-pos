<script lang="ts">
  import { loadReports } from '../api'
  import { printProductMix } from '../print'
  import type { ReportSpan } from '../range'
  import type { Reports } from '../types'
  import ReportRange from './ReportRange.svelte'
  import { toast, toastError } from '../toast'
  import VirtualTable from './VirtualTable.svelte'

  interface Props {
    currency: string
    locale: string
  }

  let { currency: _currency, locale: _locale }: Props = $props()

  let report = $state<Reports | null>(null)
  let failed = $state(false)
  let query = $state('')
  let printing = $state(false)
  let span = $state<Extract<ReportSpan, { kind: 'ready' }>>({ kind: 'ready' })

  function hit(needle: string, parts: Array<string | null | undefined>) {
    const text = needle.trim().toLowerCase()
    if (!text) return true
    return parts.some((part) => (part ?? '').toLowerCase().includes(text))
  }

  const mix = $derived(report?.mix.filter((line) => hit(query, [line.name, line.barcode, line.section_name])) ?? [])

  async function reload(next: Extract<ReportSpan, { kind: 'ready' }>) {
    span = next
    failed = false
    try {
      report = await loadReports(next.from, next.to)
    } catch (reason) {
      report = null
      failed = true
      toastError(reason, 'The report did not open.')
    }
  }

  async function printMix() {
    printing = true
    try {
      const result = await printProductMix(span.from, span.to)
      if (result.printed) toast.success('The product mix was sent to the printer.')
      else toast.warning('Printing is turned off in System settings.')
    } catch (reason) {
      toastError(reason, 'The product mix did not print.')
    } finally {
      printing = false
    }
  }
</script>

<div class="report-board">
  <ReportRange label={report?.range_label ?? ''} onchange={reload} oninvalid={(message) => toast.error(message)} />
  {#if !report}
    {#if !failed}<p class="hint">Opening the report…</p>{/if}
  {:else}
    <section class="report-card">
      <div class="summary-head">
        <div>
          <h2>Product Mix</h2>
          <p class="hint">Opening is the stock before {report.range_label}. Net is what is left after the movement in that range.</p>
        </div>
        <button type="button" class="save" disabled={printing} onclick={printMix}>{printing ? 'Printing…' : 'Print'}</button>
      </div>
      <input class="mb-2 mt-2 h-11 w-full rounded-xl border border-slate-300 bg-white px-3 text-sm font-bold text-slate-900" placeholder="Search items" bind:value={query} />
      <VirtualTable class="report-scroll" tableClass="bill-log" items={mix} estimate={49} columns={6} reset={query} key={(line) => line.id}>
        {#snippet header()}
          <tr>
            <th>Item</th>
            <th>Head</th>
            <th>Opening</th>
            <th>In</th>
            <th>Out</th>
            <th>Net</th>
          </tr>
        {/snippet}
        {#snippet row(line)}
          <tr>
            <td class="bill-no">{line.name}</td>
            <td>{line.section_name}</td>
            <td class={line.opening < 0 ? 'qty-out' : ''}>{line.opening}</td>
            <td class="qty-in">+{line.qty_in}</td>
            <td class="qty-out">{line.qty_out > 0 ? `-${line.qty_out}` : '0'}</td>
            <td class="bill-total {line.opening + line.qty_in - line.qty_out < 0 ? 'qty-out' : ''}">{line.opening + line.qty_in - line.qty_out}</td>
          </tr>
        {/snippet}
        {#snippet footer()}
          {#if mix.length === 0}
            <p class="hint">{query.trim() ? 'Nothing matches that search.' : 'No items moved in this range.'}</p>
          {:else if report?.mix_truncated}
            <p class="hint">Showing the first 400 items.</p>
          {/if}
        {/snippet}
      </VirtualTable>
    </section>
  {/if}
</div>
