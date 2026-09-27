<script lang="ts">
	import { ui } from '$lib/stores';
	import { cmd } from '$lib/tauri';

	$: minimal = $ui.layout === 'minimal';
</script>

{#if !minimal}
	<nav class="nav-btns" aria-label="Navigation">
		<button class="icon-btn" title="Back" aria-label="Back" on:click={() => void cmd('go_back')}>←</button>
		<button class="icon-btn" title="Forward" aria-label="Forward" on:click={() => void cmd('go_forward')}>→</button>
		<button class="icon-btn" title="Reload" aria-label="Reload" on:click={() => void cmd('reload_page')}>⟳</button>
		<button class="icon-btn" title="New Tab" aria-label="New Tab" on:click={() => void cmd('run_action', { action: 'new_tab'})}>＋</button>
	</nav>
{/if}

<style>
	.nav-btns {
		display: flex;
		gap: 2px;
	}
</style>
