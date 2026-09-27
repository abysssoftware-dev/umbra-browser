<script lang="ts">
	import { ui, bookmarkDialog } from '$lib/stores';
	import { cmd } from '$lib/tauri';
	import type { Bookmark } from '$lib/types';

	function addBookmark() {
		const t = $ui.tabs.find((x) => x.active);
		if (!t || t.url === 'umbra://newtab') return;
		bookmarkDialog.set({ id: null, title: t.title, url: t.url });
	}

	function editBookmark(id: number) {
		const b = ($ui.bookmarks ?? []).find((x) => x.id === id);
		if (!b) return;
		bookmarkDialog.set({ id: b.id, title: b.title, url: b.url });
	}

	async function removeBookmark(id: number) {
		const res = await cmd<Bookmark[]>('remove_bookmark', { id });
		if (res) ui.update((s) => ({ ...s, bookmarks: res }));
	}
</script>

<div class="bookmarks-bar" role="toolbar" aria-label="Bookmarks">
	{#if ($ui.bookmarks ?? []).length > 0}
		{#each $ui.bookmarks ?? [] as b (b.id)}
			<button
				class="bm"
				title={b.url}
				on:click={() => void cmd('navigate', { url: b.url })}
				on:contextmenu|preventDefault={() => editBookmark(b.id)}
			>
				<span class="bm-title">{b.title || b.url}</span>
				<span
					class="bm-edit"
					role="button"
					tabindex="-1"
					aria-label={`Edit bookmark ${b.title || b.url}`}
					title="Edit bookmark"
					on:click|stopPropagation={() => editBookmark(b.id)}
					on:keydown|stopPropagation={(e) => e.key === 'Enter' && editBookmark(b.id)}
				>✎</span>
				<span
					class="bm-x"
					role="button"
					tabindex="-1"
					aria-label={`Remove bookmark ${b.title || b.url}`}
					on:click|stopPropagation={() => void removeBookmark(b.id)}
					on:keydown|stopPropagation={(e) => e.key === 'Enter' && void removeBookmark(b.id)}
				>×</span>
			</button>
		{/each}
	{/if}
	<button class="icon-btn add" title="Bookmark current page" aria-label="Bookmark current page" on:click={addBookmark}>+</button>
</div>

<style>
	.bookmarks-bar {
		display: flex;
		align-items: center;
		gap: 10px;
		min-height: 34px;
		padding: 0 calc(var(--umbra-spacing) * 2);
		font-size: 12.5px;
	}
	.hint {
		color: var(--umbra-color-fg-dim);
	}
	.bm {
		all: unset;
		cursor: default;
		display: inline-flex;
		align-items: center;
		gap: 6px;
		max-width: 180px;
		height: 24px;
		padding: 0 8px;
		border-radius: var(--umbra-radius-small);
		color: var(--umbra-color-fg);
		transition: background var(--umbra-duration) var(--umbra-ease);
	}
	.bm:hover {
		background: color-mix(in srgb, var(--umbra-color-fg) 10%, transparent);
	}
	.bm-title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.bm-edit,
	.bm-x {
		font-size: 11px;
		color: var(--umbra-color-fg-dim);
		border-radius: 50%;
		width: 14px;
		height: 14px;
		line-height: 13px;
		text-align: center;
		opacity: 0.6;
		flex-shrink: 0;
	}
	.bm-edit:hover {
		background: color-mix(in srgb, var(--umbra-color-accent) 35%, transparent);
		color: var(--umbra-color-fg);
		opacity: 1;
	}
	.bm-x:hover {
		background: color-mix(in srgb, var(--umbra-color-danger) 35%, transparent);
		color: var(--umbra-color-danger);
		opacity: 1;
	}
	.add {
		width: 22px;
		height: 22px;
		font-size: 13px;
	}
</style>
