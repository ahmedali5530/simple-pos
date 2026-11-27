<script lang="ts">
  import { onMount } from 'svelte'
  import { Button, Input, Select, Switch } from '../../../lib/ui'
  import { loadFiscalSettings, saveFiscalSettings, type FiscalSettings } from '../api'

  let form = $state<FiscalSettings>({
    authority: 'fbr',
    mode: 'demo',
    ntn: '',
    strn: '',
    pos_id: '',
    api_token: '',
    api_base_url: '',
    business_name: '',
    province: '',
    address: '',
    scenario_id: 'SN001',
    hs_code: '0101.2100',
    uom: 'Numbers, pieces, units',
    sale_type: 'Goods at standard rate (default)',
    st_rate: '18%',
    tax_inclusive: true,
  })
  let error = $state('')
  let saved = $state('')
  let busy = $state(false)

  onMount(() => {
    loadFiscalSettings()
      .then((value) => (form = value))
      .catch((err: unknown) => {
        error = err instanceof Error ? err.message : 'Could not load fiscal settings.'
      })
  })

  async function save() {
    error = ''
    saved = ''
    busy = true
    try {
      form = await saveFiscalSettings(form)
      saved = 'Fiscal settings saved.'
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not save fiscal settings.'
    } finally {
      busy = false
    }
  }
</script>

<main class="stage">
  <header>
    <h1>Fiscal (FBR/PRA)</h1>
    <p>
      Demo prints a local QR only. Sandbox and Live post each sale to FBR Digital Invoicing and print the
      returned IRN as the receipt QR. PRA is not wired yet.
    </p>
  </header>

  <section class="form">
    <label>
      <span>Authority</span>
      <Select bind:value={form.authority}>
        <option value="fbr">FBR</option>
        <option value="pra">PRA (soon)</option>
      </Select>
    </label>
    <label>
      <span>Mode</span>
      <Select bind:value={form.mode}>
        <option value="demo">Demo (no network)</option>
        <option value="sandbox">FBR sandbox</option>
        <option value="live">FBR live</option>
      </Select>
    </label>
    <label>
      <span>Seller NTN / CNIC</span>
      <Input bind:value={form.ntn} placeholder="7 or 13 digits" />
    </label>
    <label>
      <span>Business name</span>
      <Input bind:value={form.business_name} placeholder="Defaults to shop name" />
    </label>
    <label>
      <span>Province</span>
      <Input bind:value={form.province} placeholder="e.g. Punjab" />
    </label>
    <label>
      <span>Address</span>
      <Input bind:value={form.address} placeholder="Seller address" />
    </label>
    <label>
      <span>API token</span>
      <Input bind:value={form.api_token} placeholder="Bearer token from FBR" autocomplete="off" />
    </label>
    <label>
      <span>Sandbox scenario</span>
      <Input bind:value={form.scenario_id} placeholder="SN001" />
    </label>
    <label>
      <span>Default HS code</span>
      <Input bind:value={form.hs_code} />
    </label>
    <label>
      <span>UoM</span>
      <Input bind:value={form.uom} />
    </label>
    <label>
      <span>Sale type</span>
      <Input bind:value={form.sale_type} />
    </label>
    <label>
      <span>Sales tax rate</span>
      <Input bind:value={form.st_rate} placeholder="18%" />
    </label>
    <label>
      <span>STRN (optional)</span>
      <Input bind:value={form.strn} />
    </label>
    <label>
      <span>POS ID (optional)</span>
      <Input bind:value={form.pos_id} />
    </label>
    <label class="wide">
      <span>API URL override (optional)</span>
      <Input
        bind:value={form.api_base_url}
        placeholder="Leave blank for official FBR sandbox/live URLs"
      />
    </label>
    <div class="wide">
      <Switch bind:checked={form.tax_inclusive} label="Prices include sales tax" />
    </div>
  </section>

  {#if error}<p class="error">{error}</p>{/if}
  {#if saved}<p class="ok">{saved}</p>{/if}

  <div class="actions">
    <Button disabled={busy} onclick={save}>{busy ? 'Saving…' : 'Save'}</Button>
  </div>
</main>

<style>
  .stage {
    padding: 1rem 1.25rem;
    display: grid;
    gap: 0.85rem;
  }
  header h1 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 600;
  }
  header p {
    margin: 0.35rem 0 0;
    color: var(--ui-text-muted);
    max-width: 42rem;
    line-height: 1.4;
  }
  .form {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: 0.75rem 1rem;
  }
  label {
    display: grid;
    gap: 0.3rem;
    font-size: 0.75rem;
    color: var(--ui-text-muted);
  }
  label.wide,
  .wide {
    grid-column: 1 / -1;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
  }
  .error {
    margin: 0;
    color: var(--ui-danger, tomato);
  }
  .ok {
    margin: 0;
    color: var(--ui-success, seagreen);
  }
</style>
