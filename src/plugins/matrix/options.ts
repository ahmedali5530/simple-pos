import { registerItemOptionsProvider } from '../../platform/itemOptions'
import { listEnabledItems } from './api'

let itemIds: Set<number> | null = null
let pending: Promise<void> | null = null
let generation = 0

async function load(gen: number) {
  try {
    const body = await listEnabledItems()
    if (gen === generation) itemIds = new Set(body.item_ids)
  } catch {
    if (gen === generation) itemIds = null
  }
}

function prepare() {
  if (itemIds) return Promise.resolve()
  if (!pending) {
    const gen = generation
    pending = load(gen).finally(() => {
      if (gen === generation) pending = null
    })
  }
  return pending
}

export function hasMatrixItemOptions(itemId: number) {
  return itemIds?.has(itemId) ?? false
}

export function invalidateMatrixItemOptions() {
  generation += 1
  itemIds = null
  pending = null
}

registerItemOptionsProvider({
  plugin: 'matrix',
  prepare,
  hasOptions: hasMatrixItemOptions,
  exclusive: hasMatrixItemOptions,
  reset: invalidateMatrixItemOptions,
})
