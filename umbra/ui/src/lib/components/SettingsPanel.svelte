<script lang="ts">
	import {
		ui,
		applyState,
		settingsOpen,
		settingsSection,
		pushToast,
		pendingPackage,
		tutorialOpen,
		type SettingsSection
	} from '$lib/stores';
	import { cmd } from '$lib/tauri';
	import { comboOf, KNOWN_ACTIONS } from '$lib/keybinds';
	import type {
		GalleryItem,
		ImportResult,
		ImportSource,
		PkgDiff,
		SyncStatus
	} from '$lib/types';

	const VERSION = '0.1.0';

	const SECTIONS: Array<{ id: SettingsSection; label: string }> = [
		{ id: 'appearance', label: 'Appearance' },
		{ id: 'layout', label: 'Layout' },
		{ id: 'newtab', label: 'New tab page' },
		{ id: 'keybindings', label: 'Keybindings' },
		{ id: 'flags', label: 'Flags' },
		{ id: 'import', label: 'Import' },
		{ id: 'sync', label: 'Sync' },
		{ id: 'plugins', label: 'Plugins' },
		{ id: 'config', label: 'Config files' },
		{ id: 'about', label: 'About & Help' }
	];

	const layouts = $derived($ui.layouts ?? ['compact', 'minimal', 'sidebar', 'tiling', 'floating', 'zen']);
	const presets = $derived($ui.presets ?? ['glass', 'material', 'gruvbox', 'nord', 'catppuccin', 'ocean']);

	const FLAGS: Array<[string, string]> = [
		['adblock', 'Ad / tracker blocker'],
		['pip', 'Picture-in-picture'],
		['split_view', 'Split view'],
		['modal_mode', 'Modal (vim) mode'],
		['contextual_hints', 'Contextual hints'],
		['notify_reload', 'Show notification when config reloads']
	];

	// ── tokens editor ────────────────────────────────────────────────────────
	/** Editable token definitions for the custom theme preset. */
	const TOKEN_DEFS: Array<{ key: string; label: string; kind: 'color' | 'text' }> = [
		{ key: 'color.accent', label: 'Accent', kind: 'color' },
		{ key: 'color.accent.fg', label: 'Accent text', kind: 'color' },
		{ key: 'color.bg', label: 'Background', kind: 'color' },
		{ key: 'color.surface', label: 'Surface', kind: 'color' },
		{ key: 'color.fg', label: 'Text', kind: 'color' },
		{ key: 'color.fg.dim', label: 'Text (dim)', kind: 'color' },
		{ key: 'color.border', label: 'Border', kind: 'color' },
		{ key: 'color.danger', label: 'Danger', kind: 'color' },
		{ key: 'color.ok', label: 'Success', kind: 'color' },
		{ key: 'radius', label: 'Corner radius', kind: 'text' },
		{ key: 'blur', label: 'Blur strength', kind: 'text' }
	];
	let tokValues = $state<Record<string, string>>({});
	let customPreset = $derived(($ui.themePreset ?? $ui.theme ?? 'glass') === 'custom');

	function syncTokValues() {
		tokValues = { ...($ui.themeTokens ?? {}) };
	}
	async function applyToken(key: string) {
		const value = tokValues[key] ?? '';
		await cmd('set_token', { key, value: value.trim() });
		pushToast(`${key} → ${value.trim() || '(theme default)'}`, 'ok');
	}
	async function resetTokens() {
		await cmd('reset_tokens');
		syncTokValues();
		pushToast('Tokens reset to theme defaults', 'ok');
	}

	// ── keybindings editor ───────────────────────────────────────────────────
	let captureFor = $state<string | null>(null); // action awaiting a combo
	let newAction = $state(KNOWN_ACTIONS[0]);

	function startCapture(action?: string) {
		captureFor = action ?? newAction;
		window.addEventListener('keydown', onCapture, { capture: true, once: true });
	}
	async function onCapture(e: KeyboardEvent) {
		e.preventDefault();
		e.stopPropagation();
		const combo = comboOf(e);
		const action = captureFor;
		captureFor = null;
		if (!action || e.key === 'Escape') return;
		await cmd('set_keybinding', { combo, action });
		applyState({ keybindings: { ...$ui.keybindings, [combo]: action } });
		pushToast(`${combo} → ${action}`, 'ok');
	}
	async function clearBinding(combo: string) {
		await cmd('set_keybinding', { combo, action: '' });
		const kb2 = { ...$ui.keybindings };
		delete kb2[combo];
		applyState({ keybindings: kb2 });
	}

	// ── flags ────────────────────────────────────────────────────────────────
	async function toggleFlag(key: string, value: boolean) {
		await cmd('set_flag', { key, value });
		applyState({ flags: { ...$ui.flags, [key]: value } });
	}

	// ── packages / gallery ───────────────────────────────────────────────────
	let galleryItems = $state<GalleryItem[] | null>(null);
	let galleryLoading = $state(false);
	let pkgSource = $state('');
	let installing = $state('');

	async function browseGallery() {
		galleryLoading = true;
		const items = await cmd<GalleryItem[]>('gallery_list');
		galleryLoading = false;
		galleryItems = items ?? null;
		if (!items) pushToast('Gallery unavailable', 'error');
	}
	async function installFromGallery(repo: string) {
		installing = repo;
		const diff = await cmd<PkgDiff>('gallery_install', { repo });
		installing = '';
		if (diff) pendingPackage.set(diff);
	}
	async function installFromSource() {
		const src = pkgSource.trim();
		if (!src) return;
		installing = src;
		const diff = await cmd<PkgDiff>('install_package', { source: src });
		installing = '';
		if (diff) pendingPackage.set(diff);
	}

	const pluginList = $derived($ui.plugins ?? []);

	// ── browser import ───────────────────────────────────────────────────────
	let sources = $state<ImportSource[] | null>(null);
	let detecting = $state(false);
	let importingKey = $state('');
	let selBookmarks = $state<Record<string, boolean>>({});
	let selHistory = $state<Record<string, boolean>>({});
	const srcKey = (s: ImportSource) => `${s.browser}|${s.profile}`;

	async function detectBrowsers() {
		detecting = true;
		const r = await cmd<ImportSource[]>('import_detect');
		detecting = false;
		sources = r ?? [];
		selBookmarks = {};
		selHistory = {};
		for (const s of sources) {
			if (s.has_bookmarks) selBookmarks[srcKey(s)] = true;
			if (s.has_history) selHistory[srcKey(s)] = true;
		}
	}

	async function runImport(s: ImportSource) {
		importingKey = srcKey(s);
		// cmd() already surfaces backend errors as an error toast.
		const r = await cmd<ImportResult>('import_browser', {
			browser: s.browser,
			profile: s.profile,
			bookmarks: selBookmarks[srcKey(s)] === true,
			history: selHistory[srcKey(s)] === true
		});
		importingKey = '';
		if (r)
			pushToast(
				`Imported ${r.bookmarks_imported} bookmarks, ${r.history_imported} history entries`,
				'ok'
			);
	}

	// ── config sync (GitHub gist) ────────────────────────────────────────────
	let syncSt = $state<SyncStatus | null>(null);
	let syncToken = $state('');
	let syncGist = $state('');
	let syncBusy = $state('');

	async function loadSync() {
		const s = await cmd<SyncStatus>('sync_status');
		if (s) syncSt = s;
	}
	async function saveSync() {
		syncBusy = 'save';
		const s = await cmd<SyncStatus>('sync_configure', {
			token: syncToken.trim(),
			gist_id: syncGist.trim()
		});
		syncBusy = '';
		if (s) {
			syncSt = s;
			syncToken = '';
			syncGist = '';
			pushToast('Sync settings saved', 'ok');
		}
	}
	async function syncPush() {
		syncBusy = 'push';
		const msg = await cmd<string>('sync_push');
		syncBusy = '';
		if (msg !== undefined) pushToast(msg || 'Config pushed', 'ok');
		void loadSync();
	}
	async function syncPull() {
		syncBusy = 'pull';
		const msg = await cmd<string>('sync_pull');
		syncBusy = '';
		if (msg !== undefined) pushToast(msg || 'Config pulled', 'ok');
		void loadSync();
	}

	// ── New tab page ─────────────────────────────────────────────────────────
	let ntUrl = $state('');
	let ntBackground = $state('');
	let ntGreeting = $state('');
	let ntClock = $state(true);
	let ntLoaded = $state(false);

	function loadNewtab() {
		const nt = $ui.newtab;
		if (!nt) return;
		ntUrl = nt.url ?? '';
		ntBackground = nt.background ?? '';
		ntGreeting = nt.greeting ?? '';
		ntClock = nt.show_clock !== false;
		ntLoaded = true;
	}
	async function saveNewtab() {
		await cmd('set_newtab', {
			url: ntUrl.trim(),
			background: ntBackground.trim(),
			greeting: ntGreeting,
			show_clock: ntClock
		});
		pushToast('New tab page updated', 'ok');
	}

	// ── Config-file editors (every TOML under the config dir) ────────────────
	interface ConfigFileInfo {
		path: string;
		size: number;
	}
	let cfgFiles = $state<ConfigFileInfo[]>([]);
	let cfgSelected = $state('');
	let cfgContent = $state('');
	let cfgOriginal = $state('');
	let cfgBusy = $state(false);
	const cfgDirty = $derived(cfgContent !== cfgOriginal);

	async function loadCfgList() {
		const files = await cmd<ConfigFileInfo[]>('config_list_files');
		cfgFiles = files ?? [];
		if (!cfgSelected && cfgFiles.length) void selectCfgFile(cfgFiles[0].path);
	}
	async function selectCfgFile(path: string) {
		if (cfgDirty && !confirm(`Discard unsaved changes to ${cfgSelected}?`)) return;
		const content = await cmd<string>('config_read_file', { path });
		if (content === undefined) return;
		cfgSelected = path;
		cfgContent = content;
		cfgOriginal = content;
	}
	async function saveCfgFile() {
		if (!cfgSelected || !cfgDirty) return;
		cfgBusy = true;
		const err = await cmd<string>('config_write_file', { path: cfgSelected, content: cfgContent });
		cfgBusy = false;
		if (err === undefined) {
			pushToast(`Failed to save ${cfgSelected} - see console`, 'error');
			return;
		}
		cfgOriginal = cfgContent;
		pushToast(
			cfgSelected === 'umbra.toml' ? 'umbra.toml saved & applied' : `${cfgSelected} saved`,
			'ok'
		);
	}

	let lastAppearanceSync = false;
	let cfgListLoaded = false;
	$effect(() => {
		if ($settingsOpen && !syncSt) void loadSync();
		if ($settingsOpen && $settingsSection === 'newtab' && !ntLoaded) loadNewtab();
		if ($settingsOpen && $settingsSection === 'config' && !cfgListLoaded) {
			cfgListLoaded = true;
			void loadCfgList();
		}
	});
	// Sync the token editor when the Appearance pane is (re)entered - not on
	// every state push, which would clobber in-progress edits.
	$effect(() => {
		const on = $settingsOpen && $settingsSection === 'appearance';
		if (on && !lastAppearanceSync) syncTokValues();
		lastAppearanceSync = on;
	});

	async function close() {
		settingsOpen.set(false);
	}

	function onWindowKeydown(e: KeyboardEvent) {
		if ($settingsOpen && e.key === 'Escape' && captureFor === null) void close();
	}
