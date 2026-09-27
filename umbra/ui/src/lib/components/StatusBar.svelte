<script lang="ts">
	import { ui } from '$lib/stores';

	$: safe = $ui.safe;
	$: theme = $ui.themePreset ?? $ui.theme ?? 'default';
</script>

<div class="status-bar" role="status">
	<span class="left" class:safe>
		{#if safe}
			SAFE MODE: config &amp; plugins disabled
		{:else}
			layout: {$ui.layout} · theme: {theme}/{$ui.mode}
			{$ui.split ? ' · ⧉ split view' : ''}
		{/if}
	</span>
	<span class="right dim">umbra 0.1</span>
</div>

<style>
	.status-bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		min-height: 26px;
		padding: 2px 12px;
		font-size: 11.5px;
		font-family: var(--umbra-font-mono);
	}
	.left.safe {
		color: var(--umbra-color-danger);
	}
</style>
