<script lang="ts">
  import { request } from '../../../lib/api'
  import { toast, toastError, toastSaved } from '../../../lib/toast'

  interface Unit {
    id: number
    code: string
    name: string
    factor: number
  }

  let units = $state<Unit[]>([])
  let code = $state('')
  let name = $state('')
  async function load() {
    const body = await request<{ units: Unit[] }>('/api/v1/catalog/units')
    units = body.units ?? []
  }

  $effect(() => {
    load().catch((error: unknown) => {
      toastError(error, 'Could not load units.')
    })
  })

  async function save(event: SubmitEvent) {
    event.preventDefault()
    try {
      await request('/api/v1/catalog/units', {
        method: 'POST',
        body: JSON.stringify({ code: code.trim(), name: name.trim(), base_code: code.trim() || 'pc', factor: 1 }),
      })
      code = ''
      name = ''
      await load()
      toastSaved('Unit saved.')
    } catch (error) {
      toastError(error, 'Could not save that unit.')
    }
  }
</script>

<section class="place">
  <h2>Units</h2>
  <p class="hint">Units describe how a product is sold. The stock multiplier on an option is unchanged.</p>
  <ul class="plugin-list">
    {#each units as unit (unit.id)}
      <li>
        <strong>{unit.name}</strong>
        <span class="hint">{unit.code} · ×{unit.factor}</span>
      </li>
    {/each}
  </ul>
  <form class="sheet" onsubmit={save}>
    <label>Code <input bind:value={code} /></label>
    <label>Name <input bind:value={name} /></label>
    <button type="submit" class="save">Add unit</button>
  </form>
</section>
