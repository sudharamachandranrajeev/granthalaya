<script lang="ts">
  import { onMount } from 'svelte'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { api, errText, type EditionStory, type Opened, type Report } from '../lib/api'
  import BiasMeter from './BiasMeter.svelte'
  import Stars from './Stars.svelte'
  import { news } from './news.svelte'

  let { item, onclose }: { item: EditionStory; onclose: () => void } = $props()
  const s = $derived(item.story)
  let opened = $state<Opened | null>(null)
  let failed = $state<string | null>(null)
  const report: Report = $derived(opened?.report ?? item.report)
  const fb = $derived(news.feedback[s.id] ?? [null, null])

  onMount(() => {
    api.storyOpen(item.story.id).then((o) => (opened = o)).catch((e) => (failed = errText(e)))
    const key = (e: KeyboardEvent) => e.key === 'Escape' && onclose()
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  })

  const open = (url: string | null) => url && openUrl(url).catch(() => {})
  const kinds: Record<string, string> = { person: 'Person', party: 'Party', company: 'Company', organisation: 'Organisation', government: 'Government' }
</script>

<div class="scrim" onclick={onclose} aria-hidden="true"></div>
<div class="article paper" role="dialog" aria-modal="true" aria-label={s.title}>
  <div class="bar">
    <button onclick={onclose}>← Back to the paper</button>
    <span class="spacer"></span>
    <button onclick={() => open(s.url)}>Read the original ↗</button>
  </div>

  <div class="body">
    <p class="kicker">{s.outlet_name} · {report.outlet.country}{s.topics.length ? ' · ' + s.topics.map((t) => news.topicLabel(t)).join(', ') : ''}</p>
    <h1 style="view-transition-name: story-{s.id}">{s.title}</h1>
    <BiasMeter {report} />

    <div class="cols">
      <section class="text" aria-live="polite">
        {#if opened && opened.paragraphs.length}
          {#each opened.paragraphs as p, i}<p class:first={i === 0}>{p}</p>{/each}
        {:else if opened?.note || failed}
          <p class="summary">{s.summary}</p>
          <p class="note">{opened?.note ?? failed}</p>
        {:else}
          <p class="summary">{s.summary}</p>
          <p class="setting">Setting the type…</p>
        {/if}
      </section>

      <aside class="evidence" aria-label="The evidence behind the score">
        <h2>The evidence</h2>
        <p class="basis">Checked against: <strong>{report.basis}</strong>. Every point comes from a rule shown here; nothing is a hidden label.</p>
        {#each report.components as c}
          <div class="comp">
            <div class="ch"><span>{c.label}</span><span>{c.points}/{c.max}</span></div>
            <div class="cbar"><span style="width:{(c.points / c.max) * 100}%"></span></div>
            <p>{c.detail}</p>
          </div>
        {/each}

        {#if report.influences.length}
          <h3>Possible influences</h3>
          <ul class="infl">{#each report.influences as i}<li><strong>{i.who}</strong>: {i.how}</li>{/each}</ul>
        {/if}

        <h3>Who is in this story</h3>
        {#if report.actors.length}
          <table>
            <thead><tr><th>Name</th><th>Kind</th><th>Named</th><th>Quoted</th></tr></thead>
            <tbody>
              {#each report.actors as a}<tr><td>{a.name}</td><td>{kinds[a.kind] ?? a.kind}</td><td>{a.mentions}</td><td>{a.quotes}</td></tr>{/each}
            </tbody>
          </table>
          <p class="small">Found by text rules, so a name can be missed or mis-sorted.</p>
        {:else}
          <p class="small">No people, parties, companies or governments detected.</p>
        {/if}

        <h3>Who is behind {report.outlet.name}</h3>
        <p class="small">{report.outlet.funding_text}</p>
        {#if report.outlet.owners.length}<p class="small">Owned by: {#each report.outlet.owners as o, i}{i ? ', ' : ''}<button class="link" onclick={() => open(o.url)}>{o.label}</button>{/each}</p>{/if}
        {#if report.outlet.funders.length}<p class="small">Funded by: {#each report.outlet.funders as o, i}{i ? ', ' : ''}<button class="link" onclick={() => open(o.url)}>{o.label}</button>{/each}</p>{/if}
        {#if report.outlet.wikidata}<p class="small"><button class="link" onclick={() => open(report.outlet.wikidata)}>Public record on Wikidata ↗</button></p>{/if}

        {#if item.others.length}
          <h3>The same story elsewhere</h3>
          <ul class="others">{#each item.others as o}<li><button class="link" onclick={() => open(o.url)}><strong>{o.outlet_name}</strong>: {o.title}</button></li>{/each}</ul>
        {/if}
      </aside>
    </div>

    <footer class="controls">
      <div class="rate">
        <span>Your verdict</span>
        <button class="vote" aria-pressed={fb[0] === 1} onclick={() => news.vote(s.id, 1)}>▲ More like this</button>
        <button class="vote" aria-pressed={fb[0] === -1} onclick={() => news.vote(s.id, -1)}>▼ Less like this</button>
        <Stars value={fb[1]} onrate={(v) => news.rate(s.id, v)} />
      </div>
      <div class="prefs">
        {#each s.topics as t}
          <button class="chip" aria-pressed={news.prefs.followed_topics.includes(t)} onclick={() => news.follow(t)}>Follow {news.topicLabel(t)}</button>
          <button class="chip" aria-pressed={news.prefs.muted_topics.includes(t)} onclick={() => news.mute(t)}>Mute {news.topicLabel(t)}</button>
        {/each}
        <button class="chip" aria-pressed={news.prefs.blocked_outlets.includes(s.outlet_id)} onclick={() => news.block(s.outlet_id)}>
          {news.prefs.blocked_outlets.includes(s.outlet_id) ? 'Blocked' : 'Block'} {s.outlet_name}
        </button>
      </div>
      <p class="small">Changes apply from the next edition.</p>
    </footer>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgb(0 0 0 / 0.5); z-index: 40; animation: fade 300ms both; }
  .article {
    position: fixed; z-index: 41; inset: max(env(safe-area-inset-top), 2vh) max(2vw, 0px) 0;
    overflow-y: auto; border-radius: 6px 6px 0 0; box-shadow: 0 -20px 60px rgb(0 0 0 / 0.5);
    animation: slide 520ms var(--ease-out) both;
  }
  .bar { position: sticky; top: 0; z-index: 2; display: flex; gap: 0.5rem; padding: 0.6rem 1rem; background: color-mix(in srgb, var(--paper) 92%, transparent); backdrop-filter: blur(4px); border-bottom: 1px solid var(--rule); font-family: var(--font-type); }
  .bar button { border: 1px solid var(--rule); background: transparent; border-radius: 6px; padding: 0.3em 0.8em; }
  .spacer { flex: 1; }
  .body { max-width: 1100px; margin: 0 auto; padding: 1.2rem clamp(1rem, 4vw, 2.5rem) 3rem; }
  .kicker { font-family: var(--font-type); font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-faint); margin: 0; }
  h1 { font-family: var(--font-head); font-weight: 900; font-size: clamp(1.7rem, 4vw, 2.8rem); line-height: 1.06; margin: 0.4rem 0 0.4rem; text-wrap: balance; }
  .cols { display: grid; grid-template-columns: minmax(0, 1.6fr) minmax(260px, 1fr); gap: 2rem; margin-top: 1.2rem; }
  @media (max-width: 820px) { .cols { grid-template-columns: 1fr; } }
  .text p { line-height: 1.6; font-size: 1.08rem; text-align: justify; hyphens: auto; margin: 0 0 0.9em; }
  .text p.first::first-letter { float: left; font-family: var(--font-head); font-weight: 900; font-size: 3.4em; line-height: 0.82; padding: 0.06em 0.08em 0 0; }
  .summary { font-style: italic; }
  .note, .setting { font-family: var(--font-type); font-size: 0.85rem; color: var(--ink-soft); border-left: 3px solid var(--rule); padding-left: 0.7rem; }
  .setting { animation: blink 1.2s infinite alternate; }
  .evidence { border-left: 1px solid var(--rule); padding-left: 1.4rem; }
  @media (max-width: 820px) { .evidence { border-left: 0; padding-left: 0; border-top: 2px solid var(--ink); padding-top: 1rem; } }
  .evidence h2 { font-family: var(--font-head); font-size: 1.4rem; margin: 0; }
  .evidence h3 { font-family: var(--font-head); font-size: 1.02rem; margin: 1.2rem 0 0.4rem; border-bottom: 1px solid var(--rule); padding-bottom: 0.2rem; }
  .basis, .small { font-family: var(--font-type); font-size: 0.75rem; color: var(--ink-soft); line-height: 1.45; margin: 0.3rem 0; }
  .comp { margin: 0.7rem 0; }
  .ch { display: flex; justify-content: space-between; font-family: var(--font-type); font-size: 0.8rem; }
  .cbar { height: 5px; background: color-mix(in srgb, var(--ink) 12%, transparent); margin: 0.2rem 0; }
  .cbar span { display: block; height: 100%; background: var(--accent-2); animation: grow 800ms var(--ease-out) both; transform-origin: left; }
  .comp p { font-size: 0.85rem; line-height: 1.4; margin: 0.15rem 0 0; color: var(--ink-soft); }
  .infl { padding-left: 1rem; font-size: 0.88rem; line-height: 1.45; color: var(--ink); }
  table { width: 100%; border-collapse: collapse; font-size: 0.82rem; }
  th { text-align: left; font-family: var(--font-type); font-weight: 400; color: var(--ink-faint); border-bottom: 1px solid var(--ink); }
  td, th { padding: 0.2rem 0.3rem 0.2rem 0; }
  td:nth-child(n + 3), th:nth-child(n + 3) { text-align: right; }
  tr + tr td { border-top: 1px dotted var(--rule); }
  .others { list-style: none; padding: 0; margin: 0; display: grid; gap: 0.4rem; font-size: 0.88rem; }
  .link { all: unset; cursor: pointer; text-decoration: underline; text-decoration-color: var(--rule); text-underline-offset: 2px; }
  .link:hover { text-decoration-color: var(--ink); }
  .controls { border-top: 3px double var(--ink); margin-top: 2rem; padding-top: 1rem; display: grid; gap: 0.7rem; }
  .rate { display: flex; align-items: center; flex-wrap: wrap; gap: 0.5rem; font-family: var(--font-type); font-size: 0.85rem; }
  .vote { border: 1px solid var(--rule); background: transparent; border-radius: 6px; padding: 0.3em 0.7em; font-family: var(--font-type); font-size: 0.8rem; }
  .vote[aria-pressed='true'] { background: var(--ink); color: var(--paper); }
  .prefs { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  @keyframes slide { from { transform: translateY(30px); opacity: 0; } }
  @keyframes fade { from { opacity: 0; } }
  @keyframes grow { from { transform: scaleX(0); } }
  @keyframes blink { to { opacity: 0.4; } }
</style>
