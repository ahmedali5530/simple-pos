<script lang="ts">
  import { loadBootstrap, login, saveUser } from './api'
  import { navigate } from './route.svelte'
  import { signIn } from './session.svelte'

  let name = $state('')
  let password = $state('')
  let message = $state('')
  let sending = $state(false)
  let needsUser = $state(false)
  let shopName = $state('Shop')

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
      message = 'Enter a name and a password.'
      return
    }
    sending = true
    message = ''
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
      navigate('/billing')
    } catch (error) {
      message = error instanceof Error ? error.message : 'Could not sign in.'
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
    <label>
      Name
      <input autocomplete="username" bind:value={name} />
    </label>
    <label>
      Password
      <input type="password" autocomplete="current-password" bind:value={password} />
    </label>
    {#if message}<p class="field-error">{message}</p>{/if}
    <div class="sheet-actions">
      <button type="submit" class="save" disabled={sending}>{sending ? 'Signing in…' : needsUser ? 'Create and sign in' : 'Sign in'}</button>
    </div>
  </form>
</div>
