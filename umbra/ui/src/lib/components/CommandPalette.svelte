<script lang="ts">
	import gsap from 'gsap';
	import { tick } from 'svelte';
	import { ui, paletteOpen, pushToast, tutorialOpen } from '$lib/stores';
	import { cmd } from '$lib/tauri';
	import { toggleFullscreen } from '$lib/window';
	import type { Entry } from '$lib/palette-types';
	import { trapFocus } from '$lib/actions';

	let inputEl = $state<HTMLInputElement | undefined>();
	let panelEl = $state<HTMLDivElement | undefined>();
	let shown = $state(false);
	let filter = $state('');
	let sel = $state(0);

	function fuzzy(q: string, text: string): number {
		if (!q) return 1;
		q = q.toLowerCase();
		text = text.toLowerCase();
		let i = 0;
		let score = 0;
		let last = -2;
		for (const ch of q) {
			const idx = text.indexOf(ch, i);
			if (idx < 0) return -1;
			score += idx === last + 1 ? 3 : 1;
			last = idx;
			i = idx + 1;
		}
		return score;
	}

	function baseEntries(s: typeof $ui): Entry[] {
		const acts: Entry[] = [];
		for (const l of s.layouts ?? ['compact', 'minimal', 'sidebar', 'tiling', 'floating', 'zen'])
			acts.push({ label: `Layout: ${l}`, hint: 'set_layout', run: () => void cmd('set_layout', { preset: l }) });
		for (const p of s.presets ?? ['glass', 'gruvbox', 'nord', 'catppuccin', 'ocean'])
			acts.push({ label: `Theme: ${p}`, hint: 'set_theme_preset', run: () => void cmd('set_theme_preset', { preset: p }) });
		acts.push({ label: 'Mode: toggle dark / light', hint: 'toggle_mode', run: () => void cmd('run_action', { action: 'toggle_mode' }) });
		acts.push({ label: 'Reload config from disk', hint: 'reload_config', run: async () => { const r = await cmd<string>('reload_config'); if (r !== undefined) pushToast(r === 'ok' ? 'Config reloaded' : r, 'ok'); } });
		acts.push({ label: 'Reset custom colors to theme defaults', hint: 'reset_tokens', run: async () => { await cmd('reset_tokens'); pushToast('Tokens reset', 'ok'); } });
		acts.push({ label: 'New tab', hint: 'new_tab', run: () => void cmd('run_action', { action: 'new_tab' }) });
		acts.push({ label: 'Close tab', hint: 'close_tab', run: () => void cmd('run_action', { action: 'close_tab' }) });
		acts.push({
			label: 'Toggle picture-in-picture',
			hint: 'pip_toggle',
			run: async () => {
				const on = await cmd<boolean>('pip_toggle');
				if (on !== undefined) pushToast(on ? 'Picture-in-picture opened' : 'Picture-in-picture closed', 'ok');
			}
		});
		acts.push({ label: 'Toggle fullscreen', hint: 'F11 · window only, no backend round-trip', run: () => void toggleFullscreen() });
		acts.push({
			label: 'Start tutorial',
			hint: 'interactive hot-reload walkthrough',
			run: () => tutorialOpen.set(true)
		});
		for (const t of s.tabs)
			acts.push({ label: `Go to: ${t.title || t.url}`, hint: 'activate_tab', run: () => void cmd('activate_tab', { id: t.id }) });
		return acts;
	}

	function tokenEntry(q: string): Entry[] {
		const m = q.match(/^token\s+(\S+)\s+(.+)$/i);
		if (!m) return [];
		const [, key, val] = m;
		return [
			{
				label: `Set token ${key} = ${val}`,
				hint: 'set_token',
				run: async () => {
					await cmd('set_token', { key, value: val });
					pushToast(`${key} set`, 'ok');
				}
			}
		];
	}

	function itemsFor(q: string): Entry[] {
		const tok = tokenEntry(q);
		if (tok.length) return tok;
		const all = baseEntries($ui);
		const f = q.trim().toLowerCase();
		if (!f) return all;
		return all
			.map((a) => ({ entry: a, score: fuzzy(f, `${a.label} ${a.hint}`) }))
			.filter((x) => x.score >= 0)
			.sort((a, b) => b.score - a.score)
			.map((x) => x.entry);
	}

	const items = $derived(shown ? itemsFor(filter) : []);

	$effect(() => {
		if (sel >= items.length) sel = Math.max(0, items.length - 1);
	});
	$effect(() => {
		if ($paletteOpen && !shown) void open();
	});

	async function focusInput() {
		await tick();
		inputEl?.focus();
	}

	async function open() {
		filter = '';
		sel = 0;
		shown = true;
		paletteOpen.set(true);
		await tick();
		if (panelEl) {
			gsap.fromTo(
				panelEl,
				{ opacity: 0, scale: 0.96, y: -8 },
				{ opacity: 1, scale: 1, y: 0, duration: 0.18, ease: 'power3.out' }
			);
		}
		focusInput();
	}

	async function close() {
		paletteOpen.set(false);
		const el = panelEl;
		if (el) {
			gsap.to(el, {
				opacity: 0,
				scale: 0.97,
				duration: 0.12,
				ease: 'power1.in',
				onComplete: () => (shown = false)
			});
		} else {
			shown = false;
		}
	}

	async function runSelected() {
		const e = items[sel] ?? items[0];
		if (!e || e.hint === '') return; // placeholder entries do nothing
		await close();
		e.run();
	}

	function onWindowKeydown(e: KeyboardEvent) {
		if (!shown) return;
		if (e.key === 'Escape') {
			e.preventDefault();
			void close();
		} else if (e.key === 'ArrowDown') {
			e.preventDefault();
			sel = Math.min(sel + 1, items.length - 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			sel = Math.max(sel - 1, 0);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			void runSelected();
		}
	}
</script>

<svelte:window on:keydown={onWindowKeydown} />

{#if shown}
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div
		class="palette-backdrop u-scrim"
		on:mousedown={(e) => e.target === e.currentTarget && close()}
		role="presentation"
	>
		<div class="palette glass-strong" bind:this={panelEl} use:trapFocus role="dialog" aria-label="Command palette">
			<input
				bind:this={inputEl}
				bind:value={filter}
				on:input={() => (sel = 0)}
				type="text"
				placeholder="Type a command… (layout, theme, mode, token)"
				autocomplete="off"
				aria-label="Command palette input"
			/>
			<ul class="pal-list" role="listbox" aria-label="Commands">
				{#each items as e, i (e.label)}
					<li
						class:selected={i === sel}
						role="option"
						aria-selected={i === sel}
						tabindex="-1"
						on:click={() => { sel = i; void runSelected(); }}
						on:mousemove={() => (sel = i)}
					>
						<span>{e.label}</span>
						<span class="hint">{e.hint}</span>
					</li>
				{:else}
					<li class="dim">No matching commands</li>
				{/each}
			</ul>
		</div>
	</div>
{/if}

<style>
	.palette-backdrop {
		position: fixed;
		inset: 0;
		z-index: 900;
		display: flex;
		align-items: flex-start;
		justify-content: center;
		padding-top: 12vh;
	}
	.palette {
		width: 520px;
		max-width: 86vw;
		border-radius: var(--umbra-radius);
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}
	input {
		all: unset;
		display: block;
		width: 100%;
		height: 46px;
		padding: 0 16px;
		box-sizing: border-box;
		font-size: 14px;
		border-bottom: 1px solid var(--umbra-color-border);
	}
	.busy {
		padding: 6px 16px;
	}
	.pal-list {
		list-style: none;
		margin: 0;
		max-height: 320px;
		overflow-y: auto;
		padding: 6px;
	}
	li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		padding: 9px 12px;
		font-size: 13px;
		border-radius: var(--umbra-radius-small);
		color: var(--umbra-color-fg);
		cursor: default;
	}
	li .hint {
		font-size: 11px;
		color: var(--umbra-color-fg-dim);
		font-family: var(--umbra-font-mono);
		flex-shrink: 0;
	}
	li.selected {
		background: var(--umbra-color-accent);
		color: var(--umbra-color-accent-fg);
	}
	li.selected .hint {
		color: color-mix(in srgb, var(--umbra-color-accent-fg) 75%, transparent);
	}
	.back {
		all: unset;
		cursor: default;
		padding: 6px 14px;
		border-top: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
	}
</style>
