/** Focus-trap action: keeps Tab cycling inside `node`. Escape is left to callers. */
export function trapFocus(node: HTMLElement) {
	function focusables(): HTMLElement[] {
		return Array.from(
			node.querySelectorAll<HTMLElement>(
				'button, input, select, textarea, a[href], [tabindex]:not([tabindex="-1"])'
			)
		).filter((el) => !el.hasAttribute('disabled') && el.offsetParent !== null);
	}
	function onKeydown(e: KeyboardEvent) {
		if (e.key !== 'Tab') return;
		const items = focusables();
		if (items.length === 0) return;
		const first = items[0];
		const last = items[items.length - 1];
		if (e.shiftKey && document.activeElement === first) {
			e.preventDefault();
			last.focus();
		} else if (!e.shiftKey && document.activeElement === last) {
			e.preventDefault();
			first.focus();
		}
	}
	node.addEventListener('keydown', onKeydown);
	return {
		destroy() {
			node.removeEventListener('keydown', onKeydown);
		}
	};
}
