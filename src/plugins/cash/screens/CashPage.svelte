<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import CashScreen from './CashScreen.svelte'
</script>

<main class="stage">
  <ShopGate>
    {#snippet children(snapshot)}
      <CashScreen currency={snapshot.currency} locale={snapshot.locale} />
    {/snippet}
  </ShopGate>
</main>
