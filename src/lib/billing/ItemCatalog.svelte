<script lang="ts">
  import { downloadWorkbook, importItemsWorkbook, request, type CatalogItem } from '../api'
  import ItemPicture from './ItemPicture.svelte'
  import VirtualList from './VirtualList.svelte'
  import { centsToInput, formatCost, formatMoney, moneyToCents } from '../money'
  import type { Category, PosItem, PosSection, PosSnapshot } from '../types'
  import { toast, toastError, toastSaved } from '../toast'
  import { Button, Input, Segmented, Select } from '../ui'

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
  let itemType = $state<'product' | 'service'>('product')
  let unitId = $state('')
  let units = $state<{ id: number; name: string }[]>([])

  $effect(() => {
    request<{ units: { id: number; name: string }[] }>('/api/v1/catalog/units')
      .then((body) => {
        units = body.units ?? []
      })
      .catch(() => {
        units = []
      })
  })
  let baseRate = $state('')
  let scale = $state('1')
  let variants = $state<DraftOption[]>([])
  let loaded = $state<number | 'new' | null>(null)
  let pictureFile = $state<File | null>(null)
  let pictureRemoved = $state(false)
  let picturePreview = $state<string | null>(null)
  let sheetBusy = $state(false)

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
    if (editing === 'new') {
      fill(null)
      return
    }
    fill(selected?.item ?? null)
  })

  async function exportWorkbook() {
    sheetBusy = true
    try {
      await downloadWorkbook('/api/items/export', 'items.xlsx')
      toastSaved('The item workbook downloaded.')
    } catch (error) {
      toastError(error, 'The workbook did not download.')
    } finally {
      sheetBusy = false
    }
  }

  async function importWorkbook(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
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
      toastSaved(parts.length ? `Saved the workbook: ${parts.join(', ')}.` : 'The workbook matched the catalog.')
    } catch (error) {
      toastError(error, 'The workbook was not saved.')
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
      toast.error('Use a JPEG, PNG, WebP, or GIF.')
      return
    }
    if (file.size > 2 * 1024 * 1024) {
      toast.error('Use a picture under 2 MB.')
      return
    }
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
      toast.error('Enter an item name.')
      return
    }
    if (!whole(rate) || Number(rate) < 1) {
      toast.error('Enter a shelf rate above 0.')
      return
    }
    const costCents = cost.trim() === '' ? 0 : moneyToCents(cost)
    if (costCents == null) {
      toast.error('Enter a cost of 0 or more, with up to 2 decimals.')
      return
    }
    if (!sectionId || !categoryId) {
      toast.error('Choose a section and a category.')
      return
    }
    const asking = variants.length ? 'none' : prompt
    let nextBase: number | null = null
    let nextScale = 1
    if (asking !== 'none') {
      if (!whole(baseRate) || Number(baseRate) < 1) {
        toast.error('Enter a base rate above 0.')
        return
      }
      if (!whole(scale) || Number(scale) < 1) {
        toast.error('Stock per 1 must be at least 1.')
        return
      }
      nextBase = Number(baseRate)
      nextScale = Number(scale)
    }
    const options = []
    for (const variant of variants) {
      if (!variant.name.trim()) {
        toast.error('Name each option.')
        return
      }
      if (!whole(variant.rate) || Number(variant.rate) < 1) {
        toast.error(`${variant.name.trim()} needs a rate above 0.`)
        return
      }
      if (!whole(variant.units) || Number(variant.units) < 1) {
        toast.error(`${variant.name.trim()} must take at least 1 from stock.`)
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
          item_type: itemType,
          unit_id: unitId ? Number(unitId) : null,
        },
        { file: pictureFile, remove: pictureRemoved },
      )
      loaded = result.item.id
      editing = result.item.id
      fill(result.item)
      if (result.pictureError) toast.error(result.pictureError)
      else toastSaved('Item saved.')
    } catch (error) {
      toastError(error, 'Could not save the item.')
    }
  }
</script>

