<script lang="ts">
	import { consoleLines, consoleOpen, ui } from '$lib/stores';
	import { trapFocus } from '$lib/actions';

	let bodyEl = $state<HTMLDivElement | undefined>();

	$effect(() => {
		if ($consoleOpen && bodyEl) {
			bodyEl.scrollTop = bodyEl.scrollHeight;
		}
	});

	function close() {
		consoleOpen.set(false);
	}
</script>

<svelte:window on:keydown={(e) => $consoleOpen && e.key === 'Escape' && close()} />

{#if $consoleOpen}
	<aside class="console glass-strong" use:trapFocus role="dialog" aria-label="Plugin console">
		<header>
			<h2>Plugin console</h2>
			<span class="dim small">{$ui.plugins?.length ?? 0} plugins · {$consoleLines.length} lines</span>
			<span class="spacer"></span>
			<button class="icon-btn" aria-label="Clear console" title="Clear" on:click={() => consoleLines.set([])}>⌫</button>
			<button class="icon-btn" aria-label="Close console" title="Close (Esc)" on:click={close}>×</button>
		</header>
		<div class="log mono" bind:this={bodyEl} aria-live="polite">
			{#if ($ui.plugins ?? []).some((p) => p.errors)}
				{#each ($ui.plugins ?? []).filter((p) => p.errors) as p (p.name)}
					<div class="line error">[plugin:{p.name}] {Array.isArray(p.errors) ? p.errors.join(' | ') : String(p.errors)}</div>
				{/each}
			{/if}
			{#each $consoleLines as l, i (l.ts + '-' + i)}
				<div class="line" class:error={l.level === 'error'} class:warn={l.level === 'warn'}>
					<span class="ts">{new Date(l.ts).toLocaleTimeString()}</span>
					[{l.level}] {l.message}
				</div>
			{:else}
				<div class="line dim">console empty</div>
			{/each}
		</div>
	</aside>
{/if}

<style>
	.console {
		position: fixed;
		left: 0;
		right: 0;
		bottom: 0;
		z-index: 940;
		height: 38vh;
		min-height: 220px;
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--umbra-color-border);
		animation: rise 0.2s cubic-bezier(0.22, 1, 0.36, 1);
	}
	@keyframes rise {
		from {
			transform: translateY(24px);
			opacity: 0;
		}
		to {
			transform: none;
			opacity: 1;
		}
	}
	header {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 12px;
		border-bottom: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
	}
	h2 {
		font-size: 13px;
		margin: 0;
		letter-spacing: 1px;
	}
	.spacer {
		flex: 1;
	}
	.log {
		flex: 1;
		overflow-y: auto;
		padding: 8px 12px;
		font-size: 11.5px;
		line-height: 1.55;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.mono {
		font-family: var(--umbra-font-mono);
	}
	.ts {
		color: var(--umbra-color-fg-dim);
		margin-right: 6px;
	}
	.line.warn {
		color: color-mix(in srgb, var(--umbra-color-accent) 70%, white);
	}
	.line.error {
		color: var(--umbra-color-danger);
	}
</style>
