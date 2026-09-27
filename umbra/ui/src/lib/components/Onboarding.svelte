<script lang="ts">
	import { onMount } from 'svelte';
	import gsap from 'gsap';
	import { ui, applyState, tutorialOpen } from '$lib/stores';
	import { cmd } from '$lib/tauri';

	$: firstRun = $ui.first_run === true;

	async function finish(openTutorial: boolean) {
		await cmd('set_flag', { key: 'onboarded', value: true });
		applyState({ first_run: false });
		if (openTutorial) tutorialOpen.set(true);
	}

	onMount(() => {
		gsap.from('.ob-card', { y: 20, opacity: 0, scale: 0.97, duration: 0.4, ease: 'power3.out' });
	});
</script>

{#if firstRun}
	<div class="overlay u-scrim" role="dialog" aria-label="Welcome to Umbra">
		<div class="ob-card glass-strong">
			<img class="logo-img" src="/umbra-logo.png" alt="Umbra Logo" />
			<h1>Welcome to Umbra</h1>
			<p class="desc">A fast, private browser tuned entirely from one TOML file. Themes, layouts, and keybindings all hot-reload live.</p>

			<button class="primary" on:click={() => void finish(true)}>Try the tutorial</button>
			<button class="ghost" on:click={() => void finish(false)}>Skip the tutorial, I just want to browse.</button>
			<span class="caption dim small">You can always start it later from Settings › About &amp; Help.</span>
		</div>
	</div>
{/if}

<style>
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 1050;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 20px;
	}
	.ob-card {
		width: 380px;
		max-width: 92vw;
		border-radius: var(--umbra-radius);
		padding: 30px 30px 24px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 14px;
		text-align: center;
	}
	.logo-img {
		width: 64px;
		height: 64px;
		object-fit: contain;
		margin-bottom: 2px;
		filter: drop-shadow(0 6px 16px color-mix(in srgb, var(--umbra-color-accent) 40%, transparent));
	}
	h1 {
		margin: 0;
		font-size: 22px;
		font-weight: 500;
		letter-spacing: 0.5px;
	}
	.desc {
		margin: 0;
		font-size: 13px;
		line-height: 1.55;
		color: var(--umbra-color-fg-dim);
		max-width: 300px;
	}
	.primary {
		all: unset;
		cursor: default;
		width: 100%;
		box-sizing: border-box;
		height: 34px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		font-size: 13px;
		border-radius: var(--umbra-radius-small);
		background: var(--umbra-color-accent);
		color: var(--umbra-color-accent-fg);
		transition: filter var(--umbra-duration) var(--umbra-ease);
	}
	.primary:hover {
		filter: brightness(1.12);
	}
	.ghost {
		all: unset;
		cursor: default;
		width: 100%;
		box-sizing: border-box;
		height: 32px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		font-size: 12.5px;
		border-radius: var(--umbra-radius-small);
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 90%, transparent);
		color: var(--umbra-color-fg-dim);
		transition:
			color var(--umbra-duration) var(--umbra-ease),
			border-color var(--umbra-duration) var(--umbra-ease);
	}
	.ghost:hover {
		color: var(--umbra-color-fg);
		border-color: var(--umbra-color-accent);
	}
	.caption {
		margin-top: 4px;
	}
</style>
