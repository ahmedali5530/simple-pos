<script lang="ts">
  import { removeUser, saveUser } from '../../../lib/api'
  import type { PosSnapshot, PosUser } from '../../../lib/types'
  import { toast, toastError, toastSaved } from '../../../lib/toast'
  import { Button, Input, Segmented, Switch } from '../../../lib/ui'

  interface Props {
    users: PosUser[]
    onChange: (pos: PosSnapshot) => void
  }

  let { users, onChange }: Props = $props()

  let editing = $state<number | 'new' | null>(null)
  let loaded = $state<number | 'new' | null>(null)
  let name = $state('')
  let password = $state('')
  let role = $state<'cashier' | 'manager'>('manager')
  let active = $state(true)
  let confirmRemove = $state(false)
  let saving = $state(false)

  const selected = $derived(users.find((user) => user.id === editing) ?? null)

  $effect(() => {
    if (editing === loaded) return
    loaded = editing
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
      toast.error('Enter a name.')
      return
    }
    if (editing === 'new' && password.trim().length < 4) {
      toast.error('Use at least 4 characters for the password.')
      return
    }
    if (editing !== 'new' && password.trim() && password.trim().length < 4) {
      toast.error('Use at least 4 characters for the password.')
      return
    }
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
      toastSaved('User saved.')
    } catch (error) {
      toastError(error, 'Could not save the user.')
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
      toastSaved('User removed.')
    } catch (error) {
      toastError(error, 'Could not remove the user.')
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
      <Button onclick={() => (editing = 'new')}>New user</Button>
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
        <Input autocomplete="off" bind:value={name} />
      </label>
      <label>
        Password
        <Input type="password" autocomplete="new-password" bind:value={password} />
      </label>
      {#if selected}<p class="hint">Leave the password blank to keep the current one.</p>{/if}
      <Segmented
        label="Role"
        value={role}
        onchange={(next) => (role = next as typeof role)}
        options={[
          { value: 'cashier', label: 'Cashier' },
          { value: 'manager', label: 'Manager' },
        ]}
      />
      <Switch bind:checked={active} label={active ? 'Account is on' : 'Account is off'} />
      <div class="sheet-actions">
        {#if editing !== 'new'}
          {#if confirmRemove}
            <Button variant="link" class="text-danger" disabled={saving} onclick={remove}>Remove user</Button>
            <Button variant="secondary" disabled={saving} onclick={() => (confirmRemove = false)}>Cancel</Button>
          {:else}
            <Button variant="link" class="text-danger" disabled={saving} onclick={() => (confirmRemove = true)}>Remove</Button>
          {/if}
        {/if}
        <Button disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save'}</Button>
      </div>
    {/if}
  </div>
</div>
