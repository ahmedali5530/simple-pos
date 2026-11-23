import { registerItemOptionsProvider } from '../../platform/itemOptions'
import { listDevices, listModels } from './api'

let itemIds: Set<number> | null = null
let pending: Promise<void> | null = null
let generation = 0

async function load(gen: number) {
  let ok = false
  const ids = new Set<number>()
  try {
    const body = await listModels()
    for (const model of body.models) ids.add(model.item_id)
    ok = true
  } catch {
    // Keep going: in-stock devices can still mark an item as a phone.
  }
  try {
    const body = await listDevices({ status: 'in_stock' })
    for (const device of body.devices) ids.add(device.item_id)
    ok = true
  } catch {
    // A read failure must not block the till; retry next time.
  }
  if (gen === generation) itemIds = ok ? ids : null
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

export function hasMobileItemOptions(itemId: number) {
  return itemIds?.has(itemId) ?? false
}

/** A model was saved or cleared in the catalog; reload on the next till. */
export function invalidateMobileItemOptions() {
  generation += 1
  itemIds = null
  pending = null
}

registerItemOptionsProvider({
  plugin: 'mobile',
  prepare,
  hasOptions: hasMobileItemOptions,
  reset: invalidateMobileItemOptions,
})
