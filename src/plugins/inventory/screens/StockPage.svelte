<script lang="ts">
  import CatalogGate from '../../../platform/CatalogGate.svelte'
  import StockScreen from './StockScreen.svelte'
</script>

<main class="stage">
  <CatalogGate>
    {#snippet children(snapshot, setSnapshot)}
      <StockScreen
        sections={snapshot.sections}
        categories={snapshot.categories}
        currency={snapshot.currency}
        locale={snapshot.locale}
        onChange={setSnapshot}
      />
    {/snippet}
  </CatalogGate>
</main>
