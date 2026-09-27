<script lang="ts">
	import gsap from 'gsap';
	import { tick } from 'svelte';
	import { ui, tutorialOpen } from '$lib/stores';
	import { cmd } from '$lib/tauri';
	import { trapFocus } from '$lib/actions';

	const TOTAL = 4;

	let step = $state(0);
	let cardEl = $state<HTMLDivElement | undefined>();
	let origLayout = $state<string>('compact');
	let accentApplied = $state(false);
	let layoutChanged = $state(false);
	let layoutRestored = $state(false);

	// Capture the user's current layout when the tutorial starts so we can restore it.
	$effect(() => {
		if ($tutorialOpen && step === 0 && !layoutChanged) origLayout = $ui.layout || 'compact';
	});

	$effect(() => {
		if (!$tutorialOpen) return;
		void step; // re-run on every step change
		void tick().then(() => {
			if (cardEl)
				gsap.fromTo(
					cardEl,
					{ y: 16, opacity: 0, scale: 0.97 },
					{ y: 0, opacity: 1, scale: 1, duration: 0.26, ease: 'power3.out' }
				);
		});
	});

	function close() {
		// If the user bailed mid-layout-demo, put their layout back.
		if (layoutChanged && !layoutRestored && ($ui.layout || 'compact') !== origLayout)
			void cmd('set_layout', { preset: origLayout });
		tutorialOpen.set(false);
		setTimeout(() => {
			step = 0;
			accentApplied = false;
			layoutChanged = false;
			layoutRestored = false;
		}, 250);
	}

	function next() {
		step = Math.min(step + 1, TOTAL - 1);
	}
	function back() {
		step = Math.max(step - 1, 0);
	}

	async function runAccentDemo() {
		await cmd('set_token', { key: 'color.accent', value: '#ff7b54' });
		accentApplied = true;
	}

	async function trySidebar() {
		await cmd('set_layout', { preset: 'sidebar' });
		layoutChanged = true;
		layoutRestored = false;
	}

	async function restoreLayout() {
		await cmd('set_layout', { preset: origLayout });
		layoutChanged = true;
		layoutRestored = true;
	}
</script>

<svelte:window on:keydown={(e) => $tutorialOpen && e.key === 'Escape' && close()} />

