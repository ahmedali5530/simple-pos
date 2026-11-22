import type { CashDesk, PosSnapshot, PosUser, PostedBill, Reports, StockBalance, StockMovement } from './types'
import { request } from '../platform/http'
import { session, signOut } from './session.svelte'

export { request }

export function loadPos(): Promise<PosSnapshot> {
  return request('/api/v1/pos/snapshot')
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
  return { item_id: saved.item_id, pos: await loadPos() }
}

export async function saveItemPicture(
  itemId: number,
  file: File,
): Promise<{ item_id: number; pos: PosSnapshot }> {
  const body = new FormData()
  body.append('picture', file)
  const saved = await request<{ item_id: number }>(`/api/v1/catalog/items/${itemId}/image`, { method: 'POST', body })
  return { item_id: saved.item_id, pos: await loadPos() }
}

export async function removeItemPicture(itemId: number): Promise<{ item_id: number; pos: PosSnapshot }> {
  const saved = await request<{ item_id: number }>(`/api/v1/catalog/items/${itemId}/image`, { method: 'DELETE' })
  return { item_id: saved.item_id, pos: await loadPos() }
}

export async function saveItemPrompt(
  itemId: number,
  body: { prompt: 'none' | 'quantity' | 'price'; base_rate: number | null; scale: number },
): Promise<PosSnapshot> {
  await request(`/api/v1/catalog/items/${itemId}/prompt`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
  return loadPos()
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
  return { id: saved.id, pos: await loadPos() }
}

export async function removeSection(id: number): Promise<PosSnapshot> {
  await request(`/api/v1/catalog/sections/${id}`, { method: 'DELETE' })
  return loadPos()
}

export async function orderSections(ids: number[]): Promise<PosSnapshot> {
  await request('/api/v1/catalog/sections/order', { method: 'PUT', body: JSON.stringify({ ids }) })
  return loadPos()
}

export async function saveCategory(id: number | null, name: string): Promise<SavedRecord> {
  const path = id == null ? '/api/v1/catalog/categories' : `/api/v1/catalog/categories/${id}`
  const saved = await request<{ id: number }>(path, {
    method: id == null ? 'POST' : 'PUT',
    body: JSON.stringify({ name }),
  })
  return { id: saved.id, pos: await loadPos() }
}

export async function removeCategory(id: number): Promise<PosSnapshot> {
  await request(`/api/v1/catalog/categories/${id}`, { method: 'DELETE' })
  return loadPos()
}

export async function orderCategories(ids: number[]): Promise<PosSnapshot> {
  await request('/api/v1/catalog/categories/order', { method: 'PUT', body: JSON.stringify({ ids }) })
  return loadPos()
}

export function saveUser(
  id: number | null,
  body: { name: string; password: string | null; role: 'cashier' | 'manager'; active: boolean },
): Promise<SavedRecord> {
  const path = id == null ? '/api/users' : `/api/users/${id}`
  return request(path, { method: id == null ? 'POST' : 'PUT', body: JSON.stringify(body) })
}

export function removeUser(id: number): Promise<PosSnapshot> {
  return request(`/api/users/${id}`, { method: 'DELETE' })
}

export function saveShopProfile(body: {
  name: string
  currency: string
  locale: string
  bill_prefix: string
  barcode_rules: { prefix: string; mode: 'weight' | 'price'; scale: number }[]
}): Promise<PosSnapshot> {
  return request('/api/v1/platform/shop', { method: 'PUT', body: JSON.stringify(body) })
}

export function savePrinter(body: {
  print_enabled: boolean
  printer_kind: string
  printer_target: string
  printer_port: number
}): Promise<PosSnapshot> {
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
  return { movements: adjusted.movements, pos: await loadPos() }
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
  return { applied: imported.applied, pos: await loadPos() }
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
): Promise<PostedBill> {
  return request('/api/v1/pos/bills', {
    method: 'POST',
    body: JSON.stringify({ section_id: sectionId, lines, customer }),
  })
}