<div class="item-setup">
  <div class="setup-list-pane">
    <div class="catalog-head">
      <h2>Items</h2>
      <Button onclick={() => (editing = 'new')}>New item</Button>
    </div>
    <div class="mt-2.5 grid grid-cols-2 gap-2">
      <Button variant="secondary" class="w-full" disabled={sheetBusy} onclick={exportWorkbook}>Export Excel</Button>
      <label class="btn btn-secondary w-full">
        Import Excel
        <input class="hidden" type="file" accept=".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" onchange={importWorkbook} />
      </label>
    </div>
    <p class="hint">Leave counted blank unless you recounted that item. Options are the second sheet.</p>
    <Input placeholder="Find an item" bind:value={query} />
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
            <label class="btn btn-secondary">
              {picturePreview ? 'Change picture' : 'Add picture'}
              <input class="hidden" type="file" accept="image/jpeg,image/png,image/webp,image/gif" onchange={choosePicture} />
            </label>
            {#if picturePreview}
              <Button variant="link" class="text-danger" onclick={clearPicture}>Remove picture</Button>
            {/if}
          </div>
          <p class="hint">Shown on the item card when a picture is attached. JPEG, PNG, WebP, or GIF, under 2 MB.</p>
        </div>
      </div>
      <label>
        Name
        <Input bind:value={name} />
      </label>
      <label>
        Barcode
        <Input bind:value={barcode} />
      </label>
      <label>
        Shelf rate
        <Input inputmode="numeric" bind:value={rate} />
      </label>
      <label>
        Cost
        <Input inputmode="decimal" bind:value={cost} placeholder="0" />
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
        <Select bind:value={sectionId}>
          {#each sections as section (section.id)}
            <option value={String(section.id)}>{section.short_name}</option>
          {/each}
        </Select>
      </label>
      <label>
        Category
        <Select bind:value={categoryId}>
          {#each categories as category (category.id)}
            <option value={String(category.id)}>{category.name}</option>
          {/each}
        </Select>
      </label>
      <label>
        Type
        <Select bind:value={itemType}>
          <option value="product">Product</option>
          <option value="service">Service</option>
        </Select>
      </label>
      <label>
        Unit
        <Select bind:value={unitId}>
          <option value="">Piece</option>
          {#each units as unit (unit.id)}
            <option value={String(unit.id)}>{unit.name}</option>
          {/each}
        </Select>
      </label>

      <div class="option-block">
        <h3>Options</h3>
        <p class="hint">Piece and pack, or full and half. Each option has its own rate and takes its own amount from stock.</p>
        {#each variants as variant, index (variant.id ?? `new-${index}`)}
          <div class="variant-edit">
            <label>
              Name
              <Input bind:value={variant.name} />
            </label>
            <label>
              Rate
              <Input inputmode="numeric" bind:value={variant.rate} />
            </label>
            <label>
              Stock units
              <Input inputmode="numeric" bind:value={variant.units} />
            </label>
            <label>
              Barcode
              <Input bind:value={variant.barcode} />
            </label>
            <Button variant="link" class="text-danger" onclick={() => removeOption(index)}>Remove</Button>
          </div>
        {/each}
        <Button variant="secondary" class="my-3" onclick={addOption}>Add option</Button>
      </div>

      {#if variants.length}
        <p class="field-error">This item sells by its options, so the cashier is not asked for a quantity or a price.</p>
      {:else}
        <Segmented
          label="What the cashier enters"
          value={prompt}
          onchange={(next) => (prompt = next as typeof prompt)}
          options={[
            { value: 'none', label: 'Off' },
            { value: 'quantity', label: 'Quantity / weight' },
            { value: 'price', label: 'Price' },
          ]}
        />
        {#if prompt !== 'none'}
          <label>
            Base rate
            <Input inputmode="numeric" bind:value={baseRate} />
          </label>
          <label>
            Stock per 1
            <Input inputmode="numeric" bind:value={scale} />
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
      <div class="sheet-actions">
        <Button disabled={saving} onclick={save}>
          {saving ? 'Saving…' : 'Save'}
        </Button>
      </div>
    {/if}
  </div>
</div>
