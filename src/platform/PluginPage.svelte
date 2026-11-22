<script lang="ts">
  import { location } from './route.svelte'
  import { pluginForPath } from '../plugins/registry'

  interface Props {
    path?: string
  }

  let { path = '' }: Props = $props()
  const current = $derived(path || location.path)
  const id = $derived(pluginForPath(current) ?? 'plugin')
</script>

<main class="stage">
  <section class="place">
    <h2>{id}</h2>
    <p class="hint">This part of the shop is provided by the {id} plugin. Its API is /api/v1/{id}.</p>
  </section>
</main>
