<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import StockScreen from './StockScreen.svelte'
</script>

<main class="stage">
  <ShopGate>
    {#snippet children(snapshot, setSnapshot)}
      <StockScreen
        sections={snapshot.sections}
        categories={snapshot.categories}
        currency={snapshot.currency}
        locale={snapshot.locale}
        onChange={setSnapshot}
      />
    {/snippet}
  </ShopGate>
</main>
