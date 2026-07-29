<script lang="ts">
  type InlineToken = {
    kind: "text" | "strong" | "emphasis" | "code";
    value: string;
  };

  type Block =
    { kind: "paragraph"; value: string } | { kind: "list"; items: string[] };

  type Props = {
    value: string;
  };

  let { value }: Props = $props();

  function blocks(markdown: string): Block[] {
    const result: Block[] = [];
    for (const line of markdown.split("\n")) {
      if (!line.trim()) continue;
      if (line.startsWith("- ")) {
        const previous = result.at(-1);
        if (previous?.kind === "list") {
          previous.items.push(line.slice(2));
        } else {
          result.push({ kind: "list", items: [line.slice(2)] });
        }
      } else {
        result.push({ kind: "paragraph", value: line });
      }
    }
    return result;
  }

  function inline(markdown: string): InlineToken[] {
    const tokens: InlineToken[] = [];
    const pattern = /(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`)/g;
    let preceding = 0;
    for (const match of markdown.matchAll(pattern)) {
      const index = match.index;
      if (index > preceding) {
        tokens.push({
          kind: "text",
          value: markdown.slice(preceding, index),
        });
      }
      const token = match[0];
      if (token.startsWith("**")) {
        tokens.push({ kind: "strong", value: token.slice(2, -2) });
      } else if (token.startsWith("*")) {
        tokens.push({ kind: "emphasis", value: token.slice(1, -1) });
      } else {
        tokens.push({ kind: "code", value: token.slice(1, -1) });
      }
      preceding = index + token.length;
    }
    if (preceding < markdown.length) {
      tokens.push({ kind: "text", value: markdown.slice(preceding) });
    }
    return tokens;
  }
</script>

<div class="limited-markdown">
  {#each blocks(value) as block, blockIndex (blockIndex)}
    {#if block.kind === "paragraph"}
      <p>
        {#each inline(block.value) as token, tokenIndex (tokenIndex)}
          {#if token.kind === "strong"}
            <strong>{token.value}</strong>
          {:else if token.kind === "emphasis"}
            <em>{token.value}</em>
          {:else if token.kind === "code"}
            <code>{token.value}</code>
          {:else}
            {token.value}
          {/if}
        {/each}
      </p>
    {:else}
      <ul>
        {#each block.items as item, itemIndex (itemIndex)}
          <li>
            {#each inline(item) as token, tokenIndex (tokenIndex)}
              {#if token.kind === "strong"}
                <strong>{token.value}</strong>
              {:else if token.kind === "emphasis"}
                <em>{token.value}</em>
              {:else if token.kind === "code"}
                <code>{token.value}</code>
              {:else}
                {token.value}
              {/if}
            {/each}
          </li>
        {/each}
      </ul>
    {/if}
  {/each}
</div>

<style>
  .limited-markdown {
    padding: var(--space-4);
    overflow-wrap: anywhere;
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    font-size: var(--text-sm);
    line-height: 1.6;
  }

  p,
  ul {
    margin: 0;
  }

  p + p,
  p + ul,
  ul + p,
  ul + ul {
    margin-top: var(--space-2);
  }

  code {
    padding: 0.1em 0.3em;
    border-radius: var(--radius-xs);
    background: var(--color-surface-muted);
  }
</style>