</script>

<svelte:window on:keydown={onWindowKeydown} />

{#if $settingsOpen}
	<div class="page" role="region" aria-label="Settings">
		<aside class="rail">
			<button class="icon-btn back" aria-label="Back to browsing" title="Back (Esc)" on:click={close}>←</button>
			<h2>Settings</h2>
			<nav aria-label="Settings sections">
				{#each SECTIONS as s (s.id)}
					<button
						class="rail-item"
						class:active={$settingsSection === s.id}
						on:click={() => settingsSection.set(s.id)}
					>
						{s.label}
					</button>
				{/each}
			</nav>
		</aside>

		<div class="pane">
			{#if $settingsSection === 'appearance'}
				<section>
					<h3>Theme preset</h3>
					<div class="preset-grid">
						{#each presets as p (p)}
							{@const active = ($ui.themePreset ?? $ui.theme ?? 'glass') === p}
							<button
								class="preset-chip"
								class:active
								on:click={() => void cmd('set_theme_preset', { preset: p })}
							>
								<span class="color-dot dot-{p}"></span>
								<span class="p-label">{p}</span>
								{#if active}<span class="active-badge">✓</span>{/if}
							</button>
						{/each}
						<button
							class="preset-chip"
							class:active={customPreset}
							on:click={() => void cmd('set_theme_preset', { preset: 'custom' })}
						>
							<span class="color-dot dot-custom"></span>
							<span class="p-label">custom</span>
							{#if customPreset}<span class="active-badge">✓</span>{/if}
						</button>
					</div>
					<div class="row" style="margin-top: 8px;">
						<span>Mode</span>
						<button class="btn" on:click={() => void cmd('run_action', { action: 'toggle_mode' })}>
							{$ui.mode} (click to toggle)
						</button>
					</div>
				</section>

				{#if customPreset}
					<section>
						<h3>Custom theme</h3>
						<p class="dim small">
							Custom preset is active - tweak tokens right here, no config editing needed.
							Empty = inherit from the built-in palette.
						</p>
						<div class="token-grid">
							{#each TOKEN_DEFS as t (t.key)}
								{@const overridden = ($ui.themeTokens ?? {})[t.key] ?? ''}
								<label class="token-row">
									<span class="t-label">{t.label}</span>
									{#if t.kind === 'color'}
										<input
											type="color"
											class="color-swatch"
											value={overridden || $ui.resolvedTokens?.[t.key] || '#000000'}
											on:input={(e) => {
												tokValues[t.key] = e.currentTarget.value;
												void applyToken(t.key);
											}}
											aria-label={`${t.label} color picker`}
										/>
									{/if}
									<input
										class="t-value"
										placeholder={$ui.resolvedTokens?.[t.key] ?? t.key}
										bind:value={tokValues[t.key]}
										spellcheck="false"
										on:change={() => void applyToken(t.key)}
										on:keydown={(e) => e.key === 'Enter' && void applyToken(t.key)}
										aria-label={t.label}
									/>
									{#if overridden}
										<button
											class="mini danger"
											title="Clear override"
											on:click={() => {
												tokValues[t.key] = '';
												void applyToken(t.key);
											}}>×</button
										>
									{/if}
								</label>
							{/each}
						</div>
						<div class="row">
							<button class="btn ghost" on:click={() => void resetTokens()}>Reset all to theme defaults</button>
						</div>
					</section>
				{/if}
			{:else if $settingsSection === 'layout'}
				<section>
					<h3>Layout preset</h3>
					<div class="layout-grid">
						{#each layouts as l (l)}
							{@const active = ($ui.layout ?? 'compact') === l}
							<button
								class="layout-chip"
								class:active
								on:click={() => void cmd('set_layout', { preset: l })}
							>
								<span class="p-label">{l}</span>
								{#if active}<span class="active-badge">✓</span>{/if}
							</button>
						{/each}
					</div>
					<p class="dim small" style="margin-top: 6px;">Sidebar layout moves tabs into a vertical strip; tiling grids your tabs; zen hides chrome until hover.</p>
				</section>
			{:else if $settingsSection === 'keybindings'}
				<section>
					<h3>Keybindings</h3>
					<ul class="kb-list">
						{#each Object.entries($ui.keybindings).filter(([, a]) => a) as [combo, action] (combo)}
							<li>
								<code>{combo}</code><span class="dim">→ {action}</span>
								<span class="spacer"></span>
								<button class="mini" title="Rebind" on:click={() => startCapture(action)}>rebind</button>
								<button class="mini danger" title="Remove binding" on:click={() => void clearBinding(combo)}>×</button>
							</li>
						{/each}
					</ul>
					<div class="row">
						<select bind:value={newAction} aria-label="Action">
							{#each KNOWN_ACTIONS as a}<option value={a}>{a}</option>{/each}
						</select>
						{#if captureFor === null}
							<button class="btn" on:click={() => startCapture()}>Add binding…</button>
						{:else}
							<span class="capturing">press keys… (Esc cancels)</span>
						{/if}
					</div>
				</section>
			{:else if $settingsSection === 'flags'}
				<section>
					<h3>Flags</h3>
					{#each FLAGS as [flag, label] (flag)}
						<label class="row toggle">
							<span>{label}</span>
							<input
								type="checkbox"
								checked={$ui.flags[flag] ?? false}
								on:change={(e) => void toggleFlag(flag, e.currentTarget.checked)}
							/>
						</label>
					{/each}
				</section>
			{:else if $settingsSection === 'import'}
				<section>
					<h3>Import</h3>
					<p class="dim small">Bring bookmarks and history over from another browser. Passwords are never imported.</p>
					<button class="btn" disabled={detecting} on:click={() => void detectBrowsers()}>
						{detecting ? 'detecting…' : 'Detect installed browsers'}
					</button>
					{#if sources}
						{#if sources.length === 0}
							<p class="dim small">No supported browser profiles found.</p>
						{:else}
							<ul class="import-list">
								{#each sources as s (srcKey(s))}
									<li>
										<div class="i-head">
											<strong>{s.browser}</strong>
											<span class="dim small">{s.profile}</span>
										</div>
										<div class="row checks">
											<label class="check">
												<input
													type="checkbox"
													disabled={!s.has_bookmarks}
													checked={selBookmarks[srcKey(s)] === true && s.has_bookmarks}
													on:change={(e) => (selBookmarks[srcKey(s)] = e.currentTarget.checked)}
												/>
												<span>Bookmarks{s.has_bookmarks ? '' : ' (none found)'}</span>
											</label>
											<label class="check">
												<input
													type="checkbox"
													disabled={!s.has_history}
													checked={selHistory[srcKey(s)] === true && s.has_history}
													on:change={(e) => (selHistory[srcKey(s)] = e.currentTarget.checked)}
												/>
												<span>History{s.has_history ? '' : ' (none found)'}</span>
											</label>
										</div>
										<button
											class="btn"
											disabled={!!importingKey || (!s.has_bookmarks && !s.has_history)}
											on:click={() => void runImport(s)}
										>
											{importingKey === srcKey(s) ? 'importing…' : 'Import selected'}
										</button>
									</li>
								{/each}
							</ul>
						{/if}
					{/if}
				</section>
			{:else if $settingsSection === 'sync'}
				<section>
					<h3>Config sync (GitHub gist)</h3>
					<p class="dim small">
						{syncSt
							? `${syncSt.has_token ? 'Token set' : 'No token'} · gist ${syncSt.gist_id ?? 'none'}${syncSt.last_error ? ` · last error: ${syncSt.last_error}` : ''}`
							: 'Loading sync status…'}
					</p>
					<div class="row">
						<span>Token</span>
						<input
							type="password"
							placeholder="Personal access token (empty = clear)"
							bind:value={syncToken}
							aria-label="GitHub personal access token"
							autocomplete="off"
						/>
					</div>
					<div class="row">
						<span>Gist ID</span>
						<input
							placeholder="Optional: existing gist ID"
							bind:value={syncGist}
							aria-label="Gist id"
						/>
					</div>
					<div class="row">
						<button class="btn" disabled={syncBusy === 'save'} on:click={() => void saveSync()}>
							Save
						</button>
						<button class="btn" disabled={!syncSt?.has_token || !!syncBusy} on:click={() => void syncPush()}>
							{syncBusy === 'push' ? 'pushing…' : 'Push now'}
						</button>
						<button class="btn" disabled={!syncSt?.has_token || !!syncBusy} on:click={() => void syncPull()}>
							{syncBusy === 'pull' ? 'pulling…' : 'Pull now'}
						</button>
					</div>
					<p class="warn-note small">The token needs the <code>gist</code> scope and is stored locally with 0600 permissions.</p>
					<p class="dim small">Config and bookmarks only. Browsing data never syncs.</p>
				</section>
			{:else if $settingsSection === 'plugins'}
				<section>
					<h3>Plugins</h3>
					{#if pluginList.length === 0}
						<p class="dim small">No plugins loaded.</p>
					{:else}
						<ul class="kb-list">
							{#each pluginList as p (p.name)}
								<li>
									<strong>{p.name}</strong>
									<span class="badge" class:err={p.status !== 'ok' && p.status !== 'running'}>{p.status}</span>
									{#if p.errors}
										<pre class="perr">{Array.isArray(p.errors) ? p.errors.join('\n') : String(p.errors)}</pre>
									{/if}
								</li>
							{/each}
						</ul>
					{/if}
				</section>

				<section>
					<h3>Packages &amp; gallery</h3>
					<button class="btn" on:click={() => void browseGallery()}>
						{galleryLoading ? 'loading…' : 'Browse community gallery'}
					</button>
					{#if galleryItems}
						<ul class="gallery">
							{#each galleryItems as g (g.repo)}
								<li>
									<div class="g-head"><strong>{g.name}</strong><span class="badge kind">{g.kind}</span><span class="dim small">★ {g.stars}</span></div>
									<p class="dim small">{g.description}</p>
									<p class="dim small mono">{g.repo}</p>
									<button class="btn" disabled={installing === g.repo} on:click={() => void installFromGallery(g.repo)}>
										{installing === g.repo ? 'installing…' : 'Install'}
									</button>
								</li>
							{/each}
						</ul>
					{/if}
					<div class="row">
						<input placeholder="GitHub URL or path/to/pkg.umbrapkg" bind:value={pkgSource} aria-label="Package source" />
						<button class="btn" disabled={!!installing || !pkgSource.trim()} on:click={() => void installFromSource()}>
							{installing && installing === pkgSource.trim() ? 'installing…' : 'Install'}
						</button>
					</div>
				</section>
			{:else if $settingsSection === 'newtab'}
				<section>
					<h3>New tab page</h3>
					<p class="dim small">
						Point new tabs at any page you like. Leave it empty to use Umbra's built-in page,
						which you can also customize below.
					</p>
					<label class="row">
						<span>Custom URL</span>
						<input
							placeholder="e.g. https://news.ycombinator.com"
							bind:value={ntUrl}
							aria-label="Custom new tab URL"
							autocomplete="off"
							spellcheck="false"
						/>
					</label>
					<h4>Built-in page</h4>
					<label class="row">
						<span>Background image (URL or file path)</span>
						<input
							placeholder="https://… or /home/wyind/pictures/wall.jpg"
							bind:value={ntBackground}
							aria-label="New tab background image"
							autocomplete="off"
							spellcheck="false"
						/>
					</label>
					<label class="row">
						<span>Greeting line</span>
						<input
							placeholder="Optional text under the clock"
							bind:value={ntGreeting}
							aria-label="New tab greeting"
							spellcheck="false"
						/>
					</label>
					<label class="row toggle">
						<span>Show clock</span>
						<input type="checkbox" checked={ntClock} on:change={(e) => (ntClock = e.currentTarget.checked)} />
					</label>
					<div class="row">
						<button class="btn" on:click={() => void saveNewtab()}>Save</button>
					</div>
				</section>
			{:else if $settingsSection === 'config'}
				<section>
					<h3>Config files</h3>
					<p class="dim small">
						Every TOML file Umbra keeps, editable here - no terminal needed.
						Saving <code>umbra.toml</code> validates and hot-applies; bookmarks &amp; history reload live.
					</p>
					<div class="cfg-layout">
						<ul class="cfg-list" aria-label="Config files">
							{#each cfgFiles as f (f.path)}
								<li>
									<button
										class="cfg-item"
										class:active={f.path === cfgSelected}
										on:click={() => void selectCfgFile(f.path)}
									>
										<span class="cfg-path">{f.path}</span>
										<span class="dim small">{(f.size / 1024).toFixed(1)} kB</span>
									</button>
								</li>
							{:else}
								<li class="dim small">No TOML files found.</li>
							{/each}
						</ul>
						<div class="cfg-editor-wrap">
							<textarea
								class="cfg-editor"
								bind:value={cfgContent}
								spellcheck="false"
								disabled={!cfgSelected || cfgBusy}
								placeholder={cfgSelected ? '' : 'Select a file to edit…'}
								aria-label={`Editor for ${cfgSelected || 'config file'}`}
							></textarea>
							<div class="row">
								<button class="btn" disabled={!cfgSelected || !cfgDirty || cfgBusy} on:click={() => void saveCfgFile()}>
									{cfgBusy ? 'saving…' : cfgDirty ? 'Save & apply' : 'Saved'}
								</button>
								<button class="btn ghost" disabled={!cfgSelected || cfgBusy} on:click={() => void selectCfgFile(cfgSelected)}>
									Revert
								</button>
								{#if cfgDirty}<span class="unsaved dim small">unsaved changes</span>{/if}
							</div>
						</div>
					</div>
				</section>
			{:else if $settingsSection === 'about'}
				<section class="about">
					<img class="logo-img" src="/umbra-logo.png" alt="Umbra Logo" />
					<h3 class="name">Umbra <span class="ver mono">v{VERSION}</span></h3>
					<p class="tagline dim">A quiet, glassy browser tuned entirely from one TOML file.</p>
					<p class="dim small">Built with Tauri using the WebKitGTK rendering engine.</p>
					<ul class="credits" aria-label="Credits">
						<li>SvelteKit</li>
						<li>Tailwind CSS</li>
						<li>GSAP</li>
						<li>mlua</li>
						<li>wasmtime</li>
						<li>Ollama</li>
					</ul>
					<a class="projlink" href="https://github.com/abysssoftware-dev/umbra-browser" target="_blank" rel="noreferrer">
					github.com/abysssoftware-dev/umbra-browser
				</a>
					<p class="note small">Local AI is optional and on-device only. Nothing leaves your machine unless you ask it to.</p>
				</section>

				<section>
					<h3>About &amp; Help</h3>
					<div class="row">
						<span>Tutorial</span>
						<button class="btn" on:click={() => tutorialOpen.set(true)}>Start interactive tutorial</button>
						<span class="dim small">hot-reload walkthrough, ~1 min</span>
					</div>
				</section>
			{/if}
		</div>
	</div>
{/if}

<style>
	.page {
		position: absolute;
		inset: 0;
		display: flex;
		background: color-mix(in srgb, var(--umbra-color-bg) 96%, transparent);
		z-index: 10;
		min-height: 0;
	}
	.rail {
		width: 230px;
		flex-shrink: 0;
		border-right: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
		padding: 14px 10px;
		display: flex;
		flex-direction: column;
		gap: 4px;
		overflow-y: auto;
	}
	.back {
		align-self: flex-start;
		margin-bottom: 8px;
	}
	h2 {
		font-size: 15px;
		margin: 0 0 12px 8px;
		letter-spacing: 1px;
	}
	.rail-item {
		/* explicit reset - `all: unset` breaks pointer interaction on WebKitGTK */
		appearance: none;
		-webkit-appearance: none;
		background: transparent;
		border: none;
		font: inherit;
		color: inherit;
		cursor: pointer;
		padding: 7px 12px;
		font-size: 13px;
		text-align: left;
		border-radius: var(--umbra-radius-small);
		color: var(--umbra-color-fg-dim);
		transition:
			background var(--umbra-duration) var(--umbra-ease),
			color var(--umbra-duration) var(--umbra-ease);
	}
	.rail-item:hover {
		background: color-mix(in srgb, var(--umbra-color-fg) 7%, transparent);
		color: var(--umbra-color-fg);
	}
	.rail-item.active {
		background: color-mix(in srgb, var(--umbra-color-accent) 18%, transparent);
		color: var(--umbra-color-fg);
	}
	.pane {
		flex: 1;
		min-width: 0;
		overflow-y: auto;
		padding: 20px 28px 32px;
		display: flex;
		flex-direction: column;
		gap: 20px;
		max-width: 720px;
	}
	section {
		display: flex;
		flex-direction: column;
		gap: 8px;
		border-bottom: 1px solid color-mix(in srgb, var(--umbra-color-border) 35%, transparent);
		padding-bottom: 14px;
	}
	h3 {
		font-size: 12px;
		margin: 0 0 8px;
		text-transform: uppercase;
		letter-spacing: 2px;
		color: var(--umbra-color-accent);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 13px;
		flex-wrap: wrap;
	}
	.row > span:first-child {
		min-width: 110px;
		color: var(--umbra-color-fg-dim);
	}
	select,
	input {
		appearance: none;
		-webkit-appearance: none;
		box-sizing: border-box;
		height: 28px;
		padding: 0 8px;
		font: inherit;
		font-size: 12.5px;
		border-radius: var(--umbra-radius-small);
		background: color-mix(in srgb, var(--umbra-color-surface) 85%, transparent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 70%, transparent);
		color: var(--umbra-color-fg);
		min-width: 0;
	}
	select {
		cursor: pointer;
		padding-right: 20px;
	}
	select option {
		background-color: var(--umbra-color-surface, #26262e);
		color: var(--umbra-color-fg, #eeeff2);
		padding: 6px 10px;
	}
	input {
		flex: 1;
	}

	.preset-grid,
	.layout-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
		gap: 8px;
		margin: 4px 0 10px;
	}
	.preset-chip,
	.layout-chip {
		appearance: none;
		-webkit-appearance: none;
		background: transparent;
		border: none;
		font: inherit;
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		font-size: 12px;
		border-radius: var(--umbra-radius-small);
		background: color-mix(in srgb, var(--umbra-color-surface) 60%, transparent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
		color: var(--umbra-color-fg-dim);
		transition:
			background var(--umbra-duration) var(--umbra-ease),
			border-color var(--umbra-duration) var(--umbra-ease),
			color var(--umbra-duration) var(--umbra-ease);
	}
	.preset-chip:hover,
	.layout-chip:hover {
		background: color-mix(in srgb, var(--umbra-color-surface) 95%, transparent);
		border-color: color-mix(in srgb, var(--umbra-color-accent) 45%, transparent);
		color: var(--umbra-color-fg);
	}
	.preset-chip.active,
	.layout-chip.active {
		background: color-mix(in srgb, var(--umbra-color-accent) 20%, var(--umbra-color-surface));
		border-color: var(--umbra-color-accent);
		color: var(--umbra-color-fg);
		font-weight: 500;
	}
	.color-dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		flex-shrink: 0;
	}
	.dot-glass { background: #8f7bff; box-shadow: 0 0 8px rgba(143, 123, 255, 0.6); }
	.dot-material { background: #d0bcff; box-shadow: 0 0 8px rgba(208, 188, 255, 0.6); }
	.dot-gruvbox { background: #fe8019; box-shadow: 0 0 8px rgba(254, 128, 25, 0.6); }
	.dot-nord { background: #88c0d0; box-shadow: 0 0 8px rgba(136, 192, 208, 0.6); }
	.dot-catppuccin { background: #cba6f7; box-shadow: 0 0 8px rgba(203, 166, 247, 0.6); }
	.dot-ocean { background: #82c8ea; box-shadow: 0 0 8px rgba(130, 200, 234, 0.6); }
	.dot-custom { background: conic-gradient(#ff6b6b, #fe8019, #5fd39a, #88c0d0, #8f7bff, #ff6b6b); box-shadow: 0 0 8px rgba(143, 123, 255, 0.5); }
	.p-label {
		flex: 1;
		text-transform: capitalize;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 100px;
	}
	.token-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: 8px 16px;
		margin: 4px 0 10px;
	}
	.token-row {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 12.5px;
		min-width: 0;
	}
	.t-label {
		width: 88px;
		flex-shrink: 0;
		color: var(--umbra-color-fg-dim);
	}
	.color-swatch {
		appearance: none;
		-webkit-appearance: none;
		width: 26px;
		height: 26px;
		padding: 0;
		border-radius: var(--umbra-radius-small);
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 70%, transparent);
		background: transparent;
		cursor: pointer;
		flex-shrink: 0;
	}
	.t-value {
		flex: 1;
	}

	/* config file editors */
	.cfg-layout {
		display: flex;
		gap: 12px;
		align-items: stretch;
		min-height: 380px;
	}
	.cfg-list {
		list-style: none;
		margin: 0;
		padding: 0;
		width: 210px;
		flex-shrink: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
		max-height: 460px;
		overflow-y: auto;
	}
	.cfg-item {
		appearance: none;
		-webkit-appearance: none;
		background: transparent;
		border: none;
		font: inherit;
		color: inherit;
		cursor: pointer;
		width: 100%;
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		padding: 6px 9px;
		text-align: left;
		border-radius: var(--umbra-radius-small);
		font-size: 12px;
		transition: background var(--umbra-duration) var(--umbra-ease);
	}
	.cfg-item:hover {
		background: color-mix(in srgb, var(--umbra-color-fg) 7%, transparent);
	}
	.cfg-item.active {
		background: color-mix(in srgb, var(--umbra-color-accent) 18%, transparent);
	}
	.cfg-path {
		font-family: var(--umbra-font-mono);
		word-break: break-all;
	}
	.cfg-editor-wrap {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.cfg-editor {
		appearance: none;
		flex: 1;
		min-height: 320px;
		resize: vertical;
		box-sizing: border-box;
		padding: 10px 12px;
		font-family: var(--umbra-font-mono);
		font-size: 12px;
		line-height: 1.55;
		tab-size: 4;
		color: var(--umbra-color-fg);
		background: color-mix(in srgb, var(--umbra-color-surface) 85%, transparent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 70%, transparent);
		border-radius: var(--umbra-radius-small);
		outline: none;
	}
	.cfg-editor:focus {
		border-color: color-mix(in srgb, var(--umbra-color-accent) 60%, transparent);
	}
	.unsaved {
		color: var(--umbra-color-accent);
	}
	.active-badge {
		font-size: 11px;
		color: var(--umbra-color-accent);
		font-weight: 700;
	}
	.btn {
		appearance: none;
		-webkit-appearance: none;
		background: transparent;
		border: none;
		font: inherit;
		cursor: pointer;
		height: 26px;
		padding: 0 10px;
		font-size: 12px;
		display: inline-flex;
		align-items: center;
		border-radius: var(--umbra-radius-small);
		background: color-mix(in srgb, var(--umbra-color-accent) 18%, transparent);
		border: 1px solid color-mix(in srgb, var(--umbra-color-accent) 45%, transparent);
		color: var(--umbra-color-fg);
		transition: background var(--umbra-duration) var(--umbra-ease);
	}
	.btn:hover:not(:disabled) {
		background: color-mix(in srgb, var(--umbra-color-accent) 32%, transparent);
	}
	.btn:disabled {
		opacity: 0.5;
	}
	.btn.ghost {
		background: transparent;
		border-color: color-mix(in srgb, var(--umbra-color-border) 90%, transparent);
	}
	.mini {
		appearance: none;
		-webkit-appearance: none;
		background: transparent;
		border: none;
		font: inherit;
		cursor: pointer;
		font-size: 11px;
		padding: 1px 6px;
		border-radius: 4px;
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 80%, transparent);
	}
	.mini.danger:hover {
		color: var(--umbra-color-danger);
		border-color: var(--umbra-color-danger);
	}
	.kb-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 12.5px;
	}
	.kb-list li {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.kb-list code {
		font-family: var(--umbra-font-mono);
		background: color-mix(in srgb, var(--umbra-color-fg) 8%, transparent);
		padding: 1px 6px;
		border-radius: 4px;
	}
	code {
		font-family: var(--umbra-font-mono);
		font-size: 11.5px;
		background: color-mix(in srgb, var(--umbra-color-fg) 9%, transparent);
		padding: 1px 5px;
		border-radius: 4px;
	}
	.spacer {
		flex: 1;
	}
	.capturing {
		color: var(--umbra-color-accent);
		font-size: 12px;
		animation: pulse 1s infinite;
	}
	@keyframes pulse {
		50% {
			opacity: 0.4;
		}
	}
	.toggle input[type='checkbox'] {
		appearance: none;
		-webkit-appearance: none;
		margin: 0;
		flex-shrink: 0;
		flex-grow: 0;
		width: 34px;
		height: 18px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--umbra-color-fg) 15%, transparent);
		position: relative;
		cursor: pointer;
		transition: background var(--umbra-duration) var(--umbra-ease);
	}
	.toggle input[type='checkbox'][checked],
	.toggle input[type='checkbox']:checked {
		background: var(--umbra-color-accent);
	}
	.badge {
		font-size: 10px;
		font-family: var(--umbra-font-mono);
		padding: 1px 6px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--umbra-color-ok) 18%, transparent);
		color: var(--umbra-color-ok);
		border: 1px solid color-mix(in srgb, var(--umbra-color-ok) 45%, transparent);
	}
	.badge.kind,
	.badge.err {
		background: color-mix(in srgb, var(--umbra-color-accent) 15%, transparent);
		color: var(--umbra-color-accent);
		border-color: color-mix(in srgb, var(--umbra-color-accent) 45%, transparent);
	}
	.badge.err {
		background: color-mix(in srgb, var(--umbra-color-danger) 15%, transparent);
		color: var(--umbra-color-danger);
		border-color: color-mix(in srgb, var(--umbra-color-danger) 45%, transparent);
	}
	.gallery {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.gallery li {
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
		border-radius: var(--umbra-radius-small);
		padding: 10px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.g-head {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	p {
		margin: 0;
	}
	.mono {
		font-family: var(--umbra-font-mono);
	}
	.setup {
		border: 1px dashed color-mix(in srgb, var(--umbra-color-border) 90%, transparent);
		border-radius: var(--umbra-radius-small);
		padding: 10px 12px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.setup ol {
		margin: 0;
		padding-left: 18px;
		font-size: 12.5px;
		line-height: 1.6;
	}
	.doclink {
		font-size: 11.5px;
		color: var(--umbra-color-accent);
		text-decoration: underline dotted;
		cursor: default;
		width: fit-content;
	}
	.import-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.import-list li {
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 60%, transparent);
		border-radius: var(--umbra-radius-small);
		padding: 10px;
		display: flex;
		flex-direction: column;
		gap: 6px;
		align-items: flex-start;
	}
	.i-head {
		display: flex;
		align-items: baseline;
		gap: 8px;
		font-size: 13px;
	}
	.checks {
		gap: 14px;
	}
	.check {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 12.5px;
		color: var(--umbra-color-fg-dim);
	}
	.check input[type='checkbox'] {
		appearance: none;
		-webkit-appearance: none;
		margin: 0;
		flex-shrink: 0;
		flex-grow: 0;
		width: 13px;
		height: 13px;
		border-radius: 4px;
		border: 1px solid color-mix(in srgb, var(--umbra-color-fg) 40%, transparent);
		display: inline-block;
		vertical-align: middle;
		cursor: pointer;
	}
	.check input[type='checkbox']:checked {
		background: var(--umbra-color-accent);
		border-color: var(--umbra-color-accent);
	}
	.check input[type='checkbox']:disabled {
		opacity: 0.35;
	}
	.warn-note {
		margin: 0;
		color: var(--umbra-color-danger);
	}
	.warn-note code {
		font-family: var(--umbra-font-mono);
	}
	.perr {
		width: 100%;
		margin: 0;
		font-family: var(--umbra-font-mono);
		font-size: 11px;
		color: var(--umbra-color-danger);
		white-space: pre-wrap;
		max-height: 80px;
		overflow-y: auto;
	}
	.ok-note {
		color: var(--umbra-color-ok);
	}

	/* about */
	section.about {
		align-items: flex-start;
	}
	.logo-img {
		width: 48px;
		height: 48px;
		object-fit: contain;
		margin-bottom: 4px;
		filter: drop-shadow(0 4px 12px color-mix(in srgb, var(--umbra-color-accent) 35%, transparent));
	}
	h3.name {
		text-transform: none;
		letter-spacing: 0.5px;
		font-size: 17px;
		color: var(--umbra-color-fg);
		display: flex;
		align-items: baseline;
		gap: 10px;
	}
	.ver {
		font-size: 11.5px;
		color: var(--umbra-color-fg-dim);
		background: none;
		padding: 0;
	}
	.tagline {
		margin: 0;
		font-size: 13px;
	}
	.credits {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.credits li {
		font-size: 11px;
		font-family: var(--umbra-font-mono);
		padding: 2px 9px;
		border-radius: 999px;
		border: 1px solid color-mix(in srgb, var(--umbra-color-border) 80%, transparent);
		color: var(--umbra-color-fg-dim);
	}
	.projlink {
		font-size: 12.5px;
		color: var(--umbra-color-accent);
		text-decoration: underline dotted;
	}
	.note {
		color: var(--umbra-color-fg-dim);
		border-left: 2px solid color-mix(in srgb, var(--umbra-color-accent) 55%, transparent);
		padding-left: 10px;
		max-width: 480px;
		line-height: 1.55;
	}
</style>
