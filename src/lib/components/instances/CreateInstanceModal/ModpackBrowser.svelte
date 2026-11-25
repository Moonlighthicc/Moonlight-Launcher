<script lang="ts">
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	import Loading from "$lib/icons/Loading.svelte";
	import { animDuration } from "$lib/utils/animations";
	import Select from "$lib/components/layout/Select.svelte";
	import ModpackDetails from "./ModpackDetails.svelte";
	import PackLoaderBadge from "./PackLoaderBadge.svelte";
	import { getLoaderLogo } from "$lib/icons/logos";
	import { PACK_LOADERS, type PackLoader } from "$lib/utils/modpackLoaders";
	import type {
		PackLoaders,
		PackGalleryImage,
		PackVersion,
	} from "$lib/utils/modpackVersions";
	import type { Snippet } from "svelte";

	export type ModpackItem = {
		id: string | number;
		title: string;
		description: string;
		iconUrl?: string | null;
		downloads: number;
		author?: string;
		categories?: string[];
		packLoaders?: PackLoader[];
	};

	export type ModpackSort = "relevance" | "downloads" | "newest";

	export interface ModpackFilters {
		sort: ModpackSort;
		category: string | null;
		gameVersion: string | null;
		loader: string | null;
	}

	let {
		query = $bindable(""),
		items = [],
		totalHits = 0,
		hasMore,
		manualPagination = false,
		searching = false,
		loadingMore = false,
		selectedItem = $bindable<ModpackItem | null>(null),
		versionOptions = [],
		loadingVersions = false,
		installing = false,
		installTask,
		installError = null,
		gallery = [],
		website,
		loaders,
		searchPlaceholder = "",
		emptySearchingText = "",
		emptyNoResultsText = "",
		filters = $bindable<ModpackFilters>({
			sort: "downloads",
			category: null,
			gameVersion: null,
			loader: null,
		}),
		categoryOptions = [] as { value: string; label: string }[],
		gameVersionOptions = [] as { value: string; label: string }[],
		onSearch,
		onLoadMore,
		onFilterChange,
		onSelect,
		onBack,
		onInstall,
		onRetryInstall,
		onDismissInstall,
		detailExtra,
	}: {
		query?: string;
		items?: ModpackItem[];
		totalHits?: number;
		hasMore?: boolean;
		manualPagination?: boolean;
		searching?: boolean;
		loadingMore?: boolean;
		selectedItem?: ModpackItem | null;
		versionOptions?: PackVersion[];
		loadingVersions?: boolean;
		installing?: boolean;
		installTask?: { archive: string; version: string; name: string; error: string | null };
		installError?: string | null;
		gallery?: PackGalleryImage[];
		website?: string;
		loaders?: PackLoaders;
		searchPlaceholder?: string;
		emptySearchingText?: string;
		emptyNoResultsText?: string;
		filters?: ModpackFilters;
		categoryOptions?: { value: string; label: string }[];
		gameVersionOptions?: { value: string; label: string }[];
		onSearch?: () => void;
		onLoadMore?: () => void;
		onFilterChange?: () => void;
		onSelect?: (item: ModpackItem) => void;
		onBack?: () => void;
		onInstall?: (version: string, name: string) => void;
		onRetryInstall?: () => void;
		onDismissInstall?: () => void;
		detailExtra?: Snippet<[ModpackItem]>;
	} = $props();

	let sentinelEl: HTMLDivElement | undefined = $state();
	let searchInput: HTMLInputElement | undefined = $state();
	let details = $state<ReturnType<typeof ModpackDetails>>();
	export function goBackOnEscape() {
		if (!selectedItem) return false;
		details?.goBackOnEscape();
		return true;
	}
	const detailDuration = $derived(animDuration(200));

	const SEARCH_DEBOUNCE_MS = 350;
	let searchTimer: ReturnType<typeof setTimeout> | null = null;
	let spinOnce = $state(false);
	let spinTimer: ReturnType<typeof setTimeout> | null = null;
	// Último texto con el que se lanzó una búsqueda; evita repetirla sin cambios.
	let lastSearched = query;

	function handleSearch() {
		if (searchTimer) clearTimeout(searchTimer);
		searchTimer = null;
		lastSearched = query;
		onSearch?.();
	}

	/** Botón "actualizar": repite la búsqueda ya y gira las flechas al menos una vuelta. */
	function handleRefresh() {
		spinOnce = true;
		if (spinTimer) clearTimeout(spinTimer);
		spinTimer = setTimeout(() => (spinOnce = false), 700);
		handleSearch();
	}

	function clearQuery() {
		if (!query) return;
		query = "";
		handleSearch();
		searchInput?.focus();
	}

	// Búsqueda automática al escribir (con pequeña espera para no saturar la API).
	$effect(() => {
		const current = query;
		if (current === lastSearched) return;
		if (searchTimer) clearTimeout(searchTimer);
		searchTimer = setTimeout(() => {
			searchTimer = null;
			handleSearch();
		}, SEARCH_DEBOUNCE_MS);
	});

	$effect(() => () => {
		if (searchTimer) clearTimeout(searchTimer);
		if (spinTimer) clearTimeout(spinTimer);
	});

	function handleFilterChange() {
		onFilterChange?.();
	}

	function setSort(sort: ModpackSort) {
		filters.sort = sort;
		handleFilterChange();
	}

	function setCategory(category: string | null) {
		filters.category = category;
		handleFilterChange();
	}

	function setGameVersion(version: string | null) {
		filters.gameVersion = version;
		handleFilterChange();
	}

	$effect(() => {
		const el = sentinelEl;
		if (!el) return;

		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting) {
					onLoadMore?.();
				}
			},
			{ rootMargin: "300px" },
		);

		observer.observe(el);
		return () => observer.disconnect();
	});
