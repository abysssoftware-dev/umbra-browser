<script lang="ts">
	import { ui } from '$lib/stores';
	import { mountedFor } from '$lib/slots';
	import { logLine } from '$lib/stores';
	import type { Component } from 'svelte';
	import TabStrip from './TabStrip.svelte';
	import UrlBar from './UrlBar.svelte';
	import NavButtons from './NavButtons.svelte';
	import BookmarksBar from './BookmarksBar.svelte';
	import StatusBar from './StatusBar.svelte';

	/**
	 * Generic slot host: renders the component names registered for `region`
	 * in UiState.mounted. Unknown component names render nothing and are
	 * logged once to the plugin console.
	 */
	export let region: string;

	const registry: Record<string, Component> = {
		'tab-strip': TabStrip,
		'url-bar': UrlBar,
		'nav-buttons': NavButtons,
		'bookmarks-bar': BookmarksBar,
		'status-bar': StatusBar
	};
	const warned = new Set<string>();

	$: names = (mountedFor($ui)[region] ?? []).filter(Boolean);
</script>

{#each names as name (name)}
	{@const C = registry[name]}
	{#if C}
		<span class="slot-item" data-component={name}>
			{#if name === 'tab-strip'}
				<C vertical={region === 'sidebar-left'} />
			{:else}
				<C />
			{/if}
		</span>
	{:else if !warned.has(name)}
		{@const _ignored = (warned.add(name), logLine('warn', `[slots] unknown component '${name}' in region '${region}' - skipped`))}
	{/if}
{/each}

<style>
	.slot-item {
		display: contents;
	}
</style>
