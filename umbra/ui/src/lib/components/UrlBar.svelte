<script lang="ts">
	import { activeTab, focusUrlTick } from '$lib/stores';
	import { cmd } from '$lib/tauri';

	let value = $state('');
	let inputEl = $state<HTMLInputElement | undefined>();
	let focused = $state(false);

	// Mirror the active tab URL into the input when not focused.
	$effect(() => {
		if (!focused && $activeTab) {
			value = $activeTab.url === 'umbra://newtab' ? '' : $activeTab.url;
		}
	});

	// focus_url action (from keybindings / palette)
	$effect(() => {
		if ($focusUrlTick > 0 && inputEl) {
			inputEl.focus();
			inputEl.select();
		}
	});

	function submit() {
		void cmd('navigate', { url: value });
		value = '';
		inputEl?.blur();
		focused = false;
	}
</script>

<div class="urlwrap" class:focused>
	<span
		class="lock"
		class:secure={$activeTab?.url.startsWith('https://')}
		aria-hidden="true"
		title={$activeTab?.url.startsWith('https://') ? 'Secure connection' : 'Not secure'}
	>
		{#if $activeTab?.url.startsWith('https://')}
			<svg class="lock-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
				<rect x="3.5" y="7" width="9" height="6.5" rx="1.5" />
				<path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
				<circle cx="8" cy="10.25" r="1" fill="currentColor" stroke="none" />
			</svg>
		{:else}
			<svg class="lock-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
				<rect x="3.5" y="7" width="9" height="6.5" rx="1.5" />
				<path d="M5.5 7V5a2.5 2.5 0 0 1 4.9-.6" />
				<circle cx="8" cy="10.25" r="1" fill="currentColor" stroke="none" />
			</svg>
		{/if}
	</span>
	<input
		bind:this={inputEl}
		bind:value
		type="text"
		spellcheck="false"
		placeholder="Search or enter address"
		aria-label="Address bar"
		on:focus={() => (focused = true)}
		on:blur={() => (focused = false)}
		on:keydown={(e) => e.key === 'Enter' && submit()}
	/>
</div>

<style>
	.urlwrap {
		display: flex;
		align-items: center;
		gap: 8px;
		flex: 0 1 460px;
		min-width: 180px;
		height: 34px;
		padding: 0 12px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--umbra-color-fg) 7%, transparent);
		border: 1px solid transparent;
		transition:
			border-color var(--umbra-duration) var(--umbra-ease),
			box-shadow var(--umbra-duration) var(--umbra-ease);
	}
	.urlwrap.focused,
	.urlwrap:focus-within {
		border-color: var(--umbra-color-accent);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--umbra-color-accent) 22%, transparent);
	}
	.lock {
		display: inline-flex;
		align-items: center;
		color: var(--umbra-color-fg-dim);
	}
	.lock-icon {
		width: 13px;
		height: 13px;
		flex-shrink: 0;
	}
	.lock.secure {
		color: var(--umbra-color-ok);
	}
	input {
		all: unset;
		flex: 1;
		height: 100%;
		font-size: 13px;
		color: var(--umbra-color-fg);
		min-width: 0;
	}
	input::placeholder {
		color: var(--umbra-color-fg-dim);
	}
</style>
