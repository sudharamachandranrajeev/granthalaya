<script lang="ts">
  import { nav } from './lib/nav.svelte'
  import Home from './home/Home.svelte'
  import Paper from './news/Paper.svelte'
  import Library from './books/Library.svelte'
  import Reader from './books/Reader.svelte'
</script>

{#if nav.current.name === 'home'}
  <Home />
{:else if nav.current.name === 'paper'}
  <Paper />
{:else if nav.current.name === 'library'}
  <Library />
{:else if nav.current.name === 'reader'}
  {#key nav.current.id}
    <Reader id={nav.current.id} />
  {/key}
{/if}
