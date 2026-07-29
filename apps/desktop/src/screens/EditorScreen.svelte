<script lang="ts">
  import Button from "../lib/components/Button.svelte";
  import Dialog from "../lib/components/Dialog.svelte";
  import Feedback from "../lib/components/Feedback.svelte";
  import Field from "../lib/components/Field.svelte";
  import MediaFrame from "../lib/components/MediaFrame.svelte";
  import SurfaceCard from "../lib/components/SurfaceCard.svelte";
  import TextInput from "../lib/components/TextInput.svelte";

  let sourceText = "日曜日は図書館に行きます";
  let acceptedAnswer = "行きます";
  let previewOpen = false;
  let saved = false;
</script>

<section class="screen" aria-labelledby="editor-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Source-first authoring</span>
      <h1 id="editor-title" class="screen-title">Add / Edit</h1>
      <p class="screen-description">
        Keep original text intact and attach learning metadata to stable cloze
        segments.
      </p>
    </div>
    <div class="cluster">
      <Button variant="secondary" onclick={() => (previewOpen = true)}
        >Preview</Button
      >
      <Button
        variant="primary"
        data-primary-action
        onclick={() => (saved = true)}>Save draft</Button
      >
    </div>
  </header>

  {#if saved}
    <Feedback tone="success" title="Draft saved on this device" compact />
  {/if}

  <div class="editor-grid">
    <SurfaceCard>
      <div class="stack">
        <Field
          id="source-text"
          label="Source content"
          description="Select a semantic span, then make it a cloze."
        >
          <textarea
            id="source-text"
            bind:value={sourceText}
            class="content-text"
            dir="auto"
          ></textarea>
        </Field>
        <div class="selection">
          <span class="eyebrow">Active cloze</span>
          <bdi>{acceptedAnswer}</bdi>
          <Button variant="quiet" size="small">Make cloze</Button>
        </div>
        <Field
          id="accepted-answer"
          label="Accepted answer"
          description="Alternatives are explicit; original values are preserved."
        >
          <TextInput
            id="accepted-answer"
            bind:value={acceptedAnswer}
            dir="auto"
          />
        </Field>
      </div>
    </SurfaceCard>

    <aside class="stack" aria-label="Optional card media">
      <SurfaceCard padding="compact" tone="quiet">
        <div class="stack">
          <span class="eyebrow">Media</span>
          <MediaFrame kind="audio" label="Prompt audio" />
          <MediaFrame kind="image" label="Reveal image" />
        </div>
      </SurfaceCard>
      <SurfaceCard padding="compact" tone="quiet">
        <span class="eyebrow">Direction preview</span>
        <p class="content-preview content-text" dir="auto">{sourceText}</p>
      </SurfaceCard>
    </aside>
  </div>
</section>

<Dialog
  open={previewOpen}
  title="Card preview"
  description="The active cloze is hidden without exposing answer length."
  onClose={() => (previewOpen = false)}
>
  <p class="dialog-prompt content-text" dir="auto">
    {sourceText.replace(acceptedAnswer, "[…]")}
  </p>
  {#snippet actions()}
    <Button variant="primary" onclick={() => (previewOpen = false)}>Done</Button
    >
  {/snippet}
</Dialog>

<style>
  .editor-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(16rem, 0.65fr);
    gap: var(--space-5);
  }

  .selection {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--space-2) var(--space-4);
    align-items: center;
    padding: var(--space-4);
    border-radius: var(--radius-control);
    background: var(--color-accent-soft);
  }

  .selection .eyebrow {
    grid-column: 1 / -1;
    margin: 0;
    color: var(--color-accent);
  }

  .selection bdi {
    font-family: var(--font-content);
    font-size: var(--text-lg);
    font-weight: 700;
  }

  .content-preview,
  .dialog-prompt {
    margin: 0;
    line-height: 1.8;
  }

  .dialog-prompt {
    font-size: var(--text-xl);
    text-align: center;
  }

  @media (max-width: 900px) {
    .editor-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
