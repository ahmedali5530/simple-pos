<script lang="ts">
  import { onMount } from 'svelte'
  import { loadUsers } from '../../../lib/api'
  import type { PosUser } from '../../../lib/types'
  import { toastError } from '../../../lib/toast'
  import UserCatalog from './UserCatalog.svelte'

  let users = $state<PosUser[]>([])
  let loading = $state(true)

  onMount(() => {
    loadUsers()
      .then((value) => (users = value))
      .catch((error) => toastError(error, 'Could not load users.'))
      .finally(() => (loading = false))
  })
</script>

{#if loading}
  <p class="loading">Loading users…</p>
{:else}
  <UserCatalog users={users} onChange={(value) => (users = value)} />
{/if}
