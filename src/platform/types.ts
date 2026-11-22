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
  cost: number
  stock: number
  portion: Portion
  group_name: string | null
  signature: boolean
  prompt: 'none' | 'quantity' | 'price'
  base_rate: number | null
  scale: number
  image_url: string | null
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

export interface PosUser {
  id: number
  name: string
  role: 'cashier' | 'manager' | string
  active: boolean
}

export interface PrinterSettings {
  enabled: boolean
  kind: 'network' | 'usb' | 'serial' | string
  target: string
  port: number
}

export interface PosSnapshot {
  shop_name: string
  bill_prefix: string
  currency: string
  locale: string
  barcode_rules: BarcodeRule[]
  printer: PrinterSettings
  sections: PosSection[]
  all_sections: SectionChoice[]
  categories: Category[]
  users: PosUser[]
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
  /** Plugin-owned line metadata, passed through to billing and sales.bill_posted. */
  source?: string | null
  reference?: string | null
  details?: Record<string, unknown> | null
}

export interface PostedBill {
  bill_no: string
  total: number
  pos: PosSnapshot
}

export interface StockBalance {
  id: number
  barcode: string | null
  name: string
  section_id: number
  section_name: string
  accent: string
  rate: number
  cost: number
  value: number
  stock_in: number
  stock_out: number
  stock: number
}

export interface StockMovement {
  id: number
  item_id: number
  item_name: string
  section_name: string
  qty: number
  reason: string
  bill_no: string | null
  created_at: string
}

export interface SectionTotal {
  id: number
  name: string
  short_name: string
  accent: string
  bills: number
  total: number
}

export interface BillLineView {
  name: string
  qty: number
  amount: number
}

export interface BillView {
  id: number
  bill_no: string
  section_name: string
  short_name: string
  accent: string
  total: number
  cost: number
  created_at: string
  cashier_name: string | null
  customer: string | null
  lines: BillLineView[]
}

export interface MixLine {
  id: number
  barcode: string | null
  name: string
  section_name: string
  qty_in: number
  qty_out: number
  opening: number
}

export interface Reports {
  sections: SectionTotal[]
  bills: BillView[]
  movements: StockMovement[]
  bill_count: number
  total: number
  stock_units: number
  stock_value: number
  profit: number
  bills_truncated: boolean
  movements_truncated: boolean
  mix: MixLine[]
  mix_truncated: boolean
  range_label: string
}

export interface CashExpense {
  description: string
  category: string
  amount: number
}

export interface CashShift {
  id: number
  opened_by: string
  closed_by: string | null
  opened_at: string
  closed_at: string | null
  opening_cash: number
  petty_cash: number
  sales: number
  expenses: number | null
  cash_taken: number | null
  manual_count: number | null
  system_count: number | null
  difference: number | null
  cash_left: number | null
  lines: CashExpense[]
}

export interface CashDesk {
  open: CashShift | null
  suggested_opening: number
  history: CashShift[]
}
