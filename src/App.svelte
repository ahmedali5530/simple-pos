<script lang="ts">
  import BillingScreen from './lib/billing/BillingScreen.svelte'
  import LoginScreen from './lib/LoginScreen.svelte'
  import { location } from './lib/route.svelte'
  import { session } from './lib/session.svelte'

  $effect(() => {
    if (!session.user && location.path !== '/login') {
      history.replaceState(null, '', '/login')
      location.path = '/login'
    } else if (session.user && location.path === '/login') {
      history.replaceState(null, '', '/billing')
      location.path = '/billing'
    }
  })
</script>

{#if session.user}
  <BillingScreen />
{:else}
  <LoginScreen />
{/if}
