<script lang="ts">
	import { open, save } from "@tauri-apps/plugin-dialog";
	import { invoke } from "@tauri-apps/api/core";
	import { convertFileSrc } from "@tauri-apps/api/core";
	import type { ThemeEntry } from "$lib/types/types";
	import { t } from "$lib/i18n";
	import { SvelteMap } from "svelte/reactivity";
	import CheckIcon from "$lib/icons/CheckIcon.svelte";
	import DownloadIcon from "$lib/icons/DownloadIcon.svelte";
	import Trash from "$lib/icons/Trash.svelte";
	import { applyTheme, importMoonTheme, THEME_EXTENSION } from "$lib/api/themeManager";
	import { themeDiagnostics } from "$lib/state/themeDiagnostics.svelte";
	import { showSuccess, showError } from "$lib/state/state.svelte";

	let {
		themes = $bindable(),
		value = $bindable(),
		onchange,
		onrefresh,
	}: {
		themes: ThemeEntry[];
		value: string;
		onchange?: (id: string) => void;
		onrefresh?: () => void | Promise<void>;
	} = $props();

	let importing = $state(false);

	const groups = $derived.by(() => {
		const map = new SvelteMap<string, ThemeEntry[]>();
		for (const theme of themes) {
			const author =
				theme.author || t("settings.launcher.themesUnknownAuthor");
			let list = map.get(author);
			if (!list) {
				list = [];
				map.set(author, list);
			}
			list.push(theme);
		}
		return map;
	});

	async function handleImport() {
		importing = true;
		try {
			const selected = await open({
				multiple: false,
				filters: [{ name: "MoonLauncher", extensions: [THEME_EXTENSION] }],
			});
			if (typeof selected === "string") {
				const entry = await importMoonTheme(selected);
				await onrefresh?.();
				const id = `user:${entry.id}`;
				if (value === id) await applyTheme(id, { force: true });
				selectTheme(id);
				showSuccess(
					t("themes.importSuccess"),
					t("themes.importSuccessMessage"),
				);
			}
		} catch (e) {
			showError(t("themes.importError"), String(e));
		} finally {
			importing = false;
		}
	}

	async function handleExport(theme: ThemeEntry) {
		const rawId = theme.id.startsWith("user:")
			? theme.id.slice(5)
			: theme.id;
		const savePath = await save({
			defaultPath: `${rawId}.${THEME_EXTENSION}`,
			filters: [{ name: "MoonLauncher", extensions: [THEME_EXTENSION] }],
		});
		if (!savePath) return;
		try {
			await invoke("export_theme", { id: rawId, dest: savePath });
		} catch (e) {
			console.error("Error exporting theme:", e);
		}
	}

	async function handleDelete(theme: ThemeEntry) {
		const rawId = theme.id.startsWith("user:")
			? theme.id.slice(5)
			: theme.id;
		const confirmed = confirm(
			t("themes.deleteConfirm").replace("{name}", theme.name),
		);
		if (!confirmed) return;
		try {
			await invoke("remove_theme", { id: rawId });
			if (value === theme.id) {
				value = "dark";
				onchange?.("dark");
			}
			onrefresh?.();
		} catch (e) {
			console.error("Error deleting theme:", e);
		}
	}

	function selectTheme(id: string) {
		value = id;
		onchange?.(id);
	}
</script>

