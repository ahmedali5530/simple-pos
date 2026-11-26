import type {
  BarcodeRule,
  CashDesk,
  Category,
  Portion,
  PosItem,
  PosSection,
  PosSnapshot,
  PosUser,
  PostedBill,
  Reports,
  SectionChoice,
  SectionKind,
  ShopSnapshot,
  StockBalance,
  StockMovement,
} from './types'
import { request } from '../platform/http'
import { session, signOut } from './session.svelte'

export { request }

export function loadPos(): Promise<PosSnapshot> {
  return request('/api/v1/pos/snapshot')
}

export function loadCoreSnapshot(): Promise<ShopSnapshot> {
  return request('/api/v1/core/snapshot')
}

/** Raw catalog contract snapshot (no POS dependency). */
export interface CatalogApiSnapshot {
  sections: {
    id: number
    name: string
    short_name: string
    kind: string
    accent: string
    show_on_pos: boolean
    pos_order: number
  }[]
  categories: { id: number; name: string; sort_order: number }[]
  items: {
    id: number
    section_id: number
    category_id: number
    category_name: string
    barcode: string | null
    name: string
    rate: number
    cost: number
    stock: number
    portion: string
    group_name: string | null
    signature: boolean
    prompt: string
    base_rate: number | null
    scale: number
    image_url: string | null
    variants: {
      id: number
      item_id: number
      name: string
      rate: number
      units: number
      barcode: string | null
    }[]
  }[]
}

function asSectionKind(value: string): SectionKind {
  return value === 'food' ? 'food' : 'retail'
}

function asPortion(value: string): Portion {
  if (value === 'full' || value === 'half') return value
  return 'regular'
}

function asPrompt(value: string): PosItem['prompt'] {
  if (value === 'quantity' || value === 'price') return value
  return 'none'
}

/** Build the catalog settings view from catalog + core (no POS plugin). */
export function catalogViewFrom(
  catalog: CatalogApiSnapshot,
  shop?: Pick<ShopSnapshot, 'shop_name' | 'currency' | 'locale'> | null,
): PosSnapshot {
  const items: PosItem[] = catalog.items.map((item) => ({
    id: item.id,
    section_id: item.section_id,
    category_id: item.category_id,
    category_name: item.category_name,
    barcode: item.barcode,
    name: item.name,
    rate: item.rate,
    cost: item.cost,
    stock: item.stock,
    portion: asPortion(item.portion),
    group_name: item.group_name,
    signature: item.signature,
    prompt: asPrompt(item.prompt),
    base_rate: item.base_rate,
    scale: item.scale,
    image_url: item.image_url,
    variants: item.variants,
  }))
  const all_sections: SectionChoice[] = catalog.sections.map((section) => ({
    id: section.id,
    name: section.name,
    short_name: section.short_name,
    kind: asSectionKind(section.kind),
    accent: section.accent,
    show_on_pos: section.show_on_pos,
    pos_order: section.pos_order,
  }))
  const sections: PosSection[] = all_sections.map((section) => ({
    id: section.id,
    name: section.name,
    short_name: section.short_name,
    kind: section.kind,
    accent: section.accent,
    items: items.filter((item) => item.section_id === section.id),
  }))
  const categories: Category[] = catalog.categories.map((category) => ({
    id: category.id,
    name: category.name,
    sort_order: category.sort_order,
  }))
  return {
    shop_name: shop?.shop_name ?? 'Shop',
    bill_prefix: 'BILL',
    currency: shop?.currency ?? 'PKR',
    locale: shop?.locale ?? 'en-PK',
    barcode_rules: [],
    sections,
    all_sections,
    categories,
    last_section_id: null,
  }
}

/** Catalog settings refresh — does not call POS. */
export async function loadCatalog(): Promise<PosSnapshot> {
  const [catalog, shop] = await Promise.all([
    request<CatalogApiSnapshot>('/api/v1/catalog/snapshot'),
    loadCoreSnapshot().catch(() => null),
  ])
  return catalogViewFrom(catalog, shop)
}

export function rememberSection(sectionId: number): Promise<{ last_section_id: number }> {
  return request('/api/v1/pos/focus', {
    method: 'PUT',
    body: JSON.stringify({ section_id: sectionId }),
  })
}

export function saveSectionLayout(
  sections: { id: number; show_on_pos: boolean }[],
): Promise<PosSnapshot> {
  return request('/api/v1/pos/sections', {
    method: 'PUT',
    body: JSON.stringify({ sections }),
  })
}

export interface CatalogVariant {
  id: number | null
  name: string
  rate: number
  units: number
  barcode: string | null
}

export interface CatalogItem {
  name: string
  barcode: string | null
  rate: number
  cost: number
  section_id: number
  category_id: number
  prompt: 'none' | 'quantity' | 'price'
  base_rate: number | null
  scale: number
  variants: CatalogVariant[]
  item_type?: 'product' | 'service'
  unit_id?: number | null
}

