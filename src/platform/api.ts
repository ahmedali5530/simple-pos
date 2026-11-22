import { session, signOut } from './session.svelte'
import type { CashDesk, PosSnapshot, PosUser, PostedBill, Reports, StockBalance, StockMovement } from './types'

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers)
  if (!(init?.body instanceof FormData) && !headers.has('content-type') && init?.body) {
    headers.set('content-type', 'application/json')
  }
  if (session.user?.token && !headers.has('authorization')) {
    headers.set('authorization', `Bearer ${session.user.token}`)
  }
  const response = await fetch(path, {
    ...init,
    headers,
  })
  if (response.status === 204) return undefined as T
  const body = (await response.json().catch(() => ({}))) as T & { error?: string }
  if (response.status === 401) signOut()
  if (!response.ok) {
    throw new Error(body.error || `Request failed (${response.status})`)
  }
  return body
}

export function loadPos(): Promise<PosSnapshot> {
  return request('/api/pos')
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
}

export function saveCatalogItem(
  itemId: number | null,
  body: CatalogItem,
): Promise<{ item_id: number; pos: PosSnapshot }> {
  const path = itemId == null ? '/api/items' : `/api/items/${itemId}`
  return request(path, {
    method: itemId == null ? 'POST' : 'PUT',
    body: JSON.stringify(body),
  })
}

export function saveItemPicture(
  itemId: number,
  file: File,
): Promise<{ item_id: number; pos: PosSnapshot }> {
  const body = new FormData()
  body.append('picture', file)
  return request(`/api/items/${itemId}/image`, { method: 'POST', body })
}

export function removeItemPicture(itemId: number): Promise<{ item_id: number; pos: PosSnapshot }> {
  return request(`/api/items/${itemId}/image`, { method: 'DELETE' })
}

export function saveItemPrompt(
  itemId: number,
  body: { prompt: 'none' | 'quantity' | 'price'; base_rate: number | null; scale: number },
): Promise<PosSnapshot> {
  return request(`/api/items/${itemId}/prompt`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export interface SavedRecord {
  id: number
  pos: PosSnapshot
}

export function saveSection(
  id: number | null,
  body: { name: string; short_name: string; kind: 'retail' | 'food'; accent: string },
): Promise<SavedRecord> {
  const path = id == null ? '/api/sections' : `/api/sections/${id}`
  return request(path, { method: id == null ? 'POST' : 'PUT', body: JSON.stringify(body) })
}

export function removeSection(id: number): Promise<PosSnapshot> {
  return request(`/api/sections/${id}`, { method: 'DELETE' })
}

export function orderSections(ids: number[]): Promise<PosSnapshot> {
  return request('/api/sections/order', { method: 'PUT', body: JSON.stringify({ ids }) })
}

export function saveCategory(id: number | null, name: string): Promise<SavedRecord> {
  const path = id == null ? '/api/categories' : `/api/categories/${id}`
  return request(path, {
    method: id == null ? 'POST' : 'PUT',
    body: JSON.stringify({ name }),
  })
}

export function removeCategory(id: number): Promise<PosSnapshot> {
  return request(`/api/categories/${id}`, { method: 'DELETE' })
}

export function orderCategories(ids: number[]): Promise<PosSnapshot> {
  return request('/api/categories/order', { method: 'PUT', body: JSON.stringify({ ids }) })
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

export function saveSettings(body: {
  name: string
  currency: string
  locale: string
  bill_prefix: string
  barcode_rules: { prefix: string; mode: 'weight' | 'price'; scale: number }[]
  print_enabled: boolean
  printer_kind: string
  printer_target: string
  printer_port: number
}): Promise<PosSnapshot> {
  return request('/api/settings', { method: 'PUT', body: JSON.stringify(body) })
}

export function adjustStock(body: {
  item_id: number
  qty: number
  direction: 'in' | 'out'
  note: string | null
  update_cost?: boolean
  total?: number | null
}): Promise<{ pos: PosSnapshot; movements: StockMovement[] }> {
  return request('/api/stock', { method: 'POST', body: JSON.stringify(body) })
}

export interface ItemImportResult {
  created: number
  updated: number
  options: number
  counted: number
  pos: PosSnapshot
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
  pos: PosSnapshot
}

export function importCategoriesWorkbook(file: File): Promise<CategoryImportResult> {
  const body = new FormData()
  body.append('file', file)
  return request('/api/categories/import', { method: 'POST', body })
}

export function importItemsWorkbook(file: File): Promise<ItemImportResult> {
  const body = new FormData()
  body.append('file', file)
  return request('/api/items/import', { method: 'POST', body })
}

export function importMovementsWorkbook(file: File): Promise<MovementImportResult> {
  const body = new FormData()
  body.append('file', file)
  return request('/api/stock/import', { method: 'POST', body })
}

export function loadStockSummary(): Promise<StockBalance[]> {
  return request('/api/stock/summary')
}

export function loadMovements(itemId?: number): Promise<StockMovement[]> {
  const query = itemId == null ? '' : `?item_id=${itemId}`
  return request(`/api/stock/movements${query}`)
}

export function loadReports(from?: string, to?: string): Promise<Reports> {
  const params = new URLSearchParams()
  if (from && to) {
    params.set('from', from)
    params.set('to', to)
  }
  const query = params.size ? `?${params}` : ''
  return request(`/api/reports${query}`)
}

export function loadCashDesk(): Promise<CashDesk> {
  return request('/api/cash')
}

export function openCashShift(openingCash: number, pettyCash: number): Promise<CashDesk> {
  return request('/api/cash/open', {
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
  return request('/api/cash/draft', { method: 'POST', body: JSON.stringify(body) })
}

export function closeCashShift(body: CashShiftBody): Promise<CashDesk> {
  return request('/api/cash/close', { method: 'POST', body: JSON.stringify(body) })
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
  }[],
  customer: string | null,
): Promise<PostedBill> {
  return request('/api/v1/pos/bills', {
    method: 'POST',
    body: JSON.stringify({ section_id: sectionId, lines, customer }),
  })
}
