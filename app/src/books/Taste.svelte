<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Taste } from '../lib/api'

  let { taste, ondone }: { taste: Taste; ondone: (t: Taste) => void } = $props()
  let genres = $state<string[]>([])
  let picked = $state<string[]>([])
  let authors = $state('')
  let length = $state('medium')
  let step = $state(0)

  onMount(async () => {
    genres = await api.genres()
    picked = [...taste.genres]
    authors = taste.authors.join(', ')
    length = taste.length || 'medium'
  })

  async function save() {
    const t: Taste = { genres: picked, authors: authors.split(',').map((a) => a.trim()).filter(Boolean), length, done: true }
    await api.tasteSet(t)
    ondone(t)
  }
</script>

<div class="scrim" aria-hidden="true"></div>
<div class="card paper" role="dialog" aria-modal="true" aria-label="Your taste in books">
  <p class="stamp">Library card</p>
  {#if step === 0}
    <h2>What do you like to read?</h2>
    <p>Pick as many as you like.</p>
    <div class="chips">
      {#each genres as g}
        <button class="chip" aria-pressed={picked.includes(g)} onclick={() => (picked = picked.includes(g) ? picked.filter((x) => x !== g) : [...picked, g])}>{g}</button>
      {/each}
    </div>
    <button class="btn" onclick={() => (step = 1)} disabled={!picked.length}>Next</button>
  {:else if step === 1}
    <h2>Any authors you love?</h2>
    <p>Separate names with commas. Leave it empty if you'd rather discover.</p>
    <input bind:value={authors} placeholder="Jane Austen, Arthur Conan Doyle" />
    <div class="row"><button class="btn ghost" onclick={() => (step = 0)}>Back</button><button class="btn" onclick={() => (step = 2)}>Next</button></div>
  {:else}
    <h2>How long a read?</h2>
    <div class="chips">
      {#each [['short', 'Short, an evening or two'], ['medium', 'Medium, a week or so'], ['long', 'Long, a companion for months']] as [v, l]}
        <button class="chip" aria-pressed={length === v} onclick={() => (length = v)}>{l}</button>
      {/each}
    </div>
    <div class="row"><button class="btn ghost" onclick={() => (step = 1)}>Back</button><button class="btn" onclick={save}>Fill my shelf</button></div>
  {/if}
  <p class="small">Books come from Project Gutenberg and Standard Ebooks: free, public-domain, no account.</p>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgb(0 0 0 / 0.55); z-index: 40; }
  .card {
    position: fixed; z-index: 41; left: 50%; top: 50%; translate: -50% -50%;
    width: min(94vw, 620px); max-height: 90dvh; overflow: auto;
    padding: 1.6rem 1.6rem 1.2rem; border-radius: 6px; box-shadow: var(--shadow);
    display: grid; gap: 0.8rem; text-align: center; justify-items: center;
    border: 1px solid var(--paper-edge);
    animation: card 600ms var(--ease-spring) both;
  }
  .stamp { margin: 0; font-family: var(--font-type); color: var(--accent); border: 2px solid var(--accent); padding: 0.1rem 0.6rem; rotate: -3deg; text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.8rem; }
  h2 { font-family: var(--font-fell); font-weight: 400; font-size: 1.8rem; margin: 0; }
  p { margin: 0; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.45rem; justify-content: center; }
  input { width: 100%; font-family: var(--font-book); font-size: 1.05rem; padding: 0.6rem 0.4rem; border: 0; border-bottom: 1px solid var(--ink); background: transparent; color: var(--ink); }
  .row { display: flex; gap: 0.6rem; }
  .small { font-family: var(--font-type); font-size: 0.75rem; color: var(--ink-faint); }
  @keyframes card { from { opacity: 0; transform: translateY(40px) rotate(-2deg); } }
</style>
