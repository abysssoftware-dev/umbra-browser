// Types mirroring CONTRACT.md exactly.

export interface TabInfo {
	id: number;
	title: string;
	url: string;
	active: boolean;
}

export interface Bookmark {
	id: number;
	title: string;
	url: string;
}

export interface HistoryEntry {
	url: string;
	title: string;
	ts: number;
}

export type PkgKind = 'theme' | 'plugin' | 'layout';

export interface PkgCapabilities {
	fs: boolean;
	net: boolean;
	ui: boolean;
}

export interface PkgDiff {
	name: string;
	version: string;
	kind: PkgKind;
	iface_version: string;
	capabilities: PkgCapabilities;
	token_keys: string[];
	components: string[];
	scripts: string[];
	assets: string[];
	warnings: string[];
}

export interface GalleryItem {
	repo: string; // owner/name
	name: string;
	description: string;
	url: string;
	kind: PkgKind | 'unknown';
	stars: number;
}

export interface PluginInfo {
	name: string;
	status: string;
	errors: string[] | string | null;
}

/**
 * The `mounted` field shape is not pinned down by CONTRACT.md; we accept both:
 *  - Record<slotName, componentNames[]>
 *  - Array<{ slot, component? , components? }>
 */
export type MountedSpec = Record<string, string[]> | Array<{ slot: string; component?: string; components?: string[] }>;

export interface UiState {
	tabs: TabInfo[];
	layout: string;
	themePreset?: string;
	theme?: string;
	mode: string;
	keybindings: Record<string, string>;
	flags: Record<string, boolean>;
	safe: boolean;
	layouts?: string[];
	presets?: string[];
	slots?: string[];
	mounted?: MountedSpec;
	bookmarks?: Bookmark[];
	split?: boolean;
	first_run?: boolean;
	newtab?: { url: string; background: string; greeting: string; show_clock: boolean };
	/** User's custom design-token overrides ([theme.tokens] in umbra.toml). */
	themeTokens?: Record<string, string>;
	/** Configured User-Agent override (empty = built-in default). */
	userAgent?: string;
	/** Effective token values (palette + overrides) for editor placeholders. */
	resolvedTokens?: Record<string, string>;
	plugins?: PluginInfo[];
	bookmarksBar?: boolean;
	statusBar?: boolean;
	sidebarWidth?: number;
	/** True when the backend was compiled with debug_assertions. */
	is_debug?: boolean;
}

export interface ToastPayload {
	message: string;
	kind: ToastKind;
}

export type ToastKind = 'info' | 'error' | 'ok';

export interface ConsolePayload {
	level: string;
	message: string;
}

export interface UiEventPayload {
	open?: 'settings' | 'console' | string;
}

export interface Toast {
	id: number;
	message: string;
	kind: ToastKind;
}

export interface ConsoleLine {
	level: string;
	message: string;
	ts: number;
}

/** Pass-2: browser profile found by import_detect. */
export interface ImportSource {
	browser: string; // "chrome" | "brave" | "chromium" | "edge" | "firefox"
	profile: string;
	has_bookmarks: boolean;
	has_history: boolean;
}

/** Pass-2: result of import_browser. */
export interface ImportResult {
	bookmarks_imported: number;
	history_imported: number;
}

/** Pass-2: shape of sync_status / sync_configure responses. */
export interface SyncStatus {
	has_token: boolean;
	gist_id: string | null;
	last_error: string | null;
}

export interface ChromePayload {
	css?: string;
	mode?: string;
	layout?: string;
	bookmarksBar?: boolean;
	statusBar?: boolean;
	sidebarWidth?: number;
}
