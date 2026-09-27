<script lang="ts">
	import gsap from 'gsap';
	import { ui, applyState, pushToast } from '$lib/stores';
	import { cmd } from '$lib/tauri';
	import type { Bookmark } from '$lib/types';

	export let vertical = false;

	function enter(node: HTMLElement) {
		gsap.fromTo(node, { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.2, ease: 'power2.out' });
		return { duration: 200 };
	}
	function leave(node: HTMLElement) {
		gsap.to(node, { opacity: 0, duration: 0.15, ease: 'power1.in' });
		return { duration: 160 };
	}
</script>

{#if vertical}
	<div class="tabstrip vertical" role="tablist" aria-label="Tabs" data-testid="vtabstrip">
		{#each $ui.tabs as t (t.id)}
			<div
				class="tab"
				class:active={t.active}
				role="tab"
				tabindex="-1"
				aria-selected={t.active}
				title={t.url}
				on:click={() => void cmd('activate_tab', { id: t.id })}
				on:keydown={(e) => e.key === 'Enter' && void cmd('activate_tab', { id: t.id })}
				in:enter
				out:leave
			>
				<span class="t-fav" aria-hidden="true">{(t.title || t.url || '?').trim().charAt(0).toUpperCase()}</span>
				<span class="t-title">{t.title || t.url}</span>
				<button class="t-close" aria-label={`Close tab ${t.title || t.url}`} title="Close tab" on:click|stopPropagation={() => void cmd('close_tab', { id: t.id })}>×</button>
			</div>
		{/each}
		<button class="newtab icon-btn" aria-label="New tab" title="New tab (Ctrl+T)" on:click={() => void cmd('run_action', { action: 'new_tab' })}>+</button>
	</div>
{:else}
	<div class="tabstrip horizontal" role="tablist" aria-label="Tabs" data-testid="htabstrip">
		{#each $ui.tabs as t (t.id)}
			<div
				class="tab"
				class:active={t.active}
				class:floating={$ui.layout === 'floating'}
				class:square={$ui.layout === 'tiling'}
				role="tab"
				tabindex="-1"
				aria-selected={t.active}
				title={t.url}
				on:click={() => void cmd('activate_tab', { id: t.id })}
				on:keydown={(e) => e.key === 'Enter' && void cmd('activate_tab', { id: t.id })}
				in:enter
				out:leave
			>
				<span class="t-title">{t.title || t.url}</span>
				<button class="t-close" aria-label={`Close tab ${t.title || t.url}`} title="Close tab" on:click|stopPropagation={() => void cmd('close_tab', { id: t.id })}>×</button>
			</div>
		{/each}
	</div>
{/if}

<style>
	.tabstrip {
		display: flex;
		gap: 4px;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		flex-direction: row;
	}
	.tabstrip.horizontal {
		flex: 1;
		justify-content: flex-start;
		align-items: center;
	}
	.tabstrip.vertical {
		flex-direction: column;
		align-items: stretch;
		overflow-y: auto;
	}

	.tab {
		display: flex;
		align-items: center;
		gap: 8px;
		max-width: 190px;
		min-width: 90px;
		height: 32px;
		padding: 0 12px;
		border-radius: var(--umbra-radius-small);
		background: transparent;
		color: var(--umbra-color-fg-dim);
		cursor: default;
		user-select: none;
		white-space: nowrap;
		flex-shrink: 1;
		transition:
			background var(--umbra-duration) var(--umbra-ease),
			color var(--umbra-duration) var(--umbra-ease),
			transform var(--umbra-duration) var(--umbra-ease);
	}
	.tab:hover {
		background: color-mix(in srgb, var(--umbra-color-fg) 8%, transparent);
	}
	.tab.active {
		background: color-mix(in srgb, var(--umbra-color-surface) 92%, transparent);
		color: var(--umbra-color-fg);
		box-shadow:
			inset 0 0 0 1px var(--umbra-color-border),
			0 2px 8px rgba(0, 0, 0, 0.25);
	}
	.tab .t-title {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		font-size: 12.5px;
	}
	.tab .t-close {
		all: unset;
		cursor: default;
		width: 16px;
		height: 16px;
		line-height: 14px;
		text-align: center;
		border-radius: 50%;
		font-size: 11px;
		opacity: 0;
		flex-shrink: 0;
		transition:
			opacity var(--umbra-duration) var(--umbra-ease),
			background var(--umbra-duration) var(--umbra-ease);
	}
	.tab:hover .t-close,
	.tab.active .t-close {
		opacity: 0.7;
	}
	.tab .t-close:hover {
		background: color-mix(in srgb, var(--umbra-color-danger) 35%, transparent);
		opacity: 1;
	}

	/* layout variants */
	.tab.floating {
		transform: rotate(-1deg);
		box-shadow: 0 4px 10px rgba(0, 0, 0, 0.3);
		border: 1px solid var(--umbra-color-border);
		background: color-mix(in srgb, var(--umbra-color-surface) 70%, transparent);
	}
	.tab.floating.active {
		transform: rotate(0deg) translateY(-2px);
	}
	.tab.square {
		border-radius: 4px;
	}

	.vertical .tab {
		max-width: none;
		height: 36px;
	}
	.vertical .t-fav {
		width: 20px;
		height: 20px;
		flex-shrink: 0;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: 6px;
		font-size: 11px;
		font-weight: 700;
		background: color-mix(in srgb, var(--umbra-color-accent) 24%, transparent);
		color: var(--umbra-color-accent);
	}
	.newtab {
		margin-top: 2px;
	}
</style>
