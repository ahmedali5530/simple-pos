import type { CartLine, PosItem, PosSection } from '../lib/types'

/** Save plugin-owned item data after the catalog item has an id. Throw with a user-facing message. */
export type ExtraSave = (itemId: number) => Promise<void>

/**
 * Register an extra save under a stable plugin id. The callback receives the saved item id and may
 * be registered before a new item exists. The returned function unregisters it.
 */
export type RegisterExtraSave = (id: string, save: ExtraSave) => () => void

export interface CatalogItemFormContext {
  item: PosItem | null
  itemId: number | null
  editing: number | 'new' | null
  registerExtraSave: RegisterExtraSave
}

/** A line a plugin adds to the current bill. The host resolves the catalog item and price. */
export interface PosLineDraft {
  item_id: number
  variant_id?: number | null
  rate?: number
  qty?: number
  source: string
  reference?: string
  details?: Record<string, unknown>
  label?: string
}

export type AddPosLine = (draft: PosLineDraft) => boolean

export interface PosLineSourceContext {
  section: PosSection
  addLine: AddPosLine
  currency: string
  locale: string
  /** The current cart, so a source can hide references already on the bill. */
  lines: CartLine[]
}

/** Options a plugin contributes inside the POS variant sheet for one catalog item. */
export interface PosItemOptionsContext {
  item: PosItem
  section: PosSection
  lines: CartLine[]
  addLine: AddPosLine
  close: () => void
  currency: string
  locale: string
}

/** Stock In/Out form extension when a catalog item is selected. */
export interface InventoryStockFormContext {
  item: PosItem
  direction: 'in' | 'out'
  kind: string
  currency: string
  locale: string
  onAdjusted: () => void | Promise<void>
}

/** Extra rows under the live stock summary table. */
export interface InventoryStockSummaryContext {
  currency: string
  locale: string
  headId: string
  query: string
  refreshKey: number
}

export type LineMetadataPatch = Partial<Pick<CartLine, 'source' | 'reference' | 'details'>>

export interface PosLineDetailContext {
  line: CartLine
  patch: (patch: LineMetadataPatch) => void
  currency: string
  locale: string
}

/** How the current bill will be paid. Empty tenders mean implied cash for the total. */
export interface PosTender {
  method: string
  amount: number
  reference?: string
  details?: Record<string, unknown>
}

export interface PosTenderContext {
  total: number
  currency: string
  locale: string
  tenders: PosTender[]
  setTenders: (tenders: PosTender[]) => void
}
