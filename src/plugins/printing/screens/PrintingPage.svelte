<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import PrintingSettings from './PrintingSettings.svelte'
</script>

<ShopGate>
  {#snippet children(snapshot, setSnapshot)}
    <PrintingSettings {snapshot} onChange={setSnapshot} />
  {/snippet}
</ShopGate>
