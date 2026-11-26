<script lang="ts">
  import type { PosLineSourceContext } from '../../../platform/slots'

  let { addLine }: PosLineSourceContext = $props()

  let code = $state('')
  let itemId = $state('')
  let message = $state('')

  function issue() {
    message = ''
    const trimmed = code.trim().toUpperCase()
    const id = Number(itemId)
    if (!trimmed) {
      message = 'Enter a card code.'
      return
    }
    if (!Number.isInteger(id) || id < 1) {
      message = 'Pick a catalog item id for the gift face value.'
      return
    }
    const ok = addLine({
      item_id: id,
      qty: 1,
      source: 'gift.issue',
      reference: `card:${trimmed}`,
      details: { kind: 'gift.issue' },
      label: `Gift card ${trimmed}`,
    })
    if (ok) {
      code = ''
      message = `Added gift card ${trimmed}.`
    } else {
      message = 'Could not add that gift card line.'
    }
  }
</script>

<div class="gift-issue">
  <strong>Issue gift card</strong>
  <div class="row">
    <input class="input" placeholder="Card code" maxlength="64" bind:value={code} />
    <input class="input" placeholder="Item id" inputmode="numeric" bind:value={itemId} />
    <button type="button" class="btn" onclick={issue}>Add</button>
  </div>
  {#if message}<p class="hint">{message}</p>{/if}
</div>

<style>
  .gift-issue {
    display: grid;
    gap: 0.35rem;
    margin: 0.4rem 0;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }
  .input {
    min-width: 7rem;
    flex: 1;
  }
  .hint {
    margin: 0;
    font-size: 0.85rem;
    opacity: 0.8;
  }
</style>
