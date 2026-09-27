<script lang="ts">
	import { tick } from 'svelte';
	import gsap from 'gsap';
	import { ui, bookmarkDialog, applyState } from '$lib/stores';
	import { cmd } from '$lib/tauri';
	import { trapFocus } from '$lib/actions';
	import type { Bookmark } from '$lib/types';

	let nameEl = $state<HTMLInputElement | undefined>();
	let cardEl = $state<HTMLDivElement | undefined>();
	let title = $state('');

	const draft = $derived($bookmarkDialog);
	const existing = $derived(
		draft ? ($ui.bookmarks ?? []).find((b) => b.url === draft.url) ?? null : null
	);

	$effect(() => {
		if (!draft) return;
		title = draft.title;
		if (cardEl)
			gsap.fromTo(
				cardEl,
				{ opacity: 0, scale: 0.96, y: -8 },
				{ opacity: 1, scale: 1, y: 0, duration: 0.16, ease: 'power3.out' }
			);
		void tick().then(() => {
			nameEl?.focus();
			nameEl?.select();
		});
	});

	function close() {
		bookmarkDialog.set(null);
	}

	async function done() {
		if (!draft) return;
		const res = await cmd<Bookmark[]>('add_bookmark', { title: title.trim() || draft.url, url: draft.url });
		if (res) applyState({ bookmarks: res });
		close();
	}

	async function remove() {
		if (!existing) return;
		const res = await cmd<Bookmark[]>('remove_bookmark', { id: existing.id });
		if (res) applyState({ bookmarks: res });
		close();
	}
</script>

<svelte:window on:keydown={(e) => draft && e.key === 'Escape' && close()} />

{#if draft}
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="backdrop u-scrim" role="presentation" on:mousedown={(e) => e.target === e.currentTarget && close()}>
		<div class="dialog glass-strong" bind:this={cardEl} use:trapFocus role="dialog" aria-label="Edit bookmark">
			<header>
				<h2>{existing ? 'Edit bookmark' : 'Bookmark added'}</h2>
			</header>
			<div class="body">
				<label class="row">
					<span>Name</span>
					<input bind:this={nameEl} bind:value={title} aria-label="Bookmark name" spellcheck="false" />
				</label>
				<p class="url dim small" title={draft.url}>{draft.url}</p>
			</div>
			<footer>
				{#if existing}
					<button class="btn danger" on:click={() => void remove()}>Remove</button>
				{/if}
				<span class="spacer"></span>
				<button class="btn primary" on:click={() => void done()}>Done</button>
			</footer>
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		z-index: 1000;
		display: flex;
		align-items: flex-start;
		justify-content: center;
		padding-top: 14vh;
	}
	.dialog {
		width: 380px;
		max-width: 92vw;
		border-radius: var(--umbra-radius);
		display: flex;
		flex-direction: column;
	}
	header {
		padding: 14px 18px 4px;
	}
	h2 {
		margin: 0;
		font-size: 15px;
	}
	.body {
		padding: 6px 18px 12px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 13px;
	}
	.row > span {
		color: var(--umbra-color-fg-dim);
		flex-shrink: 0;
	}
	input {
		all: unset;
		flex: 1;
		height: 28px;
		padding: 0 8px;
		font-size: 12.5px;
		border-radius: var(--umbra-radius-small);
		background: color-mix(in srgb, var(--umbra-color-fg) 7%, transparent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 70%, transparent);
		min-width: 0;
	}
	input:focus {
		border-color: var(--umbra-color-accent);
	}
	.url {
		margin: 0;
		margin-left: 44px;
		font-family: var(--umbra-font-mono);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		user-select: text;
		cursor: default;
	}
	footer {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 18px 14px;
	}
	.spacer {
		flex: 1;
	}
	.btn {
		all: unset;
		cursor: default;
		height: 28px;
		padding: 0 16px;
		font-size: 12.5px;
		display: inline-flex;
		align-items: center;
		border-radius: var(--umbra-radius-small);
		border: 1px solid transparent;
	}
	.btn.primary {
		background: var(--umbra-color-accent);
		color: var(--umbra-color-accent-fg);
	}
	.btn.danger {
		border-color: color-mix(in srgb, var(--umbra-color-danger) 55%, transparent);
		color: var(--umbra-color-danger);
	}
	.btn.danger:hover {
		background: color-mix(in srgb, var(--umbra-color-danger) 18%, transparent);
	}
</style>
