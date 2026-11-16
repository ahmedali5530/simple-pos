export type SectionKind = 'retail' | 'food'
export type Portion = 'regular' | 'full' | 'half'

export interface SectionChoice {
  id: number
  name: string
  short_name: string
  kind: SectionKind
  accent: string
  show_on_pos: boolean
  pos_order: number
}

export interface Category {
  id: number
  name: string
  sort_order: number
}

export interface ItemVariant {
  id: number
  item_id: number
  name: string
  rate: number
  units: number
  barcode: string | null
}

export interface PosItem {
  id: number
  section_id: number
  category_id: number
  category_name: string
  barcode: string | null
  name: string
  rate: number
  stock: number
  portion: Portion
  group_name: string | null
  signature: boolean
  prompt: 'none' | 'quantity' | 'price'
  base_rate: number | null
  scale: number
  variants: ItemVariant[]
}

export interface PosSection {
  id: number
  name: string
  short_name: string
  kind: SectionKind
  accent: string
  items: PosItem[]
}

export interface BarcodeRule {
  prefix: string
  mode: 'weight' | 'price'
  scale: number
}

export interface PosSnapshot {
  shop_name: string
  currency: string
  locale: string
  barcode_rules: BarcodeRule[]
  sections: PosSection[]
  all_sections: SectionChoice[]
  categories: Category[]
  last_section_id: number | null
}

export interface CartLine {
  key: string
  item_id: number
  variant_id: number | null
  name: string
  rate: number
  units: number
  qty: number
  code: string | null
  entry: string | null
}

export interface PostedBill {
  bill_no: string
  total: number
  pos: PosSnapshot
}