{#if $tutorialOpen}	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div
		class="tut-overlay u-scrim"
		on:mousedown={(e) => e.target === e.currentTarget && close()}
		role="presentation"
	>
		<div class="tut-card glass-strong" bind:this={cardEl} role="dialog" aria-label="Interactive tutorial" use:trapFocus>
			<header>
				<span class="dots" aria-hidden="true">
					{#each Array(TOTAL) as _, i (i)}<span class="dot" class:on={i === step}></span>{/each}
				</span>
				<span class="spacer"></span>
				<button class="icon-btn" aria-label="Close tutorial" title="Close (Esc)" on:click={close}>×</button>
			</header>

			{#if step === 0}
				<h2>Your config is alive</h2>
				<p>
					Umbra's entire appearance lives in one file:
					<code>~/.config/umbra/umbra.toml</code>. Edit it in any editor and save, and Umbra
					<strong>hot-reloads it live</strong>. No restart, no reload button. The next two steps
					do exactly what a hand-edit does, right now.
				</p>
			{:else if step === 1}
				<h2>Change your accent color live</h2>
				<p>This is a real config edit, applied instantly:</p>
				<pre class="codeblock">token color.accent #ff7b54</pre>
				<div class="row">
					<button class="btn" disabled={accentApplied} on:click={() => void runAccentDemo()}>
						{accentApplied ? 'applied ✓' : 'Run'}
					</button>
					{#if accentApplied}<span class="ok-note small">Watch the chrome recolor. That <em>was</em> the demo.</span>{/if}
				</div>
			{:else if step === 2}
				<h2>Change your layout live</h2>
				<p>Same file, different key. Try switching layouts without touching settings:</p>
				<div class="row">
					<button class="btn" on:click={() => void trySidebar()}>Try sidebar layout</button>
					<button class="btn ghost" disabled={!layoutChanged} on:click={() => void restoreLayout()}>
						Restore “{origLayout}”
					</button>
				</div>
				{#if layoutRestored}<p class="ok-note small">Restored. Nothing was restarted to get here.</p>{/if}
			{:else}
				<h2>That's the whole trick</h2>
				<ul>
					<li>Everything in Settings writes the same <code>umbra.toml</code>.</li>
					<li>Hand edits hot-reload: save the file and see the change.</li>
					<li>A failed edit rolls back automatically with a toast, so a typo can't break your browser.</li>
				</ul>
				<p class="dim small">Dive deeper any time: Settings → Start interactive tutorial.</p>
			{/if}

			<footer>
				<button class="mini" disabled={step === 0} on:click={back}>← back</button>
				<span class="spacer"></span>
				{#if step < TOTAL - 1}
					<button class="btn" on:click={next}>next →</button>
				{:else}
					<button class="btn" on:click={close}>Done</button>
				{/if}
			</footer>
		</div>
	</div>
{/if}

<style>
	.tut-overlay {
		position: fixed;
		inset: 0;
		z-index: 1100;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 20px;
	}
	.tut-card {
		width: 480px;
		max-width: 92vw;
		border-radius: var(--umbra-radius);
		padding: 18px 22px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	header,
	footer {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	header {
		justify-content: space-between;
	}
	h2 {
		margin: 0;
		font-size: 17px;
		font-weight: 500;
		letter-spacing: 0.5px;
	}
	p {
		margin: 0;
		font-size: 13px;
		line-height: 1.55;
		color: var(--umbra-color-fg);
	}
	code {
		font-family: var(--umbra-font-mono);
		font-size: 11.5px;
		background: color-mix(in srgb, var(--umbra-color-fg) 9%, transparent);
		padding: 1px 5px;
		border-radius: 4px;
	}
	.codeblock {
		margin: 0;
		font-family: var(--umbra-font-mono);
		font-size: 13px;
		padding: 10px 14px;
		border-radius: var(--umbra-radius-small);
		background: color-mix(in srgb, var(--umbra-color-fg) 8%, transparent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-accent) 35%, transparent);
		color: var(--umbra-color-accent);
	}
	ul {
		margin: 0;
		padding-left: 18px;
		font-size: 13px;
		line-height: 1.6;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.btn {
		all: unset;
		cursor: default;
		height: 28px;
		padding: 0 14px;
		font-size: 12.5px;
		display: inline-flex;
		align-items: center;
		border-radius: var(--umbra-radius-small);
		background: color-mix(in srgb, var(--umbra-color-accent) 18%, transparent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-accent) 45%, transparent);
		color: var(--umbra-color-fg);
		transition: background var(--umbra-duration) var(--umbra-ease);
	}
	.btn:hover:not(:disabled) {
		background: color-mix(in srgb, var(--umbra-color-accent) 32%, transparent);
	}
	.btn:disabled {
		opacity: 0.55;
	}
	.btn.ghost {
		background: transparent;
		border-color: color-mix(in srgb, var(--umbra-color-border) 90%, transparent);
	}
	.mini {
		all: unset;
		cursor: default;
		font-size: 11.5px;
		padding: 3px 8px;
		border-radius: 4px;
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 80%, transparent);
	}
	.mini:disabled {
		opacity: 0.4;
	}
	.icon-btn {
		all: unset;
		cursor: default;
		width: 24px;
		height: 24px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--umbra-radius-small);
		color: var(--umbra-color-fg-dim);
	}
	.icon-btn:hover {
		background: color-mix(in srgb, var(--umbra-color-fg) 10%, transparent);
		color: var(--umbra-color-fg);
	}
	.ok-note {
		color: var(--umbra-color-ok);
	}
	.dots {
		display: inline-flex;
		gap: 5px;
	}
	.dot {
		width: 6px;
		height: 6px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--umbra-color-fg) 25%, transparent);
		transition: background var(--umbra-duration) var(--umbra-ease);
	}
	.dot.on {
		background: var(--umbra-color-accent);
	}
	.spacer {
		flex: 1;
	}
</style>
