import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauri } from './tauri';
import { winFullscreen, winMaximized } from './stores';

/** Mirror native maximize/fullscreen state into the stores (no-op in dev browser). */
export async function refreshWindowState(): Promise<void> {
	if (!isTauri) return;
	const w = getCurrentWindow();
	winMaximized.set(await w.isMaximized());
	winFullscreen.set(await w.isFullscreen());
}

/** Toggle native maximize/restore (window control button). */
export async function toggleMaximize(): Promise<void> {
	if (!isTauri) {
		winMaximized.update((v) => !v);
		return;
	}
	const w = getCurrentWindow();
	await w.toggleMaximize();
	winMaximized.set(await w.isMaximized());
}

/** Toggle native window fullscreen. Handled entirely UI-side. */
export async function toggleFullscreen(): Promise<void> {
	if (!isTauri) {
		winFullscreen.update((v) => !v);
		return;
	}
	const w = getCurrentWindow();
	const next = !(await w.isFullscreen());
	await w.setFullscreen(next);
	winFullscreen.set(next);
}

/** Set native window title. */
export async function setWindowTitle(title: string): Promise<void> {
	if (!isTauri) {
		if (typeof document !== 'undefined') document.title = title;
		return;
	}
	const w = getCurrentWindow();
	await w.setTitle(title);
}
