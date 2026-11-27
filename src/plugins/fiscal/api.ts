import { request } from '../../platform/http'

export interface FiscalSettings {
  authority: string
  mode: string
  ntn: string
  strn: string
  pos_id: string
  api_token: string
  api_base_url: string
  business_name: string
  province: string
  address: string
  scenario_id: string
  hs_code: string
  uom: string
  sale_type: string
  st_rate: string
  tax_inclusive: boolean
}

export interface FiscalSubmission {
  id: number
  bill_id: number
  bill_no: string
  authority: string
  invoice_no: string
  qr_payload: string
  status: string
  error_message: string
  submitted_at: string
}

export function loadFiscalSettings(): Promise<FiscalSettings> {
  return request('/api/v1/fiscal/settings')
}

export function saveFiscalSettings(body: FiscalSettings): Promise<FiscalSettings> {
  return request('/api/v1/fiscal/settings', {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export function listFiscalSubmissions(): Promise<FiscalSubmission[]> {
  return request('/api/v1/fiscal/submissions')
}

export function retryFiscalSubmission(bill_no: string): Promise<FiscalSubmission> {
  return request('/api/v1/fiscal/retry', {
    method: 'POST',
    body: JSON.stringify({ bill_no }),
  })
}
