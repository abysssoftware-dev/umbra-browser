<script lang="ts">
	import gsap from 'gsap';
	import { tick } from 'svelte';
	import {
		ui,
		activeTab,
		menuOpen,
		paletteOpen,
		settingsOpen,
		settingsSection,
		bookmarkDialog,
		pushToast
	} from '$lib/stores';
	import { cmd } from '$lib/tauri';
	import { trapFocus } from '$lib/actions';

	let btnEl = $state<HTMLButtonElement | undefined>();
	let menuEl = $state<HTMLDivElement | undefined>();

	interface Item {
		label?: string;
		shortcut?: string;
		run?: () => void | Promise<void>;
		sep?: boolean;
	}

	async function bookmarkPage() {
		const t = $activeTab;
		if (!t || t.url === 'umbra://newtab') {
			pushToast('Nothing to bookmark on this page', 'info');
			return;
		}
		bookmarkDialog.set({ id: null, title: t.title, url: t.url });
	}

	async function pip() {
		const on = await cmd<boolean>('pip_toggle');
		if (on !== undefined) pushToast(on ? 'Picture-in-picture opened' : 'Picture-in-picture closed', 'ok');
	}

	const items = $derived<Item[]>([
		{ label: 'New tab', shortcut: 'Ctrl+T', run: () => void cmd('run_action', { action: 'new_tab' }) },
		{ label: 'Bookmark page…', shortcut: 'Ctrl+D', run: bookmarkPage },
		{
			label: $ui.split ? 'Exit split view' : 'Split view toggle',
			shortcut: 'Ctrl+\\',
			run: () => void cmd('run_action', { action: 'toggle_split' })
		},
		{ label: 'Picture-in-picture', shortcut: 'Ctrl+Shift+P', run: pip },
		{ sep: true },
		{ label: 'Settings', run: () => settingsOpen.set(true) },
		{
			label: 'About Umbra',
			run: () => {
				settingsSection.set('about');
				settingsOpen.set(true);
			}
		},
		{ sep: true },
		{ label: 'Command palette', shortcut: 'Ctrl+K', run: () => paletteOpen.set(true) }
	]);

	let sel = $state(0);
	const actionable = $derived(items.map((it, i) => (it.run ? i : -1)).filter((i) => i >= 0));

	function reset() {
		sel = actionable.length ? 0 : -1;
	}

	$effect(() => {
		if ($menuOpen) {
			reset();
			void tick().then(() => {
				if (menuEl)
					gsap.fromTo(
						menuEl,
						{ opacity: 0, scale: 0.95, y: -6 },
						{ opacity: 1, scale: 1, y: 0, duration: 0.16, ease: 'power3.out' }
					);
			});
		}
	});

	async function run(i: number) {
		const it = items[i];
		close();
		await it?.run?.();
	}

	function close() {
		menuOpen.set(false);
	}

	function move(delta: number) {
		if (!actionable.length) return;
		const at = actionable.indexOf(sel);
		const next = at < 0 ? 0 : (at + delta + actionable.length) % actionable.length;
		sel = actionable[next];
	}

	function onWindowMousedown(e: MouseEvent) {
		if (!$menuOpen) return;
		const t = e.target as Node;
		if (menuEl?.contains(t) || btnEl?.contains(t)) return;
		close();
	}

	function onWindowKeydown(e: KeyboardEvent) {
		if (!$menuOpen) return;
		if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			close();
		} else if (e.key === 'ArrowDown') {
			e.preventDefault();
			move(1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			move(-1);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			if (sel >= 0) void run(sel);
		}
	}
</script>

<svelte:window on:mousedown={onWindowMousedown} on:keydown={onWindowKeydown} />

<div class="menu-wrap">
	<button
		bind:this={btnEl}
		class="icon-btn u-menu"
		title="Menu"
		aria-label="Menu"
		aria-expanded={$menuOpen}
		on:click={() => menuOpen.update((v) => !v)}
	>
		☰
	</button>
	{#if $menuOpen}
		<div class="menu glass-strong" bind:this={menuEl} use:trapFocus role="menu" aria-label="Browser menu">
			{#each items as it, i (i)}
				{#if it.sep}
					<hr class="sep" />
				{:else}
					<button
						class="mi"
						class:selected={sel === i}
						role="menuitem"
						tabindex="-1"
						on:mouseenter={() => (sel = i)}
						on:click={() => void run(i)}
					>
						<span class="mi-label">{it.label}</span>
						<span class="mi-kbd">{it.shortcut ?? ''}</span>
					</button>
				{/if}
			{/each}
		</div>
	{/if}
</div>

<style>
	.menu-wrap {
		position: relative;
		display: flex;
		flex-shrink: 0;
	}
	.menu {
		position: absolute;
		top: calc(100% + 6px);
		right: 0;
		min-width: 240px;
		border-radius: var(--umbra-radius);
		padding: 5px;
		display: flex;
		flex-direction: column;
		z-index: 970;
		transform-origin: top right;
	}
	.sep {
		border: none;
		border-top: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
		margin: 5px 8px;
	}
	.mi {
		all: unset;
		cursor: default;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 24px;
		padding: 7px 12px;
		font-size: 12.5px;
		border-radius: var(--umbra-radius-small);
		color: var(--umbra-color-fg);
	}
	.mi.selected {
		background: color-mix(in srgb, var(--umbra-color-accent) 20%, transparent);
	}
	.mi-kbd {
		font-size: 10.5px;
		font-family: var(--umbra-font-mono);
		color: var(--umbra-color-fg-dim);
	}
</style>
