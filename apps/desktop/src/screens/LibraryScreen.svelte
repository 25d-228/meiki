<script lang="ts">
  import Button from "../lib/components/Button.svelte";
  import Field from "../lib/components/Field.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import TextInput from "../lib/components/TextInput.svelte";
  import Toolbar from "../lib/components/Toolbar.svelte";

  type Props = {
    onNavigate: (screen: string) => void;
  };

  let { onNavigate }: Props = $props();
  let query = $state("");
</script>

<section class="screen" aria-labelledby="library-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Your collection</span>
      <h1 id="library-title" class="screen-title">Library</h1>
      <p class="screen-description">
        Search source notes, clozes, decks, and tags without changing study
        state.
      </p>
    </div>
  </header>

  <Toolbar label="Library tools">
    <div class="toolbar-grow">
      <Field id="library-search" label="Search">
        <TextInput
          id="library-search"
          type="search"
          bind:value={query}
          placeholder="Search any script"
          aria-label="Search library"
          dir="auto"
        />
      </Field>
    </div>
    <Button variant="secondary">Filters</Button>
  </Toolbar>

  <SurfaceCard>
    <div class="empty-state">
      <span class="empty-mark" aria-hidden="true">＋</span>
      <h2>{query ? "No matching notes" : "Your library is ready"}</h2>
      <p>
        {query
          ? `Nothing matches “${query}”. Try another script or clear the search.`
          : "Create a source note and turn one or more semantic spans into clozes."}
      </p>
      <Button
        variant="primary"
        data-primary-action
        onclick={() => onNavigate("editor")}>Add a source note</Button
      >
    </div>
  </SurfaceCard>
</section>

<style>
  section {
    display: grid;
    gap: var(--space-5);
  }

  .screen-header {
    margin-bottom: 0;
  }

  .empty-state {
    display: grid;
    justify-items: center;
    min-height: 20rem;
    text-align: center;
    place-content: center;
  }

  .empty-mark {
    display: inline-grid;
    width: 3rem;
    height: 3rem;
    border: var(--border-width) solid var(--color-accent-border);
    border-radius: 50%;
    color: var(--color-accent);
    background: var(--color-accent-soft);
    font-size: var(--text-xl);
    place-items: center;
  }

  h2 {
    margin: var(--space-4) 0 var(--space-2);
    font-family: var(--font-display);
    font-size: var(--text-xl);
  }

  p {
    max-width: 34rem;
    margin: 0 0 var(--space-6);
    color: var(--color-text-muted);
    line-height: 1.6;
  }
</style>
