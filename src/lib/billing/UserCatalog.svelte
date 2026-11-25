<script lang="ts">
  import { removeUser, saveUser } from '../api'
  import { session } from '../session.svelte'
  import type { PosSnapshot, PosUser } from '../types'

  interface Props {
    users: PosUser[]
    onChange: (pos: PosSnapshot) => void
  }

  let { users, onChange }: Props = $props()
  const isManager = $derived(session.user?.role === 'manager')

  let editing = $state<number | 'new' | null>(null)
  let loaded = $state<number | 'new' | null>(null)
  let name = $state('')
  let password = $state('')
  let role = $state<'cashier' | 'manager'>('manager')
  let active = $state(true)
  let formError = $state('')
  let confirmRemove = $state(false)
  let saving = $state(false)

  const selected = $derived(users.find((user) => user.id === editing) ?? null)

  $effect(() => {
    if (editing === loaded) return
    loaded = editing
    formError = ''
    confirmRemove = false
    password = ''
    const user = users.find((entry) => entry.id === editing) ?? null
    name = user?.name ?? ''
    role = user?.role === 'cashier' ? 'cashier' : 'manager'
    active = user?.active ?? true
  })

  function roleLabel(value: string) {
    return value === 'manager' ? 'Manager' : 'Cashier'
  }

  async function save() {
    if (!name.trim()) {
      formError = 'Enter a name.'
      return
    }
    if (editing === 'new' && password.trim().length < 4) {
      formError = 'Use at least 4 characters for the password.'
      return
    }
    if (editing !== 'new' && password.trim() && password.trim().length < 4) {
      formError = 'Use at least 4 characters for the password.'
      return
    }
    formError = ''
    saving = true
    try {
      const saved = await saveUser(editing === 'new' ? null : editing, {
        name: name.trim(),
        password: password.trim() || null,
        role,
        active,
      })
      onChange(saved.pos)
      loaded = saved.id
      editing = saved.id
      password = ''
    } catch (error) {
      formError = error instanceof Error ? error.message : 'Could not save the user.'
    } finally {
      saving = false
    }
  }

  async function remove() {
    if (editing === 'new' || editing == null) return
    saving = true
    try {
      onChange(await removeUser(editing))
      loaded = null
      editing = null
    } catch (error) {
      formError = error instanceof Error ? error.message : 'Could not remove the user.'
      confirmRemove = false
    } finally {
      saving = false
    }
  }
</script>

<div class="item-setup">
  <div class="setup-list-pane">
    <div class="catalog-head">
      <h2>Users</h2>
      <button type="button" class="save" onclick={() => (editing = 'new')}>New user</button>
    </div>
    <p class="hint">Cashiers and managers sign in with the name and password saved here. The till stays open without a sign-in.</p>
    <div class="setup-list">
      {#each users as user (user.id)}
        <button type="button" class:selected={user.id === editing} onclick={() => (editing = user.id)}>
          <strong>{user.name}</strong>
          <span>{roleLabel(user.role)}{user.active ? '' : ' · Off'}</span>
        </button>
      {:else}
        <p class="hint">No users yet. The first one should be a manager.</p>
      {/each}
    </div>
  </div>
  <div class="setup-detail">
    {#if editing == null}
      <h2>User</h2>
      <p class="hint">Select a person, or add one. A manager can keep the shop's users and settings.</p>
    {:else}
      <h2>{editing === 'new' ? 'New user' : name || 'User'}</h2>
      <label>
        Name
        <input autocomplete="off" bind:value={name} />
      </label>
      <label>
        Password
        <input type="password" autocomplete="new-password" bind:value={password} />
      </label>
      {#if selected}<p class="hint">Leave the password blank to keep the current one.</p>{/if}
      <div class="prompt-choices" role="radiogroup" aria-label="Role">
        <button type="button" role="radio" class:active={role === 'cashier'} aria-checked={role === 'cashier'} onclick={() => (role = 'cashier')}>Cashier</button>
        <button type="button" role="radio" class:active={role === 'manager'} aria-checked={role === 'manager'} onclick={() => (role = 'manager')}>Manager</button>
      </div>
      <label class="switch-row">
        <button
          type="button"
          class="switch"
          class:on={active}
          role="switch"
          aria-checked={active}
          aria-label="Account is on"
          onclick={() => (active = !active)}
        ><i></i></button>
        {active ? 'Account is on' : 'Account is off'}
      </label>
      {#if formError}<p class="field-error">{formError}</p>{/if}
      <div class="sheet-actions">
        {#if isManager && editing !== 'new'}
          {#if confirmRemove}
            <button type="button" class="link" disabled={saving} onclick={remove}>Remove user</button>
            <button type="button" class="ghost light" disabled={saving} onclick={() => (confirmRemove = false)}>Cancel</button>
          {:else}
            <button type="button" class="link" disabled={saving} onclick={() => (confirmRemove = true)}>Remove</button>
          {/if}
        {/if}
        <button type="button" class="save" disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save'}</button>
      </div>
    {/if}
  </div>
</div>