<div class="theme-selector">
	<div class="theme-header-actions">
		<button
			type="button"
			class="btn-secondary import-btn"
			onclick={handleImport}
			disabled={importing}
		>
			{importing
				? t("settings.launcher.themesImporting")
				: t("settings.launcher.themesImport")}
		</button>
	</div>

	{#if value === themeDiagnostics.themeId && themeDiagnostics.warnings.length > 0}
		<aside
			class="theme-diagnostics"
			aria-label={t("themes.diagnostics.title")}
		>
			<strong>{t("themes.diagnostics.title")}</strong>
			<p>{t("themes.diagnostics.advisory")}</p>
			<ul>
				{#each themeDiagnostics.warnings as warning (warning.key)}
					<li>{t(warning.key, warning.params)}</li>
				{/each}
			</ul>
		</aside>
	{/if}

	{#each [...groups.entries()] as [author, themes] (author)}
		<div class="group">
			<span class="group-title">{author}</span>
			{#each themes as theme (theme.id)}
				<div
					class="theme-row"
					class:active={value === theme.id}
					role="button"
					tabindex="0"
					onclick={() => selectTheme(theme.id)}
					aria-pressed={value === theme.id}
					onkeydown={(e) => {
						if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) {
							e.preventDefault();
							selectTheme(theme.id);
						}
					}}
				>
					{#if theme.icon}
						<img
							class="theme-icon"
							src={convertFileSrc(theme.icon)}
							alt={theme.name}
						/>
					{:else if theme.preview}
						<div class="swatch">
							<div
								class="swatch-bar"
								style="background: {theme.preview.bg}"
							></div>
							<div
								class="swatch-bar"
								style="background: {theme.preview.accent}"
							></div>
							<div
								class="swatch-bar"
								style="background: {theme.preview.text}"
							></div>
						</div>
					{:else}
						<div class="swatch swatch-missing">
							<div
								class="swatch-bar"
								style="background: var(--bg-card)"
							></div>
							<div
								class="swatch-bar"
								style="background: var(--accent)"
							></div>
							<div
								class="swatch-bar"
								style="background: var(--text-primary)"
							></div>
						</div>
					{/if}
					<div class="theme-info">
						<span class="theme-name">{theme.name}</span>
						<span class="theme-meta">
							{theme.author}{#if theme.version}
								v{theme.version}{/if}
						</span>
					</div>
					<div class="theme-actions">
						{#if value === theme.id}
							<div
								class="active-badge"
								title={t("settings.launcher.themesActive")}
							>
								<CheckIcon size={14} />
							</div>
						{/if}
						{#if theme.type !== "builtin"}
							<button
								type="button"
								class="icon-btn"
								title={t("settings.launcher.themesExport")}
								onclick={(e) => {
									e.stopPropagation();
									handleExport(theme);
								}}
							>
								<DownloadIcon size={14} />
							</button>
							<button
								type="button"
								class="icon-btn danger"
								title={t("settings.launcher.themesDelete")}
								onclick={(e) => {
									e.stopPropagation();
									handleDelete(theme);
								}}
							>
								<Trash width={14} height={14} />
							</button>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/each}
</div>

<style>
	.theme-selector {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.theme-header-actions {
		display: flex;
		gap: 4px;
	}

	.theme-diagnostics {
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: var(--bg-card);
		color: var(--text-secondary);
		font-size: 0.7rem;
		line-height: 1.5;
		overflow-wrap: anywhere;
	}

	.theme-diagnostics strong {
		color: var(--text-primary);
	}

	.theme-diagnostics p {
		margin: 4px 0;
	}

	.theme-diagnostics ul {
		margin: 0;
		padding-left: 16px;
	}

	.theme-header-actions .import-btn {
		flex: 1;
	}

	.import-btn {
		min-height: var(--button-height);
	}

	.group {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.group-title {
		font-size: 0.65rem;
		font-weight: 700;
		color: var(--text-secondary);
		margin-bottom: 4px;
	}

	.theme-row {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 8px 10px;
		border: var(--border-width) solid transparent;
		border-radius: var(--border-radius);
		background: transparent;
		cursor: pointer;
		text-align: left;
		font-family: inherit;
		transition: background 0.15s;
	}

	.theme-row:hover {
		background: var(--surface-hover);
	}

	.theme-row.active {
		background: var(--selected-surface);
		border-color: var(--selected-border);
	}

	.swatch {
		display: flex;
		flex-direction: column;
		gap: 2px;
		width: 20px;
		flex-shrink: 0;
	}

	.swatch-bar {
		height: 6px;
		border-radius: 2px;
	}

	.swatch-missing {
		opacity: 0.4;
	}

	.theme-icon {
		width: 28px;
		height: 28px;
		object-fit: contain;
		border-radius: var(--border-radius-sm);
		flex-shrink: 0;
	}

	.theme-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.theme-name {
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--text-primary);
	}

	.theme-meta {
		font-size: 0.7rem;
		color: var(--text-tertiary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.theme-actions {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-shrink: 0;
	}

	.active-badge {
		color: var(--accent);
		display: flex;
		align-items: center;
	}

	.icon-btn {
		background: transparent;
		border: none;
		color: var(--text-muted);
		padding: 4px;
		border-radius: var(--border-radius-sm);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition:
			color 0.15s,
			background 0.15s;
	}

	.icon-btn:hover {
		color: var(--text-primary);
		background: var(--surface-hover);
	}

	.icon-btn.danger:hover {
		color: var(--color-error);
		background: rgba(var(--color-error-rgb), 0.1);
	}

</style>
