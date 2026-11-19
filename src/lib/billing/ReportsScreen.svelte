<script lang="ts">
  import { followLink, type ReportPage } from '../route.svelte'
  import LedgerReport from './LedgerReport.svelte'
  import MixReport from './MixReport.svelte'
  import SalesReport from './SalesReport.svelte'

  interface Props {
    page: ReportPage
    currency: string
    locale: string
  }

  let { page, currency, locale }: Props = $props()

  const pages: { id: ReportPage; label: string; href: string }[] = [
    { id: 'sales', label: 'Sales', href: '/reports/sales' },
    { id: 'ledger', label: 'Stock ledger', href: '/reports/ledger' },
    { id: 'mix', label: 'Product mix', href: '/reports/mix' },
  ]
</script>

<div class="settings">
  <nav class="settings-nav" aria-label="Reports">
    {#each pages as entry (entry.id)}
      <a href={entry.href} class:active={page === entry.id} onclick={(event) => followLink(event, entry.href)}>
        {entry.label}
      </a>
    {/each}
  </nav>
  <div class="settings-body">
    {#if page === 'ledger'}
      <LedgerReport {currency} {locale} />
    {:else if page === 'mix'}
      <MixReport {currency} {locale} />
    {:else}
      <SalesReport {currency} {locale} />
    {/if}
  </div>
</div>
