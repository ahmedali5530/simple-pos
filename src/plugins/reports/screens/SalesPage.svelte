<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import SalesReport from './SalesReport.svelte'
</script>

<ShopGate>
  {#snippet children(snapshot)}
    <SalesReport currency={snapshot.currency} locale={snapshot.locale} />
  {/snippet}
</ShopGate>
