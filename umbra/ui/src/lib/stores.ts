import { writable, derived } from 'svelte/store';
import { DEFAULT_KEYBINDINGS } from './keybinds';
import type {
	ConsoleLine,
	PkgDiff,
	Toast,
	ToastKind,
	UiState
} from './types';

export const FALLBACK_LAYOUTS = ['compact', 'minimal', 'sidebar', 'zen'];
export const FALLBACK_PRESETS = ['glass', 'material', 'gruvbox', 'nord', 'catppuccin', 'ocean'];

export const DEFAULT_STATE: UiState = {
	tabs: [],
	layout: 'compact',
	themePreset: 'glass',
	mode: 'dark',
	keybindings: {},
	flags: {},
	safe: false,
	slots: [],
	mounted: undefined,
	bookmarks: [],
	split: false,
	first_run: false,
	plugins: [],
	bookmarksBar: true,
	statusBar: true,
	sidebarWidth: 220,
	is_debug: false
};

// ── core state ───────────────────────────────────────────────────────────────

export const ui = writable<UiState>(structuredClone(DEFAULT_STATE));

/** Merge a partial UiState (event payload or command result) into the store. */
export function applyState(patch: Partial<UiState>): void {
	ui.update((s) => ({ ...s, ...patch }));
}

/** Effective keybindings: config overrides layered on defaults. */
export const kb = derived(ui, (s) => ({ ...DEFAULT_KEYBINDINGS, ...s.keybindings }));

export const activeTab = derived(ui, (s) => s.tabs.find((t) => t.active) ?? null);

// Injected theme CSS from umbra://chrome
export const tokenCss = writable<string>('');

// ── toasts ───────────────────────────────────────────────────────────────────

export const toasts = writable<Toast[]>([]);
let toastSeq = 0;

export function pushToast(message: string, kind: ToastKind = 'info'): void {
	const id = ++toastSeq;
	toasts.update((t) => [...t.slice(-5), { id, message, kind }]);
	setTimeout(() => dismissToast(id), 4000);
}

export function dismissToast(id: number): void {
	toasts.update((t) => t.filter((x) => x.id !== id));
}

// ── plugin console ───────────────────────────────────────────────────────────

export const consoleLines = writable<ConsoleLine[]>([]);

export function logLine(level: string, message: string): void {
	consoleLines.update((l) => [...l.slice(-500), { level, message, ts: Date.now() }]);
}

// ── drawers / overlays ───────────────────────────────────────────────────────

export const paletteOpen = writable(false);
export const settingsOpen = writable(false);
export const consoleOpen = writable(false);
export const focusUrlTick = writable(0);

export const pendingPackage = writable<PkgDiff | null>(null);
export const tutorialOpen = writable(false);

/** Hamburger menu dropdown visibility. */
export const menuOpen = writable(false);

/** Native window state (mirrored from the Tauri window; browser dev mode toggles freely). */
export const winMaximized = writable(false);
export const winFullscreen = writable(false);

/** Active section inside the full-page settings view. */
export type SettingsSection =
	| 'appearance'
	| 'layout'
	| 'browser'
	| 'newtab'
	| 'keybindings'
	| 'flags'
	| 'import'
	| 'sync'
	| 'plugins'
	| 'config'
	| 'about';
export const settingsSection = writable<SettingsSection>('appearance');

/**
 * Chrome-style bookmark dialog draft. `id` is set when editing an existing
 * bookmark; null when creating one for a tab. URL is immutable in the dialog.
 */
export interface BookmarkDraft {
	id: number | null;
	title: string;
	url: string;
}
export const bookmarkDialog = writable<BookmarkDraft | null>(null);
