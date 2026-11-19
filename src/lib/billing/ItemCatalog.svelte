<script lang="ts">
  import { downloadWorkbook, importItemsWorkbook, type CatalogItem } from '../api'
  import ItemPicture from './ItemPicture.svelte'
  import VirtualList from './VirtualList.svelte'
  import { centsToInput, formatCost, formatMoney, moneyToCents } from '../money'
  import type { Category, PosItem, PosSection, PosSnapshot } from '../types'

  interface Props {
    sections: PosSection[]
    categories: Category[]
    currency: string
    locale: string
    saving: boolean
    onSave: (
      itemId: number | null,
      body: CatalogItem,
      picture: { file: File | null; remove: boolean },
    ) => Promise<{ item: PosItem; pictureError: string | null }>
    onImported: (pos: PosSnapshot) => void
  }

  let { sections, categories, currency, locale, saving, onSave, onImported }: Props = $props()

  interface DraftOption {
    id: number | null
    name: string
    rate: string
    units: string
    barcode: string
  }

  let query = $state('')
  let editing = $state<number | 'new' | null>(null)
  let name = $state('')
  let barcode = $state('')
  let rate = $state('')
  let cost = $state('')
  let sectionId = $state('')
  let categoryId = $state('')
  let prompt = $state<'none' | 'quantity' | 'price'>('none')
  let baseRate = $state('')
  let scale = $state('1')
  let variants = $state<DraftOption[]>([])
  let formError = $state('')
  let loaded = $state<number | 'new' | null>(null)
  let pictureFile = $state<File | null>(null)
  let pictureRemoved = $state(false)
  let picturePreview = $state<string | null>(null)
  let sheetBusy = $state(false)
  let sheetNote = $state('')
  let sheetError = $state('')

  const shelfProfit = $derived.by(() => {
    const cents = cost.trim() === '' ? 0 : moneyToCents(cost)
    if (cents == null || !/^\d+$/.test(rate.trim())) return null
    return Number(rate) * 100 - cents
  })
  const rows = $derived(sections.flatMap((section) => section.items.map((item) => ({ item, section }))))
  const visible = $derived.by(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return rows
    return rows.filter((row) => {
      const codes = [row.item.barcode ?? '', ...row.item.variants.map((variant) => variant.barcode ?? '')]
      return (
        row.item.name.toLowerCase().includes(needle) ||
        codes.some((code) => code.toLowerCase().includes(needle))
      )
    })
  })
  const selected = $derived(rows.find((row) => row.item.id === editing) ?? null)

  $effect(() => {
    if (editing === loaded) return
    loaded = editing
    formError = ''
    if (editing === 'new') {
      fill(null)
      return
    }
    fill(selected?.item ?? null)
  })

  async function exportWorkbook() {
    sheetError = ''
    sheetNote = ''
    sheetBusy = true
    try {
      await downloadWorkbook('/api/items/export', 'items.xlsx')
      sheetNote = 'The item workbook downloaded.'
    } catch (error) {
      sheetError = error instanceof Error ? error.message : 'The workbook did not download.'
    } finally {
      sheetBusy = false
    }
  }

  async function importWorkbook(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    sheetError = ''
    sheetNote = ''
    sheetBusy = true
    try {
      const result = await importItemsWorkbook(file)
      onImported(result.pos)
      const parts = [
        result.created ? `${result.created} added` : '',
        result.updated ? `${result.updated} updated` : '',
        result.options ? `${result.options} options saved` : '',
        result.counted ? `${result.counted} stock counts saved` : '',
      ].filter(Boolean)
      sheetNote = parts.length ? `Saved the workbook: ${parts.join(', ')}.` : 'The workbook matched the catalog.'
    } catch (error) {
      sheetError = error instanceof Error ? error.message : 'The workbook was not saved.'
    } finally {
      sheetBusy = false
    }
  }

  function releasePreview() {
    if (picturePreview?.startsWith('blob:')) URL.revokeObjectURL(picturePreview)
  }

  function fill(item: PosItem | null) {
    releasePreview()
    pictureFile = null
    pictureRemoved = false
    picturePreview = item?.image_url ?? null
    name = item?.name ?? ''
    barcode = item?.barcode ?? ''
    rate = item ? String(item.rate) : ''
    cost = item ? centsToInput(item.cost) : ''
    sectionId = String(item?.section_id ?? sections[0]?.id ?? '')
    categoryId = String(item?.category_id ?? categories[0]?.id ?? '')
    prompt = item?.prompt ?? 'none'
    baseRate = item?.base_rate == null ? '' : String(item.base_rate)
    scale = String(item?.scale || 1)
    variants = (item?.variants ?? []).map((variant) => ({
      id: variant.id,
      name: variant.name,
      rate: String(variant.rate),
      units: String(variant.units),
      barcode: variant.barcode ?? '',
    }))
  }

  function choosePicture(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const file = input.files?.[0] ?? null
    input.value = ''
    if (!file) return
    const allowed = !file.type || ['image/jpeg', 'image/png', 'image/webp', 'image/gif'].includes(file.type)
    if (!allowed) {
      formError = 'Use a JPEG, PNG, WebP, or GIF.'
      return
    }
    if (file.size > 2 * 1024 * 1024) {
      formError = 'Use a picture under 2 MB.'
      return
    }
    formError = ''
    releasePreview()
    pictureFile = file
    pictureRemoved = false
    picturePreview = URL.createObjectURL(file)
  }

  function clearPicture() {
    releasePreview()
    pictureFile = null
    pictureRemoved = true
    picturePreview = null
  }

  function addOption() {
    prompt = 'none'
    variants = [...variants, { id: null, name: '', rate: '', units: '1', barcode: '' }]
  }

  function removeOption(index: number) {
    variants = variants.filter((_, itemIndex) => itemIndex !== index)
  }

  function summary(item: PosItem) {
    if (item.variants.length) return item.variants.map((variant) => variant.name).join(' · ')
    if (item.prompt === 'quantity') return 'Asks quantity / weight'
    if (item.prompt === 'price') return 'Asks price'
    return formatMoney(item.rate, currency, locale)
  }

  async function save() {
    const whole = (value: string) => /^\d+$/.test(value.trim())
    if (!name.trim()) {
      formError = 'Enter an item name.'
      return
    }
    if (!whole(rate) || Number(rate) < 1) {
      formError = 'Enter a shelf rate above 0.'
      return
    }
    const costCents = cost.trim() === '' ? 0 : moneyToCents(cost)
    if (costCents == null) {
      formError = 'Enter a cost of 0 or more, with up to 2 decimals.'
      return
    }
    if (!sectionId || !categoryId) {
      formError = 'Choose a section and a category.'
      return
    }
    const asking = variants.length ? 'none' : prompt
    let nextBase: number | null = null
    let nextScale = 1
    if (asking !== 'none') {
      if (!whole(baseRate) || Number(baseRate) < 1) {
        formError = 'Enter a base rate above 0.'
        return
      }
      if (!whole(scale) || Number(scale) < 1) {
        formError = 'Stock per 1 must be at least 1.'
        return
      }
      nextBase = Number(baseRate)
      nextScale = Number(scale)
    }
    const options = []
    for (const variant of variants) {
      if (!variant.name.trim()) {
        formError = 'Name each option.'
        return
      }
      if (!whole(variant.rate) || Number(variant.rate) < 1) {
        formError = `${variant.name.trim()} needs a rate above 0.`
        return
      }
      if (!whole(variant.units) || Number(variant.units) < 1) {
        formError = `${variant.name.trim()} must take at least 1 from stock.`
        return
      }
      options.push({
        id: variant.id,
        name: variant.name.trim(),
        rate: Number(variant.rate),
        units: Number(variant.units),
        barcode: variant.barcode.trim() || null,
      })
    }
    formError = ''
    try {
      const result = await onSave(
        editing === 'new' ? null : editing,
        {
          name: name.trim(),
          barcode: barcode.trim() || null,
          rate: Number(rate),
          cost: costCents,
          section_id: Number(sectionId),
          category_id: Number(categoryId),
          prompt: asking,
          base_rate: nextBase,
          scale: nextScale,
          variants: options,
        },
        { file: pictureFile, remove: pictureRemoved },
      )
      loaded = result.item.id
      editing = result.item.id
      fill(result.item)
      if (result.pictureError) formError = result.pictureError
    } catch (error) {
      formError = error instanceof Error ? error.message : 'Could not save the item.'
    }
  }
