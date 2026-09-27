<script lang="ts">
	import { pendingPackage, pushToast } from '$lib/stores';
	import { cmd } from '$lib/tauri';
	import { trapFocus } from '$lib/actions';

	$: pkg = $pendingPackage;

	async function apply() {
		const msg = await cmd<string>('apply_pending_package');
		pendingPackage.set(null);
		pushToast(msg ?? 'Package applied', 'ok');
	}

	async function cancel() {
		await cmd('cancel_pending_package');
		pendingPackage.set(null);
		pushToast('Install cancelled', 'info');
	}
</script>

{#if pkg}
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="backdrop u-scrim" role="presentation" on:mousedown={(e) => e.target === e.currentTarget && cancel()}>
		<div class="dialog glass-strong" use:trapFocus role="dialog" aria-label="Review package before applying">
			<header>
				<h2>Review package</h2>
				<span class="badge kind">{pkg.kind}</span>
			</header>

			<div class="body">
				<div class="grid">
					<span class="dim">Name</span><strong>{pkg.name}</strong>
					<span class="dim">Version</span><span>{pkg.version}</span>
					<span class="dim">Interface</span><span>v{pkg.iface_version}</span>
				</div>

				<h3>Capabilities</h3>
				<div class="caps">
				<span class="cap" class:granted={pkg.capabilities?.fs}>fs {pkg.capabilities?.fs ? '✓' : '✗'}</span>
				<span class="cap" class:granted={pkg.capabilities?.net}>net {pkg.capabilities?.net ? '✓' : '✗'}</span>
				<span class="cap" class:granted={pkg.capabilities?.ui}>ui {pkg.capabilities?.ui ? '✓' : '✗'}</span>
				</div>

				<h3>Tokens introduced</h3>
				{#if (pkg.token_keys ?? []).length === 0}<p class="dim small">none</p>{:else}
					<p class="mono">{pkg.token_keys.join(', ')}</p>
				{/if}

				<h3>Contents</h3>
				<p class="small">
					{pkg.components?.length ?? 0} components ·
					{pkg.scripts?.length ?? 0} scripts ·
					{pkg.assets?.length ?? 0} assets
				</p>
				{#if (pkg.components ?? []).length}
					<ul class="files mono">{#each pkg.components as c (c)}<li>{c}</li>{/each}</ul>
				{/if}

				{#if (pkg.warnings ?? []).length}
					<h3 class="warn">Warnings</h3>
					<ul class="warnings">
						{#each pkg.warnings as w (w)}<li>{w}</li>{/each}
					</ul>
				{/if}
			</div>

			<footer>
				<button class="btn ghost" on:click={() => void cancel()}>Cancel</button>
				<button class="btn primary" on:click={() => void apply()}>Apply</button>
			</footer>
		</div>
	</div>
{/if}

<svelte:window on:keydown={(e) => pkg && e.key === 'Escape' && void cancel()} />

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		z-index: 1000;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.dialog {
		width: 460px;
		max-width: 92vw;
		max-height: 84vh;
		display: flex;
		flex-direction: column;
		border-radius: var(--umbra-radius);
		animation: pop 0.18s cubic-bezier(0.22, 1, 0.36, 1);
	}
	@keyframes pop {
		from {
			transform: scale(0.96);
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
		padding: 14px 18px 8px;
	}
	h2 {
		font-size: 15px;
		margin: 0;
		flex: 1;
	}
	h3 {
		font-size: 11px;
		margin: 14px 0 6px;
		text-transform: uppercase;
		letter-spacing: 2px;
		color: var(--umbra-color-accent);
	}
	h3.warn {
		color: var(--umbra-color-danger);
	}
	.body {
		padding: 0 18px 14px;
		overflow-y: auto;
		font-size: 13px;
	}
	.grid {
		display: grid;
		grid-template-columns: 90px 1fr;
		gap: 4px 12px;
	}
	.caps {
		display: flex;
		gap: 8px;
	}
	.cap {
		font-family: var(--umbra-font-mono);
		font-size: 11px;
		padding: 2px 8px;
		border-radius: 999px;
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 80%, transparent);
		color: var(--umbra-color-fg-dim);
	}
	.cap.granted {
		border-color: color-mix(in srgb, var(--umbra-color-danger) 60%, transparent);
		color: var(--umbra-color-danger);
	}
	.badge.kind {
		font-size: 10px;
		font-family: var(--umbra-font-mono);
		padding: 1px 8px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--umbra-color-accent) 15%, transparent);
		color: var(--umbra-color-accent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-accent) 45%, transparent);
	}
	.mono {
		font-family: var(--umbra-font-mono);
		word-break: break-all;
	}
	.files,
	.warnings {
		margin: 0;
		padding-left: 18px;
		font-size: 12px;
		max-height: 90px;
		overflow-y: auto;
	}
	.warnings li {
		color: color-mix(in srgb, var(--umbra-color-danger) 85%, white);
	}
	p {
		margin: 0;
	}
	footer {
		display: flex;
		justify-content: flex-end;
		gap: 10px;
		padding: 12px 18px;
		border-top: 1px solid color-mix(in srgb, var(--umbra-color-border) 50%, transparent);
	}
	.btn {
		all: unset;
		cursor: default;
		height: 30px;
		padding: 0 16px;
		font-size: 13px;
		display: inline-flex;
		align-items: center;
		border-radius: var(--umbra-radius-small);
		border: 1px solid transparent;
	}
	.btn.primary {
		background: var(--umbra-color-accent);
		color: var(--umbra-color-accent-fg);
	}
	.btn.ghost {
		border-color: color-mix(in srgb, var(--umbra-color-border) 90%, transparent);
	}
</style>
