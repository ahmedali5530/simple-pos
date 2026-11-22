<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import LedgerReport from './LedgerReport.svelte'
</script>

<ShopGate>
  {#snippet children(snapshot)}
    <LedgerReport currency={snapshot.currency} locale={snapshot.locale} />
  {/snippet}
</ShopGate>
