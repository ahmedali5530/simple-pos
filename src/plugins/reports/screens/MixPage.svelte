<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import MixReport from './MixReport.svelte'
</script>

<ShopGate>
  {#snippet children(snapshot)}
    <MixReport currency={snapshot.currency} locale={snapshot.locale} />
  {/snippet}
</ShopGate>
