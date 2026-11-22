<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import WarrantyScreen from './WarrantyScreen.svelte'
</script>

<main class="stage">
  <ShopGate>
    {#snippet children(snapshot)}
      <WarrantyScreen currency={snapshot.currency} locale={snapshot.locale} />
    {/snippet}
  </ShopGate>
</main>
