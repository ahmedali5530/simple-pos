<script lang="ts">
  import { request } from '../../../lib/api'

  interface PluginRow {
    id: string
    name: string
    version: string
    enabled?: boolean
  }

  let plugins = $state<PluginRow[]>([])
  let message = $state('')
  let tokenName = $state('')
  let tokenResult = $state('')

  $effect(() => {
    request<{ plugins: PluginRow[] }>('/api/v1/platform/manifest')
      .then((body) => {
        plugins = (body.plugins ?? []).map((p) => ({ ...p, enabled: true }))
      })
      .catch((error: unknown) => {
        message = error instanceof Error ? error.message : 'Could not load plugins.'
      })
  })

  async function createToken() {
    if (!tokenName.trim()) {
      message = 'Enter a token name.'
      return
    }
    try {
      const result = await request<{ token: string; id: number }>('/api/v1/platform/tokens', {
        method: 'POST',
        body: JSON.stringify({
          name: tokenName.trim(),
          permissions: ['catalog.read', 'inventory.read', 'pos.read', 'reports.read', 'cash.read'],
          allow_destructive: false,
        }),
      })
      tokenResult = result.token
      message = 'Token created. Copy it now; it will not be shown again.'
    } catch (error) {
      message = error instanceof Error ? error.message : 'Could not create token.'
    }
  }
</script>

<section class="place">
  <h2>Plugins</h2>
  <p class="hint">Enabled plugins for this shop. Core keeps security, tenancy and lifecycle.</p>
  <ul class="plugin-list">
    {#each plugins as plugin (plugin.id)}
      <li>
        <strong>{plugin.name}</strong>
        <span class="hint">{plugin.id} · v{plugin.version}</span>
        <span class="badge">{plugin.enabled === false ? 'Off' : 'On'}</span>
      </li>
    {/each}
  </ul>

  <h2>API tokens</h2>
  <p class="hint">Tokens for AI agents and integrations. Destructive capabilities stay off by default.</p>
  <label>
    Name
    <input bind:value={tokenName} />
  </label>
  <div class="sheet-actions">
    <button type="button" class="save" onclick={createToken}>Create token</button>
  </div>
  {#if tokenResult}
    <p class="hint"><code>{tokenResult}</code></p>
  {/if}
  {#if message}<p class="field-error">{message}</p>{/if}
</section>