</script>

<div class="modpack-browser" style:--detail-duration={`${detailDuration}ms`}>
	{#if !selectedItem}
		<div class="search-bar">
			<div class="search-field">
				<input
					bind:this={searchInput}
					type="text"
					class="search-input"
					bind:value={query}
					placeholder={searchPlaceholder}
					autocomplete="off"
					spellcheck="false"
					onkeydown={(e) => {
						if (e.key === "Enter" && !e.isComposing) handleSearch();
						else if (e.key === "Escape" && query) {
							e.stopPropagation();
							clearQuery();
						}
					}}
				/>
				{#if query}
					<button
						type="button"
						class="clear-btn"
						onclick={clearQuery}
						aria-label={t("createInstance.clearSearch")}
						title={t("createInstance.clearSearch")}
					>
						<Icon name="ui:close" size={14} />
					</button>
				{/if}
			</div>
			<button
				type="button"
				class="btn-primary refresh-btn"
				onclick={handleRefresh}
				aria-label={t("createInstance.refreshSearch")}
				title={t("createInstance.refreshSearch")}
			>
				<svg
					class="refresh-icon"
					class:spinning={searching || spinOnce}
					width="18"
					height="18"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
				>
					<path d="M21 12a9 9 0 0 0-15.5-6.2L3 8" />
					<polyline points="3 3 3 8 8 8" />
					<path d="M3 12a9 9 0 0 0 15.5 6.2L21 16" />
					<polyline points="21 21 21 16 16 16" />
				</svg>
			</button>
		</div>

		<div class="filter-bar">
			<div class="filter-group">
				<span class="filter-label"
					>{t("createInstance.filterSortLabel")}</span
				>
				<div class="filter-chips">
					<button
						type="button"
						class="filter-chip"
						class:active={filters.sort === "relevance"}
						onclick={() => setSort("relevance")}
					>
						{t("createInstance.sortRelevance")}
					</button>
					<button
						type="button"
						class="filter-chip"
						class:active={filters.sort === "downloads"}
						onclick={() => setSort("downloads")}
					>
						{t("createInstance.sortDownloads")}
					</button>
					<button
						type="button"
						class="filter-chip"
						class:active={filters.sort === "newest"}
						onclick={() => setSort("newest")}
					>
						{t("createInstance.sortNewest")}
					</button>
				</div>
			</div>

			<div class="filter-selects">
				<Select
					value={filters.loader ?? ""}
					options={[
						{ value: "", label: t("createInstance.filterAllLoaders") },
						...Object.entries(PACK_LOADERS).map(([value, label]) => ({ value, label, icon: getLoaderLogo(value) })),
					]}
					label={t("createInstance.filterLoaderLabel")}
					onchange={(value) => {
						filters.loader = value || null;
						handleFilterChange();
					}}
				/>
				<Select
					value={filters.category ?? ""}
					options={[
						{
							value: "",
							label: t("createInstance.filterAllCategories"),
						},
						...categoryOptions,
					]}
					placeholder={t("createInstance.filterCategoryLabel")}
					label={t("createInstance.filterCategoryLabel")}
					onchange={(value) => setCategory(value || null)}
				/>
				<Select
					value={filters.gameVersion ?? ""}
					options={[
						{
							value: "",
							label: t("createInstance.filterAllVersions"),
						},
						...gameVersionOptions,
					]}
					placeholder={t("createInstance.filterVersionLabel")}
					label={t("createInstance.filterVersionLabel")}
					onchange={(value) => setGameVersion(value || null)}
				/>
			</div>
		</div>
	{/if}

	{#if selectedItem}
		<div class="detail-view">
			{#key selectedItem.id}
				<ModpackDetails
					bind:this={details}
					item={selectedItem}
					versions={versionOptions}
					loading={loadingVersions}
					{installing}
					{installTask}
					error={installError}
					{gallery}
					{website}
					description={detailExtra}
					{onBack}
					{onInstall}
					{onRetryInstall}
					{onDismissInstall}
					{loaders}
				/>
			{/key}
		</div>
	{:else}
		<div class="results-panel">
			{#if searching && items.length === 0}
				<div class="empty-state">
					<Loading />
					<span>{emptySearchingText}</span>
				</div>
			{:else if items.length === 0 && !(hasMore && manualPagination)}
				<div class="empty-state">{emptyNoResultsText}</div>
			{:else}
				<div class="results-grid">
					{#each items as item (item.id)}
						<button
							type="button"
							class="pack-card"
							onclick={() => onSelect?.(item)}
						>
							<div class="pack-icon-wrap">
								{#if item.iconUrl}
									<img
										src={item.iconUrl}
										alt=""
										class="pack-icon"
										loading="lazy"
										decoding="async"
										onerror={(e) => {
											(
												e.currentTarget as HTMLImageElement
											).style.display = "none";
										}}
									/>
								{/if}
							</div>
							<div class="pack-info">
								<span class="pack-title">{item.title}</span>
								<span class="pack-desc">{item.description}</span
								>
								<span class="pack-meta loader-meta">
									<span>{item.downloads.toLocaleString()} {t("createInstance.downloads")}</span>
									{#each item.packLoaders ?? [] as loader (loader)}
										<PackLoaderBadge {loader} />
									{/each}
								</span>
							</div>
						</button>
					{/each}
				</div>
				{#if (hasMore ?? items.length < totalHits) && manualPagination}
					<div class="load-sentinel">
						<button type="button" class="btn-secondary" disabled={loadingMore} onclick={onLoadMore}>
							{loadingMore ? t("createInstance.loading") : t("createInstance.loadMore")}
						</button>
					</div>
				{:else if hasMore ?? items.length < totalHits}
					<div bind:this={sentinelEl} class="load-sentinel">
						{#if loadingMore}
							<Loading />
						{:else}
							<span class="sentinel-hint"
								>{t("createInstance.scrollForMore")}</span
							>
						{/if}
					</div>
				{/if}
			{/if}
		</div>
	{/if}
</div>

<style>
	.loader-meta {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 4px 6px;
	}
	.modpack-browser {
		container-type: inline-size;
		display: flex;
		flex-direction: column;
		gap: 12px;
		height: 100%;
		min-height: 300px;
	}

	.search-bar {
		--search-h: 38px;
		display: flex;
		gap: 8px;
		align-items: stretch;
	}

	.search-field {
		position: relative;
		flex: 1;
		min-width: 0;
		display: flex;
	}

	.search-input {
		flex: 1;
		min-width: 0;
		height: var(--search-h);
		box-sizing: border-box;
		padding: 0 34px 0 12px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: var(--bg-input);
		color: var(--text-primary);
		font-size: 0.82rem;
		outline: none;
	}

	.search-input:focus {
		border-color: var(--accent);
	}

	.clear-btn {
		position: absolute;
		top: 50%;
		right: 6px;
		transform: translateY(-50%);
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 0;
		border: none;
		border-radius: 50%;
		background: transparent;
		color: var(--text-secondary);
		cursor: pointer;
	}

	.clear-btn:hover {
		background: var(--surface-hover, rgba(255, 255, 255, 0.08));
		color: var(--text-primary);
	}

	.clear-btn:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}

	.refresh-btn {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--search-h);
		height: var(--search-h);
		min-width: 0;
		padding: 0;
		box-sizing: border-box;
	}

	.refresh-icon {
		display: block;
	}

	.refresh-icon.spinning {
		animation: refresh-spin 0.7s linear infinite;
	}

	@keyframes refresh-spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.refresh-icon.spinning {
			animation-duration: 1.6s;
		}
	}

	.filter-bar {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 8px;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
	}

	.filter-group {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.filter-label {
		font-size: 0.65rem;
		font-weight: 700;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.8px;
	}

	.filter-chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.filter-chip {
		padding: 4px 11px;
		background: var(--surface-card);
		border: 1px solid var(--border);
		border-radius: 20px;
		color: var(--text-secondary);
		font-size: 0.7rem;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.15s ease;
		font-family: inherit;
		text-transform: capitalize;
	}

	.filter-chip:hover {
		background: var(--surface-hover);
		color: var(--text-primary);
		border-color: var(--text-tertiary);
	}

	.filter-chip.active {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--accent-text);
		box-shadow: var(--shadow-sm);
	}

	.filter-selects {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 140px), 1fr));
		gap: 8px;
	}

	.results-panel {
		flex: 1;
		overflow-y: auto;
		min-height: 0;
		max-height: 440px;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 10px;
		padding: 40px 16px;
		color: var(--text-muted);
		font-size: 0.82rem;
	}

	.results-grid {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.pack-card {
		display: flex;
		gap: 10px;
		padding: 10px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: transparent;
		color: inherit;
		cursor: pointer;
		text-align: left;
		width: 100%;
		transition:
			background 0.15s ease,
			border-color 0.15s ease;
	}

	.pack-card:hover {
		background: var(--surface-hover);
	}

	.pack-icon-wrap {
		width: 48px;
		height: 48px;
		border-radius: var(--border-radius-sm);
		overflow: hidden;
		flex-shrink: 0;
		background: var(--bg-card);
	}

	.pack-icon {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.pack-info {
		display: flex;
		flex-direction: column;
		gap: 3px;
		min-width: 0;
		flex: 1;
	}

	.pack-title {
		font-size: 0.82rem;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.pack-desc {
		font-size: 0.72rem;
		color: var(--text-secondary);
		display: -webkit-box;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		line-clamp: 2;
		overflow: hidden;
	}

	.pack-meta {
		font-size: 0.65rem;
		color: var(--text-tertiary);
	}

	.load-sentinel {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 12px;
	}

	.sentinel-hint {
		font-size: 0.7rem;
		color: var(--text-tertiary);
	}

	.detail-view {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 18px;
		overflow-wrap: anywhere;
	}

	.detail-view,
	.results-panel {
		animation: contentIn var(--detail-duration) ease-out;
	}

	@keyframes contentIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.detail-view,
		.results-panel {
			animation: none;
		}
	}
</style>
