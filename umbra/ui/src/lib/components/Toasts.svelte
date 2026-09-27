<script lang="ts">
	import gsap from 'gsap';
	import { toasts, dismissToast } from '$lib/stores';
	import type { Toast } from '$lib/types';

	function enter(node: HTMLElement) {
		gsap.fromTo(
			node,
			{ opacity: 0, x: 40 },
			{ opacity: 1, x: 0, duration: 0.25, ease: 'power3.out' }
		);
		return { duration: 250 };
	}
	function leave(node: HTMLElement) {
		gsap.to(node, { opacity: 0, y: 8, duration: 0.18, ease: 'power1.in' });
		return { duration: 190 };
	}

	function keyOf(t: Toast): string {
		return `${t.kind}-${t.id}`;
	}
</script>

<div class="toasts" aria-live="polite">
	{#each $toasts as t (t.id)}
		<div
			class="toast"
			class:error={t.kind === 'error'}
			class:ok={t.kind === 'ok'}
			data-kind={keyOf(t)}
			in:enter
			out:leave
		>
			{t.message}
		</div>
	{/each}
</div>

<style>
	.toasts {
		position: fixed;
		right: 16px;
		bottom: 40px;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 8px;
		z-index: 1100;
		pointer-events: none;
		max-width: min(420px, 80vw);
	}
	.toast {
		padding: 10px 18px;
		font-size: 13px;
		border-radius: var(--umbra-radius-small);
		background: color-mix(in srgb, var(--umbra-color-surface) 92%, transparent);
		backdrop-filter: blur(var(--umbra-blur));
		border: 1px solid var(--umbra-color-border);
		box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
		word-break: break-word;
	}
	.toast.error {
		border-color: color-mix(in srgb, var(--umbra-color-danger) 60%, transparent);
		color: var(--umbra-color-danger);
	}
	.toast.ok {
		border-color: color-mix(in srgb, var(--umbra-color-ok) 60%, transparent);
		color: var(--umbra-color-ok);
	}
</style>
