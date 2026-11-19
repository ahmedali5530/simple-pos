export interface Slice {
  start: number
  end: number
  before: number
  after: number
  total: number
}

export function sliceWindow(ends: number[], scrollTop: number, viewport: number, overscan: number): Slice {
  const count = Math.max(0, ends.length - 1)
  const total = ends[count] ?? 0
  const view = viewport > 0 ? viewport : 640
  if (count === 0) return { start: 0, end: 0, before: 0, after: 0, total: 0 }
  const from = Math.max(0, scrollTop - overscan)
  const to = scrollTop + view + overscan
  let low = 0
  let high = count
  while (low < high) {
    const mid = (low + high) >> 1
    if (ends[mid + 1] <= from) low = mid + 1
    else high = mid
  }
  let end = low
  while (end < count && ends[end] < to) end += 1
  const before = ends[low] ?? 0
  const after = Math.max(0, total - (ends[end] ?? total))
  return { start: low, end, before, after, total }
}

export function createVirtual(overscan = 400) {
  const heights = new Map<string, number>()
  let ends: number[] = [0]
  let count = 0
  let gap = 0
  let estimate = 48
  let keyOf: (index: number) => string = () => ''
  let node: HTMLElement | null = null
  let scrollTop = 0
  let viewH = 0
  let start = $state(0)
  let end = $state(0)
  let before = $state(0)
  let after = $state(0)
  let total = $state(0)
  let inner = $state(0)
  let placed = $state<number[]>([])
  let shown = { start: 0, end: 0, before: 0, after: 0, total: 0, placed: [] as number[] }

  function publish(next: Slice, tops: number[]) {
    if (
      next.start === shown.start &&
      next.end === shown.end &&
      next.before === shown.before &&
      next.after === shown.after &&
      next.total === shown.total &&
      tops.length === shown.placed.length &&
      tops.every((value, index) => value === shown.placed[index])
    ) {
      return
    }
    shown = { start: next.start, end: next.end, before: next.before, after: next.after, total: next.total, placed: tops }
    start = next.start
    end = next.end
    before = next.before
    after = next.after
    total = next.total
    placed = tops
  }

  function rebuild() {
    if (node) scrollTop = node.scrollTop
    ends = new Array(count + 1)
    ends[0] = 0
    for (let index = 0; index < count; index += 1) {
      const height = heights.get(keyOf(index)) ?? estimate
      ends[index + 1] = ends[index] + height + (index + 1 < count ? gap : 0)
    }
    const next = sliceWindow(ends, scrollTop, viewH, overscan)
    publish(next, ends.slice(next.start, next.end))
  }

  function configure<T>(items: T[], key: (item: T, index: number) => string | number, nextEstimate: number, nextGap = 0) {
    count = items.length
    estimate = nextEstimate
    gap = nextGap
    keyOf = (index) => String(key(items[index], index))
    rebuild()
  }

  function resetScroll() {
    scrollTop = 0
    if (node) node.scrollTop = 0
    rebuild()
  }

  function attach(element: HTMLElement) {
    node = element
    const onScroll = () => {
      scrollTop = element.scrollTop
      const next = sliceWindow(ends, scrollTop, element.clientHeight, overscan)
      publish(next, ends.slice(next.start, next.end))
    }
    element.addEventListener('scroll', onScroll, { passive: true })
    const observer = new ResizeObserver(() => {
      viewH = element.clientHeight
      const style = getComputedStyle(element)
      inner = Math.max(0, element.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight))
      rebuild()
    })
    observer.observe(element)
    viewH = element.clientHeight
    const style = getComputedStyle(element)
    inner = Math.max(0, element.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight))
    rebuild()
    return () => {
      element.removeEventListener('scroll', onScroll)
      observer.disconnect()
      if (node === element) node = null
    }
  }

  function measure(element: HTMLElement, id: string | number) {
    let token = String(id)
    const read = () => {
      const height = element.offsetHeight
      const previous = heights.get(token)
      if (height > 0 && (previous === undefined || Math.abs(height - previous) > 1)) {
        heights.set(token, height)
        rebuild()
      }
    }
    read()
    const observer = new ResizeObserver(read)
    observer.observe(element)
    return {
      update(next: string | number) {
        token = String(next)
        read()
      },
      destroy() {
        observer.disconnect()
      },
    }
  }

  return {
    get start() {
      return start
    },
    get end() {
      return end
    },
    get before() {
      return before
    },
    get after() {
      return after
    },
    get total() {
      return total
    },
    get inner() {
      return inner
    },
    get placed() {
      return placed
    },
    attach,
    measure,
    configure,
    resetScroll,
  }
}
