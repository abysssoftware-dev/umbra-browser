import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { logLine, pushToast, DEFAULT_STATE } from './stores';
import type { ImportSource, SyncStatus, UiState } from './types';

/** True when running inside a Tauri webview (backend present). */
export const isTauri: boolean =
	typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

// Mutable mock state so dev-browser sync flows feel real across calls.
let mockSyncStatus: SyncStatus = { has_token: false, gist_id: null, last_error: null };

function mockResponse<T>(name: string, args?: Record<string, unknown>): T | undefined {
	switch (name) {
		case 'get_ui_state':
			return structuredClone(DEFAULT_STATE) as unknown as T;
		case 'import_detect':
			return [
				{ browser: 'firefox', profile: 'default-release', has_bookmarks: true, has_history: true },
				{ browser: 'chrome', profile: 'Default', has_bookmarks: true, has_history: false },
				{ browser: 'brave', profile: 'Default', has_bookmarks: false, has_history: true }
			] as unknown as T;
		case 'import_browser': {
			const b = args?.bookmarks === true ? 12 : 0;
			const h = args?.history === true ? 340 : 0;
			return { bookmarks_imported: b, history_imported: h } as unknown as T;
		}
		case 'sync_status':
			return { ...mockSyncStatus } as unknown as T;
		case 'sync_configure': {
			const token = typeof args?.token === 'string' ? args.token.trim() : '';
			const gist = typeof args?.gist_id === 'string' ? args.gist_id.trim() : '';
			mockSyncStatus = {
				has_token: token.length > 0,
				gist_id: token.length > 0 && gist.length > 0 ? gist : mockSyncStatus.gist_id,
				last_error: null
			};
			return { ...mockSyncStatus } as unknown as T;
		}
		case 'sync_push':
			mockSyncStatus.gist_id ??= 'a1b2c3d4e5f6mockgist0000000000';
			return 'Pushed umbra.toml + bookmarks.toml to gist' as unknown as T;
		case 'sync_pull':
			return 'Pulled config from gist, merged bookmarks' as unknown as T;
		case 'pip_toggle':
			return true as unknown as T;
		case 'gallery_list':
			return [
				{
					repo: 'umbra-themes/gruvbox-tui',
					name: 'Gruvbox TUI',
					description: 'Boxy pixelated gruvbox with bitmap fonts.',
					url: 'https://github.com/umbra-themes/gruvbox-tui',
					kind: 'theme',
					stars: 42
				},
				{
					repo: 'umbra-plugins/adblock-lite',
					name: 'adblock-lite',
					description: 'Lightweight tracker blocker.',
					url: 'https://github.com/umbra-plugins/adblock-lite',
					kind: 'plugin',
					stars: 17
				}
			] as unknown as T;
		default:
			return undefined;
	}
}

/**
 * Invoke a Tauri command per CONTRACT.md.
 * In dev browser mode (`npm run dev` without Tauri), commands are mocked:
 * logged to the plugin console and resolved with sensible defaults.
 * Failures surface as an error toast and resolve to undefined.
 */
export async function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T | undefined> {
	if (!isTauri) {
		logLine('warn', `[mock] ${name} ${args ? JSON.stringify(args) : ''}`);
		return mockResponse<T>(name, args);
	}
	try {
		return await tauriInvoke<T>(name, args ?? {});
	} catch (e) {
		pushToast(`${name}: ${String(e)}`, 'error');
		logLine('error', `[cmd] ${name} failed: ${String(e)}`);
		return undefined;
	}
}

/**
 * Listen to a backend event (payload-only handler).
 * No-op in dev browser mode.
 */
export async function onEvent<T>(
	event: string,
	handler: (payload: T) => void
): Promise<() => void> {
	if (!isTauri) {
		logLine('warn', `[mock] listening on ${event}`);
		return () => {};
	}
	const unlisten: UnlistenFn = await tauriListen<T>(event, (e) => handler(e.payload));
	return unlisten;
}
