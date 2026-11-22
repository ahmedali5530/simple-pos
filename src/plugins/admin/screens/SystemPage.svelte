<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import SystemSettings from './SystemSettings.svelte'
</script>

<ShopGate>
  {#snippet children(snapshot, setSnapshot)}
    <SystemSettings {snapshot} onChange={setSnapshot} />
  {/snippet}
</ShopGate>
