<script lang="ts" module>
  export interface PaletteCommand {
    id: string;
    label: string;
    group: string;
    hint?: string;
    keys?: string;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    icon?: any;
    run: () => void;
  }
</script>

<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { Search, CornerDownLeft } from 'lucide-svelte';

  interface Props {
    open: boolean;
    commands: PaletteCommand[];
    onClose: () => void;
  }
  let { open, commands, onClose }: Props = $props();

  let query = $state('');
  let cursor = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((c) => {
      const hay = `${c.label} ${c.group} ${c.hint ?? ''}`.toLowerCase();
      return q.split(/\s+/).every((part) => hay.includes(part));
    });
  });

  const groups = $derived.by(() => {
    const map = new Map<string, Array<{ cmd: PaletteCommand; idx: number }>>();
    filtered.forEach((cmd, idx) => {
      if (!map.has(cmd.group)) map.set(cmd.group, []);
      map.get(cmd.group)!.push({ cmd, idx });
    });
    return [...map.entries()];
  });

  $effect(() => {
    if (open) {
      query = '';
      cursor = 0;
      queueMicrotask(() => inputEl?.focus());
    }
  });

  $effect(() => {
    // clamp cursor when the result set changes
    if (cursor >= filtered.length) cursor = Math.max(0, filtered.length - 1);
  });

  function exec(cmd: PaletteCommand | undefined) {
    if (!cmd) return;
    onClose();
    queueMicrotask(() => cmd.run());
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      cursor = (cursor + 1) % Math.max(1, filtered.length);
      scrollIntoView();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      cursor = (cursor - 1 + filtered.length) % Math.max(1, filtered.length);
      scrollIntoView();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      exec(filtered[cursor]);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  function scrollIntoView() {
    queueMicrotask(() => {
      listEl?.querySelector<HTMLElement>(`[data-idx="${cursor}"]`)?.scrollIntoView({ block: 'nearest' });
    });
  }
</script>

{#if open}
  <div class="backdrop" transition:fade={{ duration: 140 }}>
    <button type="button" class="dismiss" aria-label="Close command palette" onclick={onClose}></button>
    <div
      class="palette glass"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      transition:fly={{ y: -10, duration: 220 }}
    >
      <div class="search">
        <Search size={16} strokeWidth={1.8} />
        <input
          bind:this={inputEl}
          bind:value={query}
          onkeydown={onKey}
          placeholder="Type a command or search…"
          spellcheck="false"
          autocomplete="off"
        />
        <span class="kbd">esc</span>
      </div>

      <div class="list" bind:this={listEl}>
        {#if filtered.length === 0}
          <div class="none">No matching commands</div>
        {:else}
          {#each groups as [group, items]}
            <div class="group">
              <div class="eyebrow g-title">{group}</div>
              {#each items as { cmd, idx } (cmd.id)}
                <button
                  type="button"
                  class="item"
                  class:active={idx === cursor}
                  data-idx={idx}
                  onmouseenter={() => (cursor = idx)}
                  onclick={() => exec(cmd)}
                >
                  <span class="ico">
                    {#if cmd.icon}<cmd.icon size={15} strokeWidth={1.7} />{/if}
                  </span>
                  <span class="label">{cmd.label}</span>
                  {#if cmd.hint}<span class="hint">{cmd.hint}</span>{/if}
                  {#if cmd.keys}<span class="kbd">{cmd.keys}</span>{/if}
                  <span class="enter"><CornerDownLeft size={12} /></span>
                </button>
              {/each}
            </div>
          {/each}
        {/if}
      </div>

      <div class="foot">
        <span><span class="kbd">↑</span><span class="kbd">↓</span> navigate</span>
        <span><span class="kbd">↵</span> run</span>
        <span class="brand mono">AegisMesh / cmd</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 95;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 14vh;
    background: var(--backdrop-bg);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
  }
  .dismiss {
    position: fixed;
    inset: 0;
    border: none;
    background: transparent;
  }
  .palette {
    position: relative;
    z-index: 1;
    width: min(620px, 92vw);
    border-radius: 18px;
    overflow: hidden;
    box-shadow: var(--shadow-pop), 0 0 80px -20px rgba(var(--accent-rgb), 0.18);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 18px;
    height: 58px;
    border-bottom: 1px solid var(--line);
    color: var(--fg-3);
  }
  .search input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--fg);
    font-family: inherit;
    font-size: 15.5px;
    letter-spacing: -0.01em;
  }
  .search input::placeholder {
    color: var(--fg-4);
  }
  .list {
    max-height: min(420px, 52vh);
    overflow-y: auto;
    padding: 8px;
  }
  .group + .group {
    margin-top: 6px;
  }
  .g-title {
    padding: 8px 10px 6px;
    font-size: 9.5px;
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    height: 40px;
    border-radius: 10px;
    border: none;
    background: transparent;
    color: var(--fg-2);
    font-size: 13.5px;
    cursor: pointer;
    text-align: left;
  }
  .item.active {
    background: var(--s-3);
    color: var(--fg);
  }
  .ico {
    width: 26px;
    height: 26px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: var(--s-2);
    border: 1px solid var(--line);
    color: var(--fg-2);
  }
  .item.active .ico {
    color: var(--accent);
    border-color: rgba(var(--accent-rgb), 0.3);
  }
  .label {
    flex: 1;
  }
  .hint {
    font-size: 12px;
    color: var(--fg-3);
  }
  .enter {
    display: inline-flex;
    color: var(--fg-3);
    opacity: 0;
  }
  .item.active .enter {
    opacity: 1;
  }
  .none {
    padding: 32px;
    text-align: center;
    color: var(--fg-3);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
    border-top: 1px solid var(--line);
    font-size: 11px;
    color: var(--fg-3);
  }
  .foot > span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .brand {
    margin-left: auto;
    color: var(--fg-4);
  }
</style>
