import { request } from '../../lib/api'

export interface MatrixGroupValue {
  id?: number
  value: string
  sort_order?: number
}

export interface MatrixGroup {
  id?: number
  name: string
  sort_order?: number
  values: MatrixGroupValue[]
}

export interface MatrixSku {
  variant_id: number
  item_id: number
  fingerprint: string
  name: string
  barcode: string | null
  rate: number
  stock: number
  avg_cost: number
  value: number
  parent_name: string
  section_id: number
  section_name: string
  accent: string
}

export interface MatrixRecipe {
  item_id: number
  enabled: boolean
  barcode_prefix: string
  groups: MatrixGroup[]
  skus: MatrixSku[]
}

export function listEnabledItems(): Promise<{ item_ids: number[] }> {
  return request('/api/v1/matrix/enabled')
}

export function getRecipe(itemId: number): Promise<MatrixRecipe> {
  return request(`/api/v1/matrix/items/${itemId}`)
}

export function saveRecipe(
  itemId: number,
  body: {
    enabled: boolean
    barcode_prefix: string
    groups: { name: string; sort_order?: number; values: { value: string; sort_order?: number }[] }[]
  },
): Promise<MatrixRecipe> {
  return request(`/api/v1/matrix/items/${itemId}`, { method: 'PUT', body: JSON.stringify(body) })
}

export function generateVariants(itemId: number): Promise<MatrixRecipe> {
  return request(`/api/v1/matrix/items/${itemId}/generate`, {
    method: 'POST',
    body: JSON.stringify({}),
  })
}

export function updateSku(
  variantId: number,
  body: { barcode?: string | null; rate?: number; stock?: number; avg_cost?: number },
): Promise<{ sku: MatrixSku }> {
  return request(`/api/v1/matrix/skus/${variantId}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export function adjustSku(
  variantId: number,
  body: {
    direction: 'in' | 'out'
    qty: number
    update_cost?: boolean
    total?: number | null
    note?: string | null
  },
): Promise<{ sku: MatrixSku }> {
  return request(`/api/v1/matrix/skus/${variantId}/adjust`, {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function loadSkuSummary(): Promise<{ skus: MatrixSku[] }> {
  return request('/api/v1/matrix/skus/summary')
}
