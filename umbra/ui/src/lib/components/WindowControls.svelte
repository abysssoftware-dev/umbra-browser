<script lang="ts">
	import { onMount } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { isTauri } from '$lib/tauri';
	import { winFullscreen, winMaximized } from '$lib/stores';
	import { refreshWindowState, toggleMaximize } from '$lib/window';

	let poll: ReturnType<typeof setInterval> | null = null;

	function minimize() {
		if (isTauri) void getCurrentWindow().minimize();
	}
	function close() {
		if (isTauri) void getCurrentWindow().close();
	}

	onMount(() => {
		if (!isTauri) return;
		void refreshWindowState();
		// onResized exists but polling also catches maximize transitions on all WMs.
		poll = setInterval(() => void refreshWindowState(), 800);
		return () => {
			if (poll) clearInterval(poll);
		};
	});
</script>

<div class="win-controls" role="group" aria-label="Window controls">
	<button class="wc-btn" title="Minimize" aria-label="Minimize" on:click={minimize}>
		<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
			<path d="M0 5h10" stroke="currentColor" stroke-width="1" />
		</svg>
	</button>
	<button class="wc-btn" title={$winMaximized ? 'Restore' : 'Maximize'} aria-label="Toggle maximize" on:click={() => void toggleMaximize()}>
		{#if $winMaximized}
			<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
				<path d="M2.5 2.5V0.5h7v7h-2M0.5 2.5h7v7h-7z" fill="none" stroke="currentColor" stroke-width="1" />
			</svg>
		{:else}
			<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
				<rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" />
			</svg>
		{/if}
	</button>
	<button class="wc-btn close" title={$winFullscreen ? 'Exit fullscreen' : 'Close'} aria-label="Close window" on:click={close}>
		<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
			<path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" />
		</svg>
	</button>
</div>

<style>
	.win-controls {
		display: flex;
		align-items: stretch;
		flex-shrink: 0;
		margin-left: 2px;
	}
	.wc-btn {
		all: unset;
		cursor: default;
		width: 36px;
		align-self: stretch;
		min-height: 30px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: 0;
		color: var(--umbra-color-fg-dim);
		transition:
			background var(--umbra-duration) var(--umbra-ease),
			color var(--umbra-duration) var(--umbra-ease);
	}
	.wc-btn:hover {
		background: color-mix(in srgb, var(--umbra-color-fg) 12%, transparent);
		color: var(--umbra-color-fg);
	}
	.wc-btn.close:hover {
		background: #e81123;
		color: #fff;
	}
</style>
