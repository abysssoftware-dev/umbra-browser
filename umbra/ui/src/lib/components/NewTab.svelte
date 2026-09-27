<script lang="ts">
	import { onMount } from 'svelte';
	import gsap from 'gsap';
	import { activeTab, ui } from '$lib/stores';
	import { cmd } from '$lib/tauri';

	let clock = $state('');
	let date = $state('');
	let inputEl = $state<HTMLInputElement | undefined>();
	let shown = $state(false);

	function tick() {
		const now = new Date();
		clock = now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		date = now.toLocaleDateString([], { weekday: 'long', month: 'long', day: 'numeric' });
	}

	function submit(e: SubmitEvent) {
		e.preventDefault();
		const v = (inputEl?.value ?? '').trim();
		if (!v) return;
		void cmd('navigate', { url: v });
		inputEl!.value = '';
	}

	function open(url: string) {
		void cmd('navigate', { url });
	}

	const visible = $derived(!!$activeTab && $activeTab.url === 'umbra://newtab');
	const hint = $derived(
		$ui.layout === 'tiling' && $ui.tabs.length > 2 && $ui.flags.contextual_hints !== false
	);
	// Built-in page customization from [newtab] in umbra.toml
	const bg = $derived($ui.newtab?.background ?? '');
	const greeting = $derived(($ui.newtab?.greeting ?? '').trim());
	const showClock = $derived($ui.newtab?.show_clock !== false);
	// Quick links: first bookmarks (capped - the bar is a launcher, not a list)
	const quickLinks = $derived(($ui.bookmarks ?? []).slice(0, 8));

	function letter(url: string): string {
		try {
			return new URL(url).hostname.replace(/^www\./, '').charAt(0).toUpperCase();
		} catch {
			return url.charAt(0).toUpperCase();
		}
	}

	function hostOf(url: string): string {
		try {
			return new URL(url).hostname.replace(/^www\./, '');
		} catch {
			return url;
		}
	}

	onMount(() => {
		tick();
		const iv = setInterval(tick, 10_000);
		return () => clearInterval(iv);
	});

	$effect(() => {
		if (visible && !shown) {
			shown = true;
			gsap.fromTo('.nt-inner > *', { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.35, stagger: 0.05, ease: 'power2.out' });
		} else if (!visible) {
			shown = false;
		}
	});
</script>

{#if visible}
	<div class="newtab" style={bg ? `background-image: url('${bg.replaceAll("'", "%27")}'); background-size: cover; background-position: center;` : ''}>
		<div class="nt-scrim"></div>
		<form class="nt-inner" on:submit={submit}>
			{#if showClock}
				<div class="clock-block">
					<div class="clock">{clock}</div>
					<div class="date dim">{date}</div>
				</div>
			{/if}
			{#if greeting}
				<p class="greeting">{greeting}</p>
			{/if}
			<input
				bind:this={inputEl}
				type="text"
				placeholder="Search or enter address…"
				autocomplete="off"
				aria-label="Search or enter address"
			/>
			{#if quickLinks.length}
				<div class="quick-links" role="navigation" aria-label="Quick links">
					{#each quickLinks as b (b.id)}
						<button class="quick" type="button" title={b.title} on:click={() => open(b.url)}>
							<span class="q-avatar">{letter(b.url)}</span>
							<span class="q-label">{b.title || hostOf(b.url)}</span>
						</button>
					{/each}
				</div>
			{/if}
			{#if hint}
				<span class="hint-chip">Tip: try split view with Ctrl+\</span>
			{/if}
			<p class="dim small hints">
				Ctrl+K command palette · Ctrl+T new tab · Ctrl+, settings
			</p>
		</form>
	</div>
{/if}

<style>
	.newtab {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background:
			radial-gradient(
				1200px 600px at 50% -10%,
				color-mix(in srgb, var(--umbra-color-accent) 14%, transparent),
				transparent
			),
			var(--umbra-color-bg);
		overflow: hidden;
	}
	.nt-scrim {
		position: absolute;
		inset: 0;
		background: color-mix(in srgb, var(--umbra-color-bg) 55%, transparent);
		pointer-events: none;
	}
	.nt-inner {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 18px;
		width: 100%;
		padding: 24px;
		box-sizing: border-box;
	}
	.clock-block {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
	}
	.clock {
		font-size: 72px;
		font-weight: 200;
		letter-spacing: 2px;
		line-height: 1;
		opacity: 0.92;
		font-variant-numeric: tabular-nums;
	}
	.date {
		font-size: 13px;
		letter-spacing: 1.5px;
		text-transform: capitalize;
	}
	input {
		appearance: none;
		-webkit-appearance: none;
		border: none;
		outline: none;
		margin: 6px 0 0;
		width: 460px;
		max-width: 70vw;
		height: 46px;
		padding: 0 22px;
		box-sizing: border-box;
		text-align: center;
		font-family: inherit;
		font-size: 14px;
		color: var(--umbra-color-fg);
		border-radius: 999px;
		background: color-mix(in srgb, var(--umbra-color-fg) 8%, transparent);
		backdrop-filter: blur(calc(var(--umbra-blur) * 1.5));
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 70%, transparent);
		transition:
			border-color var(--umbra-duration) var(--umbra-ease),
			box-shadow var(--umbra-duration) var(--umbra-ease),
			background var(--umbra-duration) var(--umbra-ease);
	}
	input::placeholder {
		color: var(--umbra-color-fg-dim);
	}
	input:focus {
		border-color: var(--umbra-color-accent);
		box-shadow: 0 0 0 4px color-mix(in srgb, var(--umbra-color-accent) 18%, transparent);
	}
	.quick-links {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 12px;
		max-width: 560px;
	}
	.quick {
		appearance: none;
		-webkit-appearance: none;
		background: transparent;
		border: none;
		font: inherit;
		color: inherit;
		cursor: pointer;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 7px;
		width: 84px;
		padding: 12px 4px 9px;
		border-radius: var(--umbra-radius-small);
		background: color-mix(in srgb, var(--umbra-color-fg) 5%, transparent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 45%, transparent);
		backdrop-filter: blur(var(--umbra-blur));
		transition:
			background var(--umbra-duration) var(--umbra-ease),
			border-color var(--umbra-duration) var(--umbra-ease),
			transform var(--umbra-duration) var(--umbra-ease);
	}
	.quick:hover {
		background: color-mix(in srgb, var(--umbra-color-fg) 11%, transparent);
		border-color: color-mix(in srgb, var(--umbra-color-accent) 50%, transparent);
		transform: translateY(-2px);
	}
	.q-avatar {
		width: 30px;
		height: 30px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		font-size: 13px;
		font-weight: 600;
		color: var(--umbra-color-accent-fg);
		background: color-mix(in srgb, var(--umbra-color-accent) 75%, transparent);
	}
	.q-label {
		width: 100%;
		font-size: 11px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--umbra-color-fg-dim);
	}
	.hint-chip {
		font-size: 12px;
		padding: 4px 12px;
		border-radius: 999px;
		border: 1px dashed color-mix(in srgb, var(--umbra-color-accent) 55%, transparent);
		color: var(--umbra-color-accent);
		background: color-mix(in srgb, var(--umbra-color-accent) 8%, transparent);
	}
	.hints {
		margin: 4px 0 0;
	}
	.greeting {
		margin: -6px 0 0;
		font-size: 15px;
		color: var(--umbra-color-fg-dim);
	}
	/* keep text readable over a custom backdrop */
	.newtab[style*='background-image'] .nt-inner {
		text-shadow: 0 1px 6px rgba(0, 0, 0, 0.55);
	}
</style>
