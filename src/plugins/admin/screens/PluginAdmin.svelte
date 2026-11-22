<script lang="ts">
  import { request } from '../../../lib/api'
  import { toast, toastError } from '../../../lib/toast'
  import { Button, Field, Input } from '../../../lib/ui'

  interface PluginRow {
    id: string
    name: string
    version: string
    enabled: boolean
  }

  let plugins = $state<PluginRow[]>([])
  let tokenName = $state('')
  let tokenResult = $state('')
  let pending = $state('')

  $effect(() => {
    request<{ available?: PluginRow[] }>('/api/v1/platform/manifest')
      .then((body) => {
        plugins = body.available ?? []
      })
      .catch((error: unknown) => {
        toastError(error, 'Could not load plugins.')
      })
  })

  async function toggle(plugin: PluginRow) {
    const enabled = !plugin.enabled
    pending = plugin.id
    try {
      await request(`/api/v1/platform/plugins/${plugin.id}`, {
        method: 'PUT',
        body: JSON.stringify({ enabled }),
      })
      plugin.enabled = enabled
      window.dispatchEvent(new Event('plugins-changed'))
      toast.success(enabled ? `${plugin.name} is on.` : `${plugin.name} is off.`)
    } catch (error) {
      toastError(error, 'Could not update the plugin.')
    } finally {
      pending = ''
    }
  }

  async function createToken() {
    if (!tokenName.trim()) {
      toast.error('Enter a token name.')
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
      toast.success('Token created. Copy it now; it will not be shown again.')
    } catch (error) {
      toastError(error, 'Could not create token.')
    }
  }
</script>

<section class="place">
  <h2>Plugins</h2>
  <p class="hint">Turn a plugin off without deleting its data. A plugin stays off while something else still needs it.</p>
  <ul class="plugin-list">
    {#each plugins as plugin (plugin.id)}
      <li>
        <strong>{plugin.name}</strong>
        <span class="hint">{plugin.id} · v{plugin.version}</span>
        <Button variant={plugin.enabled ? 'primary' : 'secondary'} size="sm" disabled={pending === plugin.id} onclick={() => toggle(plugin)}>
          {plugin.enabled ? 'On' : 'Off'}
        </Button>
      </li>
    {/each}
  </ul>

  <h2>API tokens</h2>
  <p class="hint">Tokens for AI agents and integrations. Destructive capabilities stay off by default.</p>
  <Field label="Name"><Input bind:value={tokenName} /></Field>
  <div class="sheet-actions">
    <Button onclick={createToken}>Create token</Button>
  </div>
  {#if tokenResult}
    <p class="hint"><code>{tokenResult}</code></p>
  {/if}
</section>
