<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import UserCatalog from './UserCatalog.svelte'
</script>

<ShopGate>
  {#snippet children(snapshot, setSnapshot)}
    <UserCatalog users={snapshot.users ?? []} onChange={setSnapshot} />
  {/snippet}
</ShopGate>
