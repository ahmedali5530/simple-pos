<script lang="ts">
  import { request } from '../../../lib/api'
  import { toast, toastError } from '../../../lib/toast'
  import { Button, Field, Input, Switch } from '../../../lib/ui'

  interface SwitchConsent {
    title: string
    body: string
    acknowledge?: boolean
  }

  interface PluginRow {
    id: string
    name: string
    version: string
    description?: string
    enabled: boolean
    consents?: {
      enable?: SwitchConsent | null
      disable?: SwitchConsent | null
    }
  }

  let plugins = $state<PluginRow[]>([])
  let tokenName = $state('')
  let tokenResult = $state('')
  let pending = $state('')
  let prompt = $state<{ plugin: PluginRow; enabled: boolean; consent: SwitchConsent } | null>(null)
  let accepted = $state(false)

  $effect(() => {
    request<{ available?: PluginRow[] }>('/api/v1/platform/manifest')
      .then((body) => {
        plugins = body.available ?? []
      })
      .catch((error: unknown) => {
        toastError(error, 'Could not load plugins.')
      })
  })

  function consentFor(plugin: PluginRow, enabled: boolean) {
    const consent = enabled ? plugin.consents?.enable : plugin.consents?.disable
    return consent?.title && consent.body ? consent : null
  }

  function toggle(plugin: PluginRow) {
    const enabled = !plugin.enabled
    const consent = consentFor(plugin, enabled)
    if (consent) {
      accepted = false
      prompt = { plugin, enabled, consent }
      return
    }
    apply(plugin, enabled)
  }

  function dismiss() {
    prompt = null
    accepted = false
  }

  async function confirm() {
    const current = prompt
    if (!current) return
    if (current.consent.acknowledge && !accepted) return
    prompt = null
    accepted = false
    await apply(current.plugin, current.enabled)
  }

  async function apply(plugin: PluginRow, enabled: boolean) {
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
        <div class="plugin-copy">
          <div class="flex gap-5 items-center">
            <strong>{plugin.name}</strong>
            <span class="hint mt-0">{plugin.id} · v{plugin.version}</span>
          </div>
          {#if plugin.description}
            <p class="hint">{plugin.description}</p>
          {/if}
        </div>
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

{#if prompt}
  <div
    class="sheet-backdrop"
    role="presentation"
    onclick={dismiss}
    onkeydown={(event) => {
      if (event.key === 'Escape') dismiss()
    }}
  >
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="plugin-consent-title"
      tabindex="-1"
      onclick={(event) => event.stopPropagation()}
      onkeydown={(event) => event.stopPropagation()}
    >
      <h2 id="plugin-consent-title">{prompt.consent.title}</h2>
      <p class="hint">{prompt.consent.body}</p>
      {#if prompt.consent.acknowledge}
        <Switch bind:checked={accepted} label="I understand and agree" />
      {/if}
      <div class="sheet-actions">
        <Button variant="secondary" onclick={dismiss}>Cancel</Button>
        <Button disabled={prompt.consent.acknowledge && !accepted} onclick={confirm}>
          {prompt.enabled ? 'Turn on' : 'Turn off'}
        </Button>
      </div>
    </div>
  </div>
{/if}
