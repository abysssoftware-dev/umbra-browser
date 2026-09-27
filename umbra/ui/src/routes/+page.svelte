<script lang="ts">
	import { onMount } from 'svelte';
	import {
		ui,
		tokenCss,
		kb,
		applyState,
		pushToast,
		logLine,
		paletteOpen,
		settingsOpen,
		consoleOpen,
		pendingPackage,
		tutorialOpen,
		focusUrlTick,
		menuOpen,
		bookmarkDialog,
		activeTab
	} from '$lib/stores';
	import { cmd, onEvent, isTauri } from '$lib/tauri';
	import { comboOf } from '$lib/keybinds';
	import { toggleFullscreen, setWindowTitle } from '$lib/window';
	import SlotHost from '$lib/components/SlotHost.svelte';
	import NewTab from '$lib/components/NewTab.svelte';
	import Toasts from '$lib/components/Toasts.svelte';
	import CommandPalette from '$lib/components/CommandPalette.svelte';
	import SettingsPanel from '$lib/components/SettingsPanel.svelte';
	import SettingsOverlay from '$lib/components/SettingsOverlay.svelte';
	import PluginConsole from '$lib/components/PluginConsole.svelte';
	import ConfigDiffDialog from '$lib/components/ConfigDiffDialog.svelte';
	import ModalBar from '$lib/components/ModalBar.svelte';
	import Onboarding from '$lib/components/Onboarding.svelte';
	import Tutorial from '$lib/components/Tutorial.svelte';
	import MenuDropdown from '$lib/components/MenuDropdown.svelte';
	import WindowControls from '$lib/components/WindowControls.svelte';
	import BookmarkDialog from '$lib/components/BookmarkDialog.svelte';
	import type { ChromePayload, ConsolePayload, ToastPayload, UiEventPayload, UiState } from '$lib/types';

	let contentEl: HTMLElement | undefined = $state();
	let observer: ResizeObserver | null = null;
	let resizeTimer: ReturnType<typeof setTimeout> | undefined;
	let lastBounds: { x: number; y: number; w: number; h: number } | null = null;

	const layout = $derived($ui.layout);
	const mode = $derived($ui.mode);
	const tilingHint = $derived(layout === 'tiling' && $ui.tabs.length > 2 && $ui.flags.contextual_hints !== false);
	const isDebug = $derived($ui.is_debug === true);
	// The chrome is stacked above the tabs; when a real web page is active the
	// content area is click-through (GTK input-shape routes to the tab). The
	// built-in new-tab page needs interaction, so it opts back in.
	const contentInteractive = $derived($activeTab?.url === 'umbra://newtab');

	let debugBannerDismissed = $state(false);

	function reportBounds() {
		if (!isTauri || !contentEl) return;
		const r = contentEl.getBoundingClientRect();
		// Physical pixels: wry positions child webviews through two scale-factor
		// conversions when given logical units (tauri-runtime converts with the
		// window's scale, wry again with the webview widget's scale) - any
		// mismatch displaces the page. Sending device pixels skips both.
		const dpr = window.devicePixelRatio || 1;
		const b = { x: r.x * dpr, y: r.y * dpr, w: r.width * dpr, h: r.height * dpr };
		// Skip redundant updates: every update triggers reposition_all (moves and
		// resizes every tab webview), a major jank source if fired often.
		if (lastBounds) {
			const d =
				Math.abs(lastBounds.x - b.x) +
				Math.abs(lastBounds.y - b.y) +
				Math.abs(lastBounds.w - b.w) +
				Math.abs(lastBounds.h - b.h);
			if (d < 1) return;
		}
		lastBounds = b;
		void cmd('update_content_bounds', b);
	}

	/** Debounced window-resize backup for the #content ResizeObserver. */
	function onWindowResize() {
		clearTimeout(resizeTimer);
		resizeTimer = setTimeout(reportBounds, 150);
	}

	function bookmarkActiveTab() {
		const t = $ui.tabs.find((x) => x.active);
		if (!t || t.url === 'umbra://newtab') {
			pushToast('Nothing to bookmark on this page', 'info');
			return;
		}
		bookmarkDialog.set({ id: null, title: t.title, url: t.url });
	}

	function handleKeydown(e: KeyboardEvent) {
		if ($paletteOpen || $tutorialOpen || $menuOpen) return; // overlays handle their own keys
		if (e.key === 'Escape') {
			if ($pendingPackage) return; // dialog handles Escape (cancel)
			if ($consoleOpen) {
				consoleOpen.set(false);
				return;
			}
			if ($settingsOpen) {
				settingsOpen.set(false);
				return;
			}
		}
		const action = $kb[comboOf(e)];
		if (!action) return;
		e.preventDefault();
		if (action === 'palette') paletteOpen.set(true);
		else if (action === 'focus_url') focusUrlTick.update((n) => n + 1);
		else if (action === 'open_settings') settingsOpen.set(true);
		else if (action === 'open_console') consoleOpen.set(true);
		else if (action === 'toggle_fullscreen') void toggleFullscreen();
		else if (action === 'bookmark_page') bookmarkActiveTab();
		else void cmd('run_action', { action });
	}

	$effect(() => {
		// Whenever layout, bookmarks, status bar or tabs change, re-measure content bounds
		const _l = $ui.layout;
		const _b = $ui.bookmarksBar;
		const _s = $ui.statusBar;
		const _d = debugBannerDismissed;
		requestAnimationFrame(reportBounds);
	});

	// Manage native tab webview interactivity: while any full-screen chrome
	// overlay is open (Settings, Tutorial, Onboarding, Palette, Console,
	// Package diff, Bookmark dialog), tell the backend to blank the tab
	// webviews' input regions and clear the chrome input-shape carve, so the
	// overlay receives every pointer event (the GTK input-shape hole would
	// otherwise let clicks fall through to the page underneath). The webviews
	// themselves stay VISIBLE so the page shows behind translucent overlay
	// backdrops instead of the desktop.
	$effect(() => {
		const shouldHide =
			$settingsOpen ||
			$tutorialOpen ||
			$pendingPackage !== null ||
			$paletteOpen ||
			$consoleOpen ||
			$bookmarkDialog !== null ||
			$ui.first_run === true;
		void cmd('set_webviews_visible', { visible: !shouldHide });
		if (!shouldHide) {
			requestAnimationFrame(reportBounds);
		}
	});

	// While the hamburger dropdown is open it can overlap the content area (which
	// is otherwise click-through to the tab), so tell the backend to keep the
	// chrome capturing all pointer events.
	$effect(() => {
		const open = $menuOpen;
		void cmd('set_menu_open', { open });
	});

	// Manage window title
	$effect(() => {
		if ($settingsOpen) {
			void setWindowTitle('Settings - Umbra');
		} else {
			const t = $activeTab?.title;
			void setWindowTitle(t && t !== 'New Tab' ? `${t} - Umbra` : 'Umbra');
		}
	});

	onMount(() => {
		(async () => {
			const st = await cmd<Partial<UiState>>('get_ui_state');
			if (st) applyState(st);
			await onEvent<Partial<typeof $ui>>('umbra://state', (p) => applyState(p));
			await onEvent<Partial<typeof $ui>>('umbra://ui_state', (p) => applyState(p));
			await onEvent<ChromePayload>('umbra://chrome', (p) => {
				if (typeof p.css === 'string') tokenCss.set(p.css);
				const patch: Record<string, unknown> = { ...p };
				delete patch.css;
				applyState(patch);
			});
			await onEvent<ToastPayload>('umbra://toast', (p) => pushToast(p.message, p.kind));
			await onEvent<ConsolePayload>('umbra://console', (p) => logLine(p.level, p.message));
			await onEvent<UiEventPayload>('umbra://ui', (p) => {
				if (p.open === 'settings') settingsOpen.set(true);
				else if (p.open === 'console') consoleOpen.set(true);
				else if (p.open === 'palette') paletteOpen.set(true);
				else logLine('warn', `unknown umbra://ui action: ${JSON.stringify(p)}`);
			});
			reportBounds();
			setTimeout(reportBounds, 100);
			setTimeout(reportBounds, 300);
		})();

		observer = new ResizeObserver(reportBounds);
		if (contentEl) observer.observe(contentEl);
		window.addEventListener('resize', onWindowResize);
		return () => {
			observer?.disconnect();
			window.removeEventListener('resize', onWindowResize);
			clearTimeout(resizeTimer);
		};
	});

