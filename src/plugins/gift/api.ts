import { request } from '../../platform/http'

export interface GiftCard {
  id: number
  code: string
  balance: number
  status: string
  issued_bill_no: string
  issued_at: string | null
}

export interface GiftLiability {
  cards: number
  outstanding: number
  rows: GiftCard[]
}

export interface GiftBalance {
  code: string
  balance: number
  status: string
}

export function listGiftCards(): Promise<GiftCard[]> {
  return request('/api/v1/gift/cards')
}

export function giftBalance(code: string): Promise<GiftBalance> {
  const query = new URLSearchParams({ code })
  return request(`/api/v1/gift/balance?${query}`)
}

export function giftLiability(): Promise<GiftLiability> {
  return request('/api/v1/gift/liability')
}

export function voidGiftCard(code: string): Promise<GiftCard> {
  return request('/api/v1/gift/void', {
    method: 'POST',
    body: JSON.stringify({ code }),
  })
}
