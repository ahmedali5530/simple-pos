<script lang="ts">
  import { session } from '../session.svelte'

  let { src, class: className = '' }: { src: string | null; class?: string } = $props()
  let shown = $state('')

  $effect(() => {
    const url = src
    const token = session.user?.token
    if (!url) {
      shown = ''
      return
    }
    if (url.startsWith('blob:') || url.startsWith('data:')) {
      shown = url
      return
    }
    let cancelled = false
    let objectUrl = ''
    const headers = new Headers()
    if (token) headers.set('authorization', `Bearer ${token}`)
    fetch(url, { headers })
      .then((response) => (response.ok ? response.blob() : Promise.reject(new Error('picture'))))
      .then((blob) => {
        if (cancelled) return
        objectUrl = URL.createObjectURL(blob)
        shown = objectUrl
      })
      .catch(() => {
        if (!cancelled) shown = ''
      })
    return () => {
      cancelled = true
      if (objectUrl) URL.revokeObjectURL(objectUrl)
    }
  })
</script>

{#if shown}
  <img src={shown} alt="" class={className} />
{/if}
