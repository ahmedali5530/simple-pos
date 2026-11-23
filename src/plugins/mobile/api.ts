import { request } from '../../lib/api'

export interface MobileModel {
  item_id: number
  brand: string
  model: string
  warranty_months: number
}

export interface MobileDevice {
  id: number
  item_id: number
  imei: string
  serial: string
  status: 'in_stock' | 'sold'
  received_on: string
  warranty_months: number
  sold_bill_no: string
  sold_at: string | null
  customer_name: string
  brand: string
  model: string
  starts_on: string | null
  ends_on: string | null
}

export type ClaimKind = 'repair' | 'replacement'
export type ClaimStatus = 'open' | 'in_progress' | 'resolved' | 'rejected'

export interface MobileClaim {
  id: number
  device_id: number
  kind: ClaimKind
  status: ClaimStatus
  problem: string
  resolution: string
  cost: number
  opened_on: string
  closed_on: string | null
  imei: string
  serial: string
  brand: string
  model: string
  warranty_ends_on: string | null
}

export function listModels(): Promise<{ models: MobileModel[] }> {
  return request('/api/v1/mobile/models')
}

export function getModel(itemId: number): Promise<{ model: MobileModel | null }> {
  return request(`/api/v1/mobile/models/${itemId}`)
}

export function saveModel(
  itemId: number,
  body: { brand: string; model: string; warranty_months: number },
): Promise<{ model: MobileModel }> {
  return request(`/api/v1/mobile/models/${itemId}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export function clearModel(itemId: number): Promise<{ ok: boolean }> {
  return request(`/api/v1/mobile/models/${itemId}`, { method: 'DELETE' })
}

export function listDevices(
  params: { status?: string; search?: string; item_id?: number } = {},
): Promise<{
  devices: MobileDevice[]
  in_stock: number
  sold: number
}> {
  const query = new URLSearchParams()
  if (params.status) query.set('status', params.status)
  if (params.search) query.set('search', params.search)
  if (params.item_id) query.set('item_id', String(params.item_id))
  const suffix = query.toString()
  return request(`/api/v1/mobile/devices${suffix ? `?${suffix}` : ''}`)
}

export function addDevice(body: {
  item_id: number
  imei: string
  serial: string
  warranty_months?: number
}): Promise<{ id: number }> {
  return request('/api/v1/mobile/devices', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function listClaims(
  params: { status?: string; device_id?: number; search?: string } = {},
): Promise<{ claims: MobileClaim[] }> {
  const query = new URLSearchParams()
  if (params.status) query.set('status', params.status)
  if (params.device_id) query.set('device_id', String(params.device_id))
  if (params.search) query.set('search', params.search)
  const suffix = query.toString()
  return request(`/api/v1/mobile/claims${suffix ? `?${suffix}` : ''}`)
}

export function openClaim(body: {
  device_id: number
  kind: ClaimKind
  problem: string
  cost: number
}): Promise<{ claim: MobileClaim }> {
  return request('/api/v1/mobile/claims', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateClaim(
  id: number,
  body: { status: ClaimStatus; resolution?: string; cost?: number },
): Promise<{ claim: MobileClaim }> {
  return request(`/api/v1/mobile/claims/${id}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}
