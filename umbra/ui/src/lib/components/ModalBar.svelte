<script lang="ts">
	import { ui, applyState, pushToast, logLine } from '$lib/stores';
	import { cmd } from '$lib/tauri';

	let armed = $state(false);
	let cmdline = $state('');
	let inputEl = $state<HTMLInputElement | undefined>();

	const modalMode = $derived($ui.flags.modal_mode === true);

	function onWindowKeydown(e: KeyboardEvent) {
		if (!modalMode) {
			armed = false;
			return;
		}
		if (e.key === 'Escape' && !armed) {
			armed = true;
			cmdline = ':';
			setTimeout(() => inputEl?.focus(), 0);
		} else if (e.key === 'Escape' && armed) {
			armed = false;
		}
	}

	async function exec(e: SubmitEvent) {
		e.preventDefault();
		const raw = cmdline.trim();
		if (!raw.startsWith(':') || raw.length < 2) return;
		const body = raw.slice(1).trim();
		const [verb, ...rest] = body.split(/\s+/);
		const arg = rest.join(' ');

		switch (verb) {
			case 'q':
			case 'quit': {
				const t = $ui.tabs.find((x) => x.active);
				if (t) await cmd('close_tab', { id: t.id });
				break;
			}
			case 'tabnew':
				await cmd('run_action', { action: 'new_tab' });
				break;
			case 'theme':
				if (!arg) pushToast(':theme <preset>', 'error');
				else {
					await cmd('set_theme_preset', { preset: arg });
					applyState({ themePreset: arg });
					pushToast(`theme → ${arg}`, 'ok');
				}
				break;
			case 'layout':
				if (!arg) pushToast(':layout <preset>', 'error');
				else {
					await cmd('set_layout', { preset: arg });
					pushToast(`layout → ${arg}`, 'ok');
				}
				break;
			case 'wq':
				pushToast(':wq: nothing to save (noop)', 'info');
				break;
			case 'help':
				logLine('info', 'modal commands: :q · :tabnew · :theme X · :layout X · :wq (noop) · :help');
				pushToast('see plugin console for command list', 'ok');
				break;
			default:
				pushToast(`unknown command: ${verb}`, 'error');
		}
		cmdline = ':';
		inputEl?.focus();
	}
</script>

<svelte:window on:keydown={onWindowKeydown} />

{#if modalMode && armed}
	<form class="cmdline glass-strong" on:submit={exec}>
		<input
			bind:this={inputEl}
			bind:value={cmdline}
			spellcheck="false"
			autocomplete="off"
			aria-label="Modal command line"
			on:blur={() => setTimeout(() => { if (!cmdline || cmdline === ':') armed = false; else inputEl?.focus(); }, 80)}
		/>
		<span class="dim small hint">:help for commands</span>
	</form>
{:else if modalMode}
	<div class="armed-hint dim small" aria-hidden="true">…</div>
{/if}

<style>
	.cmdline {
		position: fixed;
		left: 0;
		right: 0;
		bottom: 0;
		z-index: 960;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 14px;
		border-top: 1px solid var(--umbra-color-border);
		animation: rise 0.12s ease-out;
	}
	@keyframes rise {
		from {
			transform: translateY(100%);
		}
		to {
			transform: none;
		}
	}
	input {
		all: unset;
		flex: 1;
		font-family: var(--umbra-font-mono);
		font-size: 13px;
		color: var(--umbra-color-fg);
		height: 24px;
	}
	.hint {
		flex-shrink: 0;
	}
	.armed-hint {
		position: fixed;
		left: 6px;
		bottom: 4px;
		z-index: 955;
		font-family: var(--umbra-font-mono);
		pointer-events: none;
	}
</style>
