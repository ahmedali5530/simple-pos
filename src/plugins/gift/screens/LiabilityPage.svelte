<script lang="ts">
  import { onMount } from 'svelte'
  import { formatMoney } from '../../../lib/money'
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import { toast, toastError } from '../../../lib/toast'
  import { exportGiftLiability, giftLiability, type GiftLiability } from '../api'

  let report = $state<GiftLiability | null>(null)
  let failed = $state(false)
  let query = $state('')
  let exporting = $state<'xlsx' | 'pdf' | ''>('')

  const rows = $derived(
    report?.rows.filter((row) => {
      const needle = query.trim().toLowerCase()
      if (!needle) return true
      return [row.code, row.issued_bill_no].some((part) => part.toLowerCase().includes(needle))
    }) ?? [],
  )

  onMount(() => {
    giftLiability()
      .then((value) => (report = value))
      .catch((reason) => {
        failed = true
        toastError(reason, 'Could not load gift liability.')
      })
  })

  async function download(format: 'xlsx' | 'pdf') {
    exporting = format
    try {
      await exportGiftLiability(format)
    } catch (reason) {
      toastError(reason, 'The export did not download.')
    } finally {
      exporting = ''
    }
  }
</script>

<ShopLocaleGate>
  {#snippet children(shop)}
    <div class="report-board">
      {#if !report}
        {#if !failed}<p class="hint">Opening the report…</p>{/if}
      {:else}
        <div class="kpi-row">
          <article class="kpi">
            <header>
              <span>Total issued</span>
              <i class="kpi-icon purple" aria-hidden="true">
                <svg viewBox="0 0 24 24"><path d="M4 7h16v3H4zm0 5h16v3H4zm2 5h12v2H6z" /></svg>
              </i>
            </header>
            <strong>{formatMoney(report.issued, shop.currency, shop.locale)}</strong>
            <p>Face value on active cards</p>
          </article>
          <article class="kpi">
            <header>
              <span>Balance outstanding</span>
              <i class="kpi-icon green" aria-hidden="true">
                <svg viewBox="0 0 24 24"><path d="M5 16l5-5 3 3 6-7v4h2V5h-6v2h4l-5 6-3-3-6 6z" /></svg>
              </i>
            </header>
            <strong>{formatMoney(report.outstanding, shop.currency, shop.locale)}</strong>
            <p>Still available to redeem</p>
          </article>
          <article class="kpi">
            <header>
              <span>Cards</span>
              <i class="kpi-icon slate" aria-hidden="true">
                <svg viewBox="0 0 24 24"><path d="M3 6h18v12H3zm2 2v8h14V8zm2 2h4v2H7zm6 0h4v2h-4z" /></svg>
              </i>
            </header>
            <strong>{report.cards}</strong>
            <p>{report.cards === 1 ? '1 active card' : `${report.cards} active cards`}</p>
          </article>
        </div>

        <section class="report-card">
          <div class="summary-head">
            <div>
              <h2>Gift liability</h2>
              <p class="hint">Outstanding balances on active gift cards</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <button type="button" class="inline-flex h-10 items-center rounded-xl border border-slate-300 bg-white px-3 text-sm font-extrabold text-slate-900 disabled:opacity-50" disabled={!!exporting} onclick={() => download('pdf')}>
                {exporting === 'pdf' ? 'Exporting…' : 'Export PDF'}
              </button>
              <button type="button" class="inline-flex h-10 items-center rounded-xl border border-slate-300 bg-white px-3 text-sm font-extrabold text-slate-900 disabled:opacity-50" disabled={!!exporting} onclick={() => download('xlsx')}>
                {exporting === 'xlsx' ? 'Exporting…' : 'Export Excel'}
              </button>
            </div>
          </div>
          <input class="input mb-2" placeholder="Search cards" bind:value={query} />
          <div class="report-scroll">
            <table class="bill-log">
              <thead>
                <tr>
                  <th>Code</th>
                  <th>Issued</th>
                  <th>Balance</th>
                  <th>Issued on</th>
                </tr>
              </thead>
              <tbody>
                {#each rows as row}
                  <tr>
                    <td class="bill-no">{row.code}</td>
                    <td>{formatMoney(row.issued, shop.currency, shop.locale)}</td>
                    <td class="bill-total">{formatMoney(row.balance, shop.currency, shop.locale)}</td>
                    <td>{row.issued_at ? row.issued_at.slice(0, 10) : row.issued_bill_no || '—'}</td>
                  </tr>
                {:else}
                  <tr>
                    <td colspan="4">
                      {query.trim() ? 'Nothing matches that search.' : 'No outstanding liability.'}
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