</script>

<div class="item-setup">
  <div class="setup-list-pane">
    <div class="catalog-head">
      <h2>Items</h2>
      <button type="button" class="save" onclick={() => (editing = 'new')}>New item</button>
    </div>
    <div class="mt-2.5 grid grid-cols-2 gap-2">
      <button type="button" class="inline-flex h-[46px] items-center justify-center rounded-xl border border-slate-300 bg-white px-3 text-center text-sm font-extrabold leading-none whitespace-nowrap text-slate-900" disabled={sheetBusy} onclick={exportWorkbook}>Export Excel</button>
      <label class="mt-0 inline-flex h-[46px] cursor-pointer items-center justify-center rounded-xl border border-slate-300 bg-white px-3 text-center text-sm font-extrabold leading-none whitespace-nowrap text-slate-900">
        Import Excel
        <input class="hidden" type="file" accept=".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" onchange={importWorkbook} />
      </label>
    </div>
    <p class="hint">Leave counted blank unless you recounted that item. Options are the second sheet.</p>
    {#if sheetNote}<p class="mt-2 text-[13px] font-bold text-emerald-700">{sheetNote}</p>{/if}
    {#if sheetError}<p class="field-error">{sheetError}</p>{/if}
    <input class="setup-search" placeholder="Find an item" bind:value={query} />
    <VirtualList class="setup-list" items={visible} estimate={64} reset={query} key={(row) => row.item.id}>
      {#snippet children(row)}
        <button type="button" class:selected={row.item.id === editing} onclick={() => (editing = row.item.id)}>
          {#if row.item.image_url}
            <ItemPicture class="catalog-thumb" src={row.item.image_url} />
          {/if}
          <strong>{row.item.name}</strong>
          <span>{row.section.short_name} · {summary(row.item)}</span>
        </button>
      {/snippet}
    </VirtualList>
  </div>
  <div class="setup-detail">
    {#if editing == null}
      <h2>Catalog</h2>
      <p class="hint">Select an item, or add one. Each item belongs to one section and one category.</p>
    {:else}
      <h2>{editing === 'new' ? 'New item' : name || 'Item'}</h2>
      {#if selected}
        <p class="hint">{selected.item.stock} on hand.</p>
      {:else}
        <p class="hint">A new item starts with nothing on hand.</p>
      {/if}
      <div class="picture-field">
        {#if picturePreview}
          <ItemPicture class="picture-preview" src={picturePreview} />
        {:else}
          <span class="picture-empty" aria-hidden="true"></span>
        {/if}
        <div class="picture-actions">
          <div class="picture-buttons">
            <label class="picture-pick">
              {picturePreview ? 'Change picture' : 'Add picture'}
              <input type="file" accept="image/jpeg,image/png,image/webp,image/gif" onchange={choosePicture} />
            </label>
            {#if picturePreview}
              <button type="button" class="picture-remove" onclick={clearPicture}>Remove picture</button>
            {/if}
          </div>
          <p class="hint">Shown on the item card when a picture is attached. JPEG, PNG, WebP, or GIF, under 2 MB.</p>
        </div>
      </div>
      <label>
        Name
        <input bind:value={name} />
      </label>
      <label>
        Barcode
        <input bind:value={barcode} />
      </label>
      <label>
        Shelf rate
        <input inputmode="numeric" bind:value={rate} />
      </label>
      <label>
        Cost
        <input inputmode="decimal" bind:value={cost} placeholder="0" />
      </label>
      <p class="hint">
        {#if shelfProfit == null}
          Cost of one unit in stock. A purchase updates this average.
        {:else if cost.trim() === '' || moneyToCents(cost) === 0}
          No cost yet, so this item adds nothing to stock value or profit.
        {:else}
          Profit {formatCost(shelfProfit, currency, locale)} on each sale at the shelf rate. An option costs this average times the stock units it takes.
        {/if}
      </p>
      <label>
        Section
        <select bind:value={sectionId}>
          {#each sections as section (section.id)}
            <option value={String(section.id)}>{section.short_name}</option>
          {/each}
        </select>
      </label>
      <label>
        Category
        <select bind:value={categoryId}>
          {#each categories as category (category.id)}
            <option value={String(category.id)}>{category.name}</option>
          {/each}
        </select>
      </label>

      <div class="option-block">
        <h3>Options</h3>
        <p class="hint">Piece and pack, or full and half. Each option has its own rate and takes its own amount from stock.</p>
        {#each variants as variant, index (variant.id ?? `new-${index}`)}
          <div class="variant-edit">
            <label>
              Name
              <input bind:value={variant.name} />
            </label>
            <label>
              Rate
              <input inputmode="numeric" bind:value={variant.rate} />
            </label>
            <label>
              Stock units
              <input inputmode="numeric" bind:value={variant.units} />
            </label>
            <label>
              Barcode
              <input bind:value={variant.barcode} />
            </label>
            <button type="button" class="link" onclick={() => removeOption(index)}>Remove</button>
          </div>
        {/each}
        <button type="button" class="ghost my-3" onclick={addOption}>Add option</button>
      </div>

      {#if variants.length}
        <p class="field-error">This item sells by its options, so the cashier is not asked for a quantity or a price.</p>
      {:else}
        <div class="prompt-choices" role="radiogroup" aria-label="What the cashier enters">
          <button type="button" class:active={prompt === 'none'} aria-checked={prompt === 'none'} role="radio" onclick={() => (prompt = 'none')}>Off</button>
          <button type="button" class:active={prompt === 'quantity'} aria-checked={prompt === 'quantity'} role="radio" onclick={() => (prompt = 'quantity')}>Quantity / weight</button>
          <button type="button" class:active={prompt === 'price'} aria-checked={prompt === 'price'} role="radio" onclick={() => (prompt = 'price')}>Price</button>
        </div>
        {#if prompt !== 'none'}
          <label>
            Base rate
            <input inputmode="numeric" bind:value={baseRate} />
          </label>
          <label>
            Stock per 1
            <input inputmode="numeric" bind:value={scale} />
          </label>
          <p class="hint">
            {#if prompt === 'quantity'}
              Amount is the base rate times what they type. One of that value removes this many from stock.
            {:else}
              They type the price. Stock removed is that price times stock-per-1, divided by the base rate.
            {/if}
          </p>
        {/if}
      {/if}
      {#if formError}<p class="field-error">{formError}</p>{/if}
      <div class="sheet-actions">
        <button type="button" class="save" disabled={saving} onclick={save}>
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    {/if}
  </div>
</div>
