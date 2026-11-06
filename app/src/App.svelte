<script lang="ts">
  import { nav } from './lib/nav.svelte'
  import Home from './home/Home.svelte'
  import Paper from './news/Paper.svelte'
  import Library from './books/Library.svelte'
  import Reader from './books/Reader.svelte'
  import Catalogues from './books/Catalogues.svelte'
  import Archive from './news/Archive.svelte'
</script>

{#if nav.current.name === 'home'}
  <Home />
{:else if nav.current.name === 'paper'}
  {#key nav.current.date}
    <Paper date={nav.current.date} story={nav.current.story} />
  {/key}
{:else if nav.current.name === 'archive'}
  <Archive />
{:else if nav.current.name === 'library'}
  <Library />
{:else if nav.current.name === 'catalogues'}
  <Catalogues />
{:else if nav.current.name === 'reader'}
  {#key nav.current.id}
    <Reader id={nav.current.id} />
  {/key}
{/if}
