<script lang="ts">
  import { loadBootstrap, login, saveUser } from './api'
  import { navigate } from './route.svelte'
  import { navFirst } from '../platform/route.svelte'
  import { signIn } from './session.svelte'
  import { toast, toastError } from './toast'
  import { Button, Field, Input } from './ui'
  import type { UiNavItem } from './registry'

  let name = $state('')
  let password = $state('')
  let sending = $state(false)
  let needsUser = $state(false)
  let shopName = $state('Shop')
  let { nav }: { nav: UiNavItem[] } = $props()

  $effect(() => {
    document.title = shopName
  })

  $effect(() => {
    loadBootstrap()
      .then((status) => {
        needsUser = status.needs_user
        shopName = status.shop_name.trim() || 'Shop'
      })
      .catch(() => {})
  })

  function initials(name: string) {
    const words = name.trim().split(/\s+/).filter(Boolean)
    const letters = words.length >= 2 ? `${words[0][0]}${words[1][0]}` : name.slice(0, 2)
    return letters.toUpperCase() || 'SH'
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (!name.trim() || !password) {
      toast.error('Enter a name and a password.')
      return
    }
    sending = true
    try {
      if (needsUser) {
        await saveUser(null, {
          name: name.trim(),
          password,
          role: 'manager',
          active: true,
        })
      }
      const user = await login(name.trim(), password)
      signIn({ id: user.id, name: user.name, role: user.role, token: user.token })
      navigate(navFirst(nav))
    } catch (error) {
      toastError(error, 'Could not sign in.')
    } finally {
      sending = false
    }
  }
</script>

<div class="login">
  <form class="sheet" onsubmit={submit}>
    <div class="mark">{initials(shopName)}</div>
    <h1>{shopName}</h1>
    <p class="hint">{needsUser ? 'Create the first manager. This account opens the shop.' : 'Sign in to open the shop.'}</p>
    <Field label="Name">
      <Input autocomplete="username" bind:value={name} />
    </Field>
    <Field label="Password">
      <Input type="password" autocomplete="current-password" bind:value={password} />
    </Field>
    <div class="sheet-actions">
      <Button type="submit" disabled={sending}>{sending ? 'Signing in…' : needsUser ? 'Create and sign in' : 'Sign in'}</Button>
    </div>
  </form>
</div>
