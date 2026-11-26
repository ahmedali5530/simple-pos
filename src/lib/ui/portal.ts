/** Move a node to `document.body` so fixed overlays escape shell layout. */
export function portal(node: HTMLElement) {
  document.body.appendChild(node)
  return {
    destroy() {
      node.remove()
    },
  }
}
