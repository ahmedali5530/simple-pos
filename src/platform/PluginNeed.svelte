<script lang="ts">
  interface Props {
    /** Display name of the missing plugin, e.g. "Catalog". */
    plugin: string
    /** What this page needs that plugin for. */
    purpose?: string
  }

  let { plugin, purpose }: Props = $props()
</script>

<main class="stage">
  <section class="place">
    <h2>{plugin} is not enabled</h2>
    <p class="hint">
      {#if purpose}
        {purpose}
      {:else}
        Enable the {plugin} plugin in Settings → Plugins to use this page.
      {/if}
    </p>
  </section>
</main>
