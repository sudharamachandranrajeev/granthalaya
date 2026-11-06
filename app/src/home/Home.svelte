<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Edition, type Shelved } from '../lib/api'
  import { nav } from '../lib/nav.svelte'
  import { tilt } from '../lib/motion'

  let edition = $state<Edition | null>(null)
  let reading = $state<Shelved | null>(null)
  let lamp = $state({ x: 50, y: 20 })

  const now = new Date()
  const dateLine = now.toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })
  const iso = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`

  onMount(async () => {
    try { edition = await api.editionGet(iso) } catch { /* first run */ }
    try { reading = (await api.library())[0] ?? null } catch { /* empty shelf */ }
  })

  const lead = $derived(edition?.pages[0]?.stories[0]?.story.title)

  function moveLamp(e: PointerEvent) {
    lamp = { x: (e.clientX / window.innerWidth) * 100, y: (e.clientY / window.innerHeight) * 100 }
  }
</script>

<main class="desk" onpointermove={moveLamp} style="--lx:{lamp.x}%; --ly:{lamp.y}%">
  <header class="title">
    <h1>Granthālaya</h1>
    <p class="sub">{dateLine}</p>
  </header>

  <section class="cards" aria-label="Cards">
    <button class="card news" style="--i:0" use:tilt onclick={() => nav.go({ name: 'paper' })} aria-label="Open today's newspaper">
      <div class="sheet paper" style="view-transition-name: paper-sheet">
        <div class="mast">The Granthālaya Daily</div>
        <div class="dateline"><span>No. {edition?.number ?? '—'}</span><span>{iso}</span><span>Free · No ads</span></div>
        <h2 class="headline">{lead ?? 'Print today’s paper'}</h2>
        <div class="cols" aria-hidden="true">
          {#each Array(3) as _, c}
            <div class="col">
              {#each Array(c === 1 ? 5 : 7) as _, i}<span style="width:{70 + ((i * 37 + c * 11) % 30)}%"></span>{/each}
              {#if c === 1}<div class="photo"></div>{/if}
            </div>
          {/each}
        </div>
        <div class="fold" aria-hidden="true"></div>
      </div>
      <span class="label">Newspaper</span>
      <span class="hint">{edition ? `${edition.pages.length} pages · today's edition is ready` : 'World news from open sources, with the evidence behind every story'}</span>
      <span class="glare" aria-hidden="true"></span>
    </button>

    <button class="card books" style="--i:1" use:tilt onclick={() => nav.go({ name: 'library' })} aria-label="Open the library">
      <div class="cover" style="view-transition-name: books-cover">
        <div class="frame">
          <div class="crest" aria-hidden="true">❦</div>
          <div class="ctitle">The Library</div>
          <div class="rule"></div>
          {#if reading}
            <div class="reading">Now reading<br /><em>{reading.book.title}</em></div>
            <div class="bar"><span style="width:{Math.round(reading.progress * 100)}%"></span></div>
          {:else}
            <div class="reading">Open books, free to read</div>
          {/if}
        </div>
        <div class="ribbon" aria-hidden="true"></div>
      </div>
      <span class="label">Books</span>
      <span class="hint">{reading ? `${Math.round(reading.progress * 100)}% through ${reading.book.title}` : 'A shelf that learns your taste, and pages that rest your eyes'}</span>
      <span class="glare" aria-hidden="true"></span>
    </button>
  </section>

  <footer class="foot">No accounts · no ads · everything stays on this device</footer>
</main>

<style>
  .desk {
    min-height: 100dvh;
    padding: calc(env(safe-area-inset-top) + 2.5rem) 1.25rem 2rem;
    display: grid;
    grid-template-rows: auto 1fr auto;
    gap: 2rem;
    background:
      radial-gradient(circle at var(--lx) var(--ly), color-mix(in srgb, var(--desk-glow) 70%, transparent), transparent 45%),
      repeating-linear-gradient(92deg, rgb(255 255 255 / 0.018) 0 2px, transparent 2px 9px),
      repeating-linear-gradient(88deg, rgb(0 0 0 / 0.06) 0 1px, transparent 1px 23px),
      linear-gradient(160deg, var(--desk-2), var(--desk));
    transition: background-position 200ms;
    overflow-x: hidden;
  }
  .title { text-align: center; color: var(--paper); }
  .title h1 {
    font-family: var(--font-mast);
    font-weight: 400;
    font-size: clamp(2.6rem, 8vw, 4.8rem);
    margin: 0;
    letter-spacing: 0.01em;
    text-shadow: 0 2px 0 rgb(0 0 0 / 0.5), 0 0 30px rgb(196 154 69 / 0.25);
    animation: rise 900ms var(--ease-out) both;
  }
  .sub { font-family: var(--font-type); color: color-mix(in srgb, var(--paper) 70%, transparent); margin: 0.4rem 0 0; animation: rise 900ms 120ms var(--ease-out) both; }

  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 300px), 360px));
    justify-content: center;
    align-content: start;
    gap: clamp(1.5rem, 4vw, 3.5rem);
    perspective: 1400px;
  }
  /* Side by side on a wide screen: size the cards to the window height so both fit without scrolling. */
  @media (min-width: 640px) and (orientation: landscape) {
    .cards { grid-template-columns: repeat(2, clamp(220px, calc((100dvh - 21rem) * 0.75), 360px)); }
  }

  .card {
    --rx: 0deg; --ry: 0deg; --gx: 50%; --gy: 50%;
    position: relative;
    display: grid;
    gap: 0.4rem;
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    transform: rotateX(var(--rx)) rotateY(var(--ry)) translateZ(0);
    transform-style: preserve-3d;
    transition: translate 380ms var(--ease-spring), scale 380ms var(--ease-spring);
    animation: drop 900ms calc(200ms + var(--i) * 140ms) var(--ease-spring) both;
  }
  .card:hover { translate: 0 -8px; }
  .card:active { scale: 0.98; }
  .label { font-family: var(--font-head); font-weight: 700; font-size: 1.35rem; color: var(--paper); margin-top: 0.8rem; transform: translateZ(20px); }
  .hint { font-family: var(--font-type); font-size: 0.85rem; color: color-mix(in srgb, var(--paper) 65%, transparent); transform: translateZ(10px); }
  .glare {
    position: absolute; inset: 0 0 auto 0; aspect-ratio: 3 / 4; pointer-events: none; border-radius: 6px;
    background: radial-gradient(circle at var(--gx) var(--gy), rgb(255 255 255 / 0.22), transparent 45%);
    mix-blend-mode: soft-light; opacity: 0; transition: opacity 250ms;
  }
  .card:hover .glare { opacity: 1; }

  /* Newspaper card */
  .sheet {
    container-type: inline-size;
    aspect-ratio: 3 / 4;
    padding: 1.1rem 1rem;
    border-radius: 3px;
    box-shadow: var(--shadow);
    rotate: -1.5deg;
    position: relative;
    overflow: hidden;
  }
  .mast { font-family: var(--font-mast); font-size: 8.2cqi; text-align: center; line-height: 1.1; }
  .dateline {
    display: flex; justify-content: space-between; font-family: var(--font-type); font-size: max(0.6rem, 2.9cqi);
    border-block: 1px solid var(--ink); margin: 0.4rem 0 0.6rem; padding: 0.15rem 0; color: var(--ink-soft);
  }
  .headline { font-family: var(--font-head); font-weight: 900; font-size: 5.8cqi; line-height: 1.12; margin: 0 0 0.7rem; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .cols { display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.6rem; }
  .col { display: grid; gap: 0.32rem; align-content: start; }
  .col span { height: 0.3rem; background: color-mix(in srgb, var(--ink) 28%, transparent); border-radius: 1px; }
  .photo { height: 3.2rem; margin-top: 0.2rem; background: repeating-radial-gradient(circle at 30% 40%, color-mix(in srgb, var(--ink) 45%, transparent) 0 1px, transparent 1px 3px), color-mix(in srgb, var(--ink) 12%, transparent); }
  .fold { position: absolute; left: 0; right: 0; top: 52%; height: 22%; background: linear-gradient(rgb(0 0 0 / 0.08), transparent 30%, rgb(255 255 255 / 0.08) 60%, transparent); pointer-events: none; }

  /* Books card */
  .cover {
    container-type: inline-size;
    display: grid;
    aspect-ratio: 3 / 4;
    border-radius: 4px 10px 10px 4px;
    padding: 1.2rem;
    position: relative;
    rotate: 1.2deg;
    background:
      repeating-linear-gradient(45deg, rgb(255 255 255 / 0.035) 0 1px, transparent 1px 3px),
      repeating-linear-gradient(-45deg, rgb(0 0 0 / 0.08) 0 1px, transparent 1px 3px),
      linear-gradient(90deg, #1f3e35 0 6%, #2f5d50 9%, #2a5246 100%);
    box-shadow: var(--shadow), inset 10px 0 12px -6px rgb(0 0 0 / 0.5), inset -2px 0 0 rgb(255 255 255 / 0.08);
  }
  .frame {
    min-height: 0;
    overflow: hidden;
    border: 2px solid color-mix(in srgb, var(--gold) 80%, transparent);
    outline: 1px solid color-mix(in srgb, var(--gold) 50%, transparent);
    outline-offset: -8px;
    border-radius: 3px;
    display: grid; align-content: center; justify-items: center; gap: 3.5cqi;
    text-align: center; color: var(--gold); padding: 4.5cqi;
  }
  .crest { font-size: 9.8cqi; text-shadow: 0 1px 0 rgb(0 0 0 / 0.5); }
  .ctitle { font-family: var(--font-fell); font-size: 8.8cqi; letter-spacing: 0.06em; text-shadow: 0 1px 0 rgb(0 0 0 / 0.6), 0 -1px 0 rgb(255 230 160 / 0.2); }
  .rule { width: 50%; height: 1px; background: var(--gold); opacity: 0.7; }
  .reading { font-family: var(--font-fell); font-size: max(0.8rem, 4.4cqi); color: color-mix(in srgb, var(--gold) 85%, white); }
  .bar { width: 70%; height: 4px; background: rgb(0 0 0 / 0.3); border-radius: 2px; overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--gold); }
  .ribbon { position: absolute; right: 18%; bottom: -14px; width: 14px; height: 40px; background: var(--accent); clip-path: polygon(0 0, 100% 0, 100% 100%, 50% 78%, 0 100%); box-shadow: 0 2px 4px rgb(0 0 0 / 0.4); }

  .foot { text-align: center; font-family: var(--font-type); font-size: 0.78rem; color: color-mix(in srgb, var(--paper) 45%, transparent); }

  @keyframes rise { from { opacity: 0; transform: translateY(12px); } }
  @keyframes drop {
    from { opacity: 0; transform: translateY(-40px) rotateX(25deg) rotateZ(-4deg); }
  }
</style>
