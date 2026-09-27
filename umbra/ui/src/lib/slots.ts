import type { MountedSpec, UiState } from './types';

/**
 * Slot registry: maps component names declared in UiState.mounted
 * to concrete Svelte implementations. Unknown names render nothing
 * and are logged to the plugin console.
 */
export const SLOT_COMPONENT_NAMES: Record<string, unknown> = {
	'tab-strip': 'TabStrip',
	'url-bar': 'UrlBar',
	'nav-buttons': 'NavButtons',
	'bookmarks-bar': 'BookmarksBar',
	'status-bar': 'StatusBar',
	'command-palette': 'CommandPalette'
};

/** Default mounting when the backend ships no slot registry yet. */
export const DEFAULT_MOUNTED: Record<string, string[]> = {
	'top-bar': ['nav-buttons', 'url-bar', 'tab-strip'],
	'bookmarks-bar': ['bookmarks-bar'],
	'status-bar': ['status-bar'],
	'sidebar-left': ['tab-strip'],
	overlay: ['command-palette']
};

/**
 * Normalize UiState.mounted into Record<slot, components[]>.
 * Accepts both record form and array-of-entries form.
 */
export function mountedFor(state: UiState): Record<string, string[]> {
	const out: Record<string, string[]> = structuredClone(DEFAULT_MOUNTED);
	const m = state.mounted as MountedSpec | undefined;
	if (!m) return out;
	if (Array.isArray(m)) {
		for (const entry of m) {
			if (!entry || typeof entry !== 'object' || !entry.slot) continue;
			const incoming = Array.isArray(entry.components)
				? entry.components
				: entry.component
					? [entry.component]
					: [];
			// Multiple registrations can target one slot (core + plugins) - append, dedupe.
			out[entry.slot] = [...new Set([...(out[entry.slot] ?? []), ...incoming])];
		}
	} else if (typeof m === 'object') {
		for (const [k, v] of Object.entries(m)) {
			if (Array.isArray(v)) out[k] = v.filter((x): x is string => typeof x === 'string');
		}
	}
	// Vertical-tabs layout: the strip lives entirely in the sidebar - never in the topbar.
	if ((state.layout ?? 'compact') === 'sidebar') {
		out['top-bar'] = (out['top-bar'] ?? []).filter((n) => n !== 'tab-strip');
	}
	return out;
}