</script>

<svelte:window on:keydown={handleKeydown} />
<svelte:head>
	{@html `<style>${$tokenCss}</style>`}
</svelte:head>

<div
	class="u-root u-noise"
	data-layout={layout}
	data-mode={mode}
	data-bookmarks={$ui.bookmarksBar ? 'on' : 'off'}
	data-status={$ui.statusBar ? 'on' : 'off'}
	data-tiling-hint={tilingHint || undefined}
	style="--layout-sidebar-width: {$ui.sidebarWidth ?? 220}px"
>
	{#if !isTauri}
		<div class="mock-banner" role="status">
			Umbra dev mode: Tauri backend not detected. All commands are mocked.
		</div>
	{/if}

	{#if isDebug && !debugBannerDismissed}
		<div class="debug-banner" role="status" aria-live="polite">
			<span>⚙ Running in <strong>debug mode</strong>. Onboarding will show on every launch.</span>
			<button class="debug-close icon-btn" aria-label="Dismiss debug notice" onclick={() => (debugBannerDismissed = true)}>✕</button>
		</div>
	{/if}

	{#if layout === 'sidebar'}
		<aside class="u-sidebar glass" aria-label="Sidebar">
			<img class="side-logo-img" src="/logo.png" alt="Umbra" />
			<SlotHost region="sidebar-left" />
		</aside>
	{/if}

	<div class="u-frame">
		<header class="u-topbar glass" data-tauri-drag-region>
			<SlotHost region="top-bar" />
			<span class="tb-spacer" data-tauri-drag-region></span>
			<MenuDropdown />
			<WindowControls />
		</header>

		<div class="u-region-bookmarks" data-slot="bookmarks-bar">
			<SlotHost region="bookmarks-bar" />
		</div>

		<main class="u-content" bind:this={contentEl} class:pointer-auto={contentInteractive}>
			<NewTab />
		</main>

		<footer class="u-region-status" data-slot="status-bar">
			<SlotHost region="status-bar" />
		</footer>
	</div>

	<CommandPalette />
	<SettingsOverlay />
	<PluginConsole />
	<ConfigDiffDialog />
	<BookmarkDialog />
	<ModalBar />
	<Onboarding />
	<Tutorial />
	<Toasts />
</div>

<style>
	.u-root {
		position: fixed;
		inset: 0;
		display: flex;
		flex-direction: row;
		font-family: var(--umbra-font-ui);
		color: var(--umbra-color-fg);
	}
	u-root[data-mode='light'] .side-logo-img {
		filter: brightness(0) invert();
	}

	.u-root[data-mode='light'] {
		color-scheme: light;
	}
	.u-root[data-mode='dark'] {
		color-scheme: dark;
	}

	/* noise/grain overlay for readability over blur */
	.u-noise::after {
		content: '';
		position: fixed;
		inset: 0;
		pointer-events: none;
		opacity: var(--umbra-noise-opacity);
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='128' height='128'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
		z-index: 999;
	}

	.mock-banner {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 1200;
		text-align: center;
		font-size: 11px;
		padding: 3px 8px;
		font-family: var(--umbra-font-mono);
		color: var(--umbra-color-accent-fg);
		background: color-mix(in srgb, var(--umbra-color-danger) 70%, black);
	}

	.debug-banner {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 1100;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 10px;
		font-size: 11px;
		padding: 4px 10px;
		font-family: var(--umbra-font-mono);
		color: color-mix(in srgb, var(--umbra-color-fg) 90%, transparent);
		background: color-mix(in srgb, var(--umbra-color-accent) 22%, var(--umbra-color-bg));
		border-bottom: 1px solid color-mix(in srgb, var(--umbra-color-accent) 45%, transparent);
	}
	.debug-banner strong {
		color: var(--umbra-color-accent);
	}
	.debug-close {
		width: 20px;
		height: 20px;
		font-size: 11px;
		flex-shrink: 0;
	}

	.u-sidebar {
		display: flex;
		flex-direction: column;
		gap: var(--umbra-spacing);
		width: var(--layout-sidebar-width, 220px);
		min-width: 0;
		height: 100%;
		padding: var(--umbra-spacing);
		border-right: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
	}
	.side-logo-img {
		width: 32px;
		height: 32px;
		object-fit: contain;
		margin: 2px auto 4px;
		border-radius: var(--umbra-radius-small);
	}

	.u-frame {
		display: flex;
		flex-direction: column;
		flex: 1 1 auto;
		min-width: 0;
		min-height: 0;
		height: 100%;
		width: 100%;
	}

	.u-topbar {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		gap: calc(var(--umbra-spacing) * 1.5);
		padding: calc(var(--umbra-spacing) * 1.2) calc(var(--umbra-spacing) * 1.5);
		min-height: var(--umbra-chrome-height);
		border-bottom: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
		z-index: 100;
	}
	.tb-spacer {
		flex: 1;
		align-self: stretch;
	}

	.u-content {
		position: relative;
		flex: 1 1 auto;
		min-height: 0;
		width: 100%;
		height: 100%;
		/* Click-through when a real web page is active; the GTK input-shape carve-out
		   routes pointer events to the tab webview below. The new-tab page needs
		   interaction, so it opts back in via .pointer-auto. */
		pointer-events: none;
	}
	.u-content.pointer-auto {
		pointer-events: auto;
	}

	.u-region-bookmarks {
		flex-shrink: 0;
		border-bottom: 1px solid color-mix(in srgb, var(--umbra-color-border) 50%, transparent);
	}
	.u-root[data-bookmarks='off'] .u-region-bookmarks,
	.u-root[data-layout='zen'] .u-region-bookmarks,
	.u-root[data-layout='minimal'] .u-region-bookmarks {
		display: none;
	}

	.u-region-status {
		flex-shrink: 0;
		border-top: 1px solid color-mix(in srgb, var(--umbra-color-border) 50%, transparent);
	}
	.u-root[data-status='off'] .u-region-status,
	.u-root[data-layout='zen'] .u-region-status {
		display: none;
	}

	/* minimal: nav buttons hidden (component CSS), url pill centered */
	.u-root[data-layout='minimal'] .u-topbar {
		justify-content: center;
	}
	.u-root[data-layout='minimal'] .tb-spacer {
		display: none;
	}

	/* zen: chrome fades out until hover/focus */
	.u-root[data-layout='zen'] .u-topbar {
		position: absolute;
		z-index: 50;
		left: 0;
		right: 0;
		top: 0;
		opacity: 0;
		transition: opacity var(--umbra-duration) var(--umbra-ease);
	}
	.u-root[data-layout='zen'] .u-topbar:hover,
	.u-root[data-layout='zen'] .u-topbar:focus-within {
		opacity: 1;
	}

	/* sidebar layout reveals the vertical tab strip */
	/* the sidebar strip owns the full column height */
	.u-sidebar :global(.tabstrip.vertical) {
		flex: 1;
		min-height: 0;
	}

	/* tiling hint outline */
	.u-root[data-layout='tiling'][data-tiling-hint] .u-content::before {
		content: '';
		position: absolute;
		inset: 12px;
		border: 2px dashed color-mix(in srgb, var(--umbra-color-accent) 40%, transparent);
		border-radius: var(--umbra-radius);
		pointer-events: none;
	}
</style>