export async function saveCatalogItem(
  itemId: number | null,
  body: CatalogItem,
): Promise<{ item_id: number; pos: PosSnapshot }> {
  const path = itemId == null ? '/api/v1/catalog/items' : `/api/v1/catalog/items/${itemId}`
  const saved = await request<{ item_id: number }>(path, {
    method: itemId == null ? 'POST' : 'PUT',
    body: JSON.stringify(body),
  })
  return { item_id: saved.item_id, pos: await loadCatalog() }
}

export async function saveItemPicture(
  itemId: number,
  file: File,
): Promise<{ item_id: number; pos: PosSnapshot }> {
  const body = new FormData()
  body.append('picture', file)
  const saved = await request<{ item_id: number }>(`/api/v1/catalog/items/${itemId}/image`, { method: 'POST', body })
  return { item_id: saved.item_id, pos: await loadCatalog() }
}

export async function removeItemPicture(itemId: number): Promise<{ item_id: number; pos: PosSnapshot }> {
  const saved = await request<{ item_id: number }>(`/api/v1/catalog/items/${itemId}/image`, { method: 'DELETE' })
  return { item_id: saved.item_id, pos: await loadCatalog() }
}

export async function saveItemPrompt(
  itemId: number,
  body: { prompt: 'none' | 'quantity' | 'price'; base_rate: number | null; scale: number },
): Promise<PosSnapshot> {
  await request(`/api/v1/catalog/items/${itemId}/prompt`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
  return loadCatalog()
}

export interface SavedRecord {
  id: number
  pos: PosSnapshot
}

export async function saveSection(
  id: number | null,
  body: { name: string; short_name: string; kind: 'retail' | 'food'; accent: string },
): Promise<SavedRecord> {
  const path = id == null ? '/api/v1/catalog/sections' : `/api/v1/catalog/sections/${id}`
  const saved = await request<{ id: number }>(path, { method: id == null ? 'POST' : 'PUT', body: JSON.stringify(body) })
  return { id: saved.id, pos: await loadCatalog() }
}

export async function removeSection(id: number): Promise<PosSnapshot> {
  await request(`/api/v1/catalog/sections/${id}`, { method: 'DELETE' })
  return loadCatalog()
}

export async function orderSections(ids: number[]): Promise<PosSnapshot> {
  await request('/api/v1/catalog/sections/order', { method: 'PUT', body: JSON.stringify({ ids }) })
  return loadCatalog()
}

export async function saveCategory(id: number | null, name: string): Promise<SavedRecord> {
  const path = id == null ? '/api/v1/catalog/categories' : `/api/v1/catalog/categories/${id}`
  const saved = await request<{ id: number }>(path, {
    method: id == null ? 'POST' : 'PUT',
    body: JSON.stringify({ name }),
  })
  return { id: saved.id, pos: await loadCatalog() }
}

export async function removeCategory(id: number): Promise<PosSnapshot> {
  await request(`/api/v1/catalog/categories/${id}`, { method: 'DELETE' })
  return loadCatalog()
}

export async function orderCategories(ids: number[]): Promise<PosSnapshot> {
  await request('/api/v1/catalog/categories/order', { method: 'PUT', body: JSON.stringify({ ids }) })
  return loadCatalog()
}

export function saveUser(
  id: number | null,
  body: { name: string; password: string | null; role: 'cashier' | 'manager'; active: boolean },
): Promise<PosUser[]> {
  const path = id == null ? '/api/users' : `/api/users/${id}`
  return request(path, { method: id == null ? 'POST' : 'PUT', body: JSON.stringify(body) })
}

export function loadUsers(): Promise<PosUser[]> {
  return request('/api/users')
}

export function removeUser(id: number): Promise<PosUser[]> {
  return request(`/api/users/${id}`, { method: 'DELETE' })
}

export async function saveShopProfile(body: {
  name: string
  currency: string
  locale: string
  bill_prefix: string
  barcode_rules: { prefix: string; mode: 'weight' | 'price'; scale: number }[]
}): Promise<ShopSnapshot & { bill_prefix: string; barcode_rules: BarcodeRule[] }> {
  const shop = await request<ShopSnapshot>('/api/v1/core/settings', {
    method: 'PUT',
    body: JSON.stringify({
      name: body.name,
      currency: body.currency,
      locale: body.locale,
    }),
  })
  let bill_prefix = body.bill_prefix
  let barcode_rules = body.barcode_rules
  try {
    const pos = await request<PosSnapshot>('/api/v1/pos/settings', {
      method: 'PUT',
      body: JSON.stringify({
        bill_prefix: body.bill_prefix,
        barcode_rules: body.barcode_rules,
      }),
    })
    bill_prefix = pos.bill_prefix
    barcode_rules = pos.barcode_rules
  } catch {
    // POS plugin may be off; profile still saved.
  }
  return { ...shop, bill_prefix, barcode_rules }
}

export interface PrinterSettings {
  enabled: boolean
  kind: 'network' | 'usb' | 'serial' | string
  target: string
  port: number
}

export function loadPrinterSettings(): Promise<PrinterSettings> {
  return request('/api/v1/printing/settings')
}

export function savePrinter(body: {
  print_enabled: boolean
  printer_kind: string
  printer_target: string
  printer_port: number
}): Promise<PrinterSettings> {
  return request('/api/v1/printing/settings', { method: 'PUT', body: JSON.stringify(body) })
}

export async function adjustStock(body: {
  item_id: number
  qty: number
  direction: 'in' | 'out'
  note: string | null
  update_cost?: boolean
  total?: number | null
}): Promise<{ pos: PosSnapshot; movements: StockMovement[] }> {
  const adjusted = await request<{ movements: StockMovement[] }>('/api/v1/inventory/adjust', {
    method: 'POST',
    body: JSON.stringify(body),
  })
  return { movements: adjusted.movements, pos: await loadCatalog() }
}

export interface ItemImportResult {
  created: number
  updated: number
  options: number
  counted: number
}

export interface MovementImportResult {
  applied: number
  pos: PosSnapshot
}

export async function downloadWorkbook(path: string, filename: string) {
  const headers = new Headers()
  if (session.user?.token) headers.set('authorization', `Bearer ${session.user.token}`)
  const response = await fetch(path, { headers })
  if (response.status === 401) signOut()
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as { error?: string }
    throw new Error(body.error || 'The workbook did not download.')
  }
  const blob = await response.blob()
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  link.click()
  URL.revokeObjectURL(url)
}

