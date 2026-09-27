/** Build a normalized combo string from a KeyboardEvent (legacy comboOf logic). */
export function comboOf(e: KeyboardEvent): string {
	let c = '';
	if (e.ctrlKey) c += 'Ctrl+';
	if (e.altKey) c += 'Alt+';
	if (e.metaKey) c += 'Meta+';
	const k =
		e.key === ' '
			? 'Space'
			: e.key === 'Escape'
				? 'Esc'
				: e.key.startsWith('Arrow')
					? e.key.slice(5)
					: e.key.length === 1
						? e.key.toUpperCase()
						: e.key;
	// Shift only matters for keys whose identity it does not already encode
	// (letters/digits and named keys): "Ctrl+Shift+D" must not collapse to
	// "Ctrl+D", while symbols like "\" or "," keep matching the bindings
	// written for their base key regardless of layout-driven Shift state.
	if (e.shiftKey && (k.length > 1 || /[a-z0-9]/i.test(k))) c += 'Shift+';
	return c + k;
}

export const DEFAULT_KEYBINDINGS: Record<string, string> = {
	'Ctrl+K': 'palette',
	'Ctrl+L': 'focus_url',
	'Ctrl+T': 'new_tab',
	'Ctrl+W': 'close_tab',
	'Ctrl+Shift+D': 'toggle_mode',
	'Ctrl+Alt+L': 'cycle_layout',
	'Ctrl+D': 'bookmark_page',
	'Ctrl+\\': 'toggle_split',
	'Ctrl+,': 'open_settings',
	'Ctrl+Shift+C': 'open_console',
	'Ctrl+Shift+S': 'toggle_modal',
	'Ctrl+Shift+P': 'pip',
	F11: 'toggle_fullscreen'
};

export const KNOWN_ACTIONS: string[] = [
	'palette',
	'new_tab',
	'close_tab',
	'next_tab',
	'prev_tab',
	'back',
	'forward',
	'reload',
	'focus_url',
	'reload_config',
	'toggle_mode',
	'cycle_layout',
	'bookmark_page',
	'toggle_split',
	'open_settings',
	'open_console',
	'toggle_modal',
	'pip',
	'toggle_fullscreen'
];