export interface CategoryImportResult {
  created: number
  updated: number
  removed: number
  reordered: boolean
}

export function importCategoriesWorkbook(file: File): Promise<CategoryImportResult> {
  const body = new FormData()
  body.append('file', file)
  return request('/api/v1/catalog/categories/import', { method: 'POST', body })
}

export function importItemsWorkbook(file: File): Promise<ItemImportResult> {
  const body = new FormData()
  body.append('file', file)
  return request('/api/v1/catalog/items/import', { method: 'POST', body })
}

export async function importMovementsWorkbook(file: File): Promise<MovementImportResult> {
  const body = new FormData()
  body.append('file', file)
  const imported = await request<{ applied: number }>('/api/v1/catalog/movements/import', { method: 'POST', body })
  return { applied: imported.applied, pos: await loadCatalog() }
}

export function loadStockSummary(): Promise<StockBalance[]> {
  return request('/api/v1/inventory/summary')
}

export function loadMovements(itemId?: number): Promise<StockMovement[]> {
  const query = itemId == null ? '' : `?item_id=${itemId}`
  return request(`/api/v1/inventory/movements${query}`)
}

export function loadReports(from?: string, to?: string): Promise<Reports> {
  const params = new URLSearchParams()
  if (from && to) {
    params.set('from', from)
    params.set('to', to)
  }
  const query = params.size ? `?${params}` : ''
  return request(`/api/v1/reports/summary${query}`)
}

export function loadCashDesk(): Promise<CashDesk> {
  return request('/api/v1/cash/desk')
}

export function openCashShift(openingCash: number, pettyCash: number): Promise<CashDesk> {
  return request('/api/v1/cash/open', {
    method: 'POST',
    body: JSON.stringify({ opening_cash: openingCash, petty_cash: pettyCash }),
  })
}

export interface CashShiftBody {
  expenses: { description: string; category: string; amount: number }[]
  cash_taken: number
  manual_count: number | null
}

export function saveCashDraft(body: CashShiftBody): Promise<CashDesk> {
  return request('/api/v1/cash/draft', { method: 'POST', body: JSON.stringify(body) })
}

export function closeCashShift(body: CashShiftBody): Promise<CashDesk> {
  return request('/api/v1/cash/close', { method: 'POST', body: JSON.stringify(body) })
}

export function login(name: string, password: string): Promise<PosUser & { token: string }> {
  return request('/api/login', { method: 'POST', body: JSON.stringify({ name, password }) })
}

export function logout(): Promise<void> {
  return request('/api/logout', { method: 'POST' })
}

export function currentSession(): Promise<PosUser> {
  return request('/api/session')
}

export function loadBootstrap(): Promise<{ needs_user: boolean; shop_name: string }> {
  return request('/api/bootstrap')
}

export function postBill(
  sectionId: number,
  lines: {
    item_id: number
    variant_id: number | null
    rate: number
    qty: number
    code: string | null
    entry: string | null
    source?: string | null
    reference?: string | null
    details?: Record<string, unknown> | null
  }[],
  customer: string | null,
  tenders: {
    method: string
    amount: number
    reference?: string | null
    details?: Record<string, unknown> | null
  }[] = [],
): Promise<PostedBill> {
  return request('/api/v1/pos/bills', {
    method: 'POST',
    body: JSON.stringify({ section_id: sectionId, lines, customer, tenders }),
  })
}
