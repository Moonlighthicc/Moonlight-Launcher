<script lang="ts">
	import { onMount, untrack } from "svelte";
	import { ask } from "@tauri-apps/plugin-dialog";
	import { invoke } from "@tauri-apps/api/core";
	import { SvelteSet } from "svelte/reactivity";
	import {
		getAvailableVersions,
		addToQueue,
		getFabricVersions,
		getFabricLoaderVersions,
		downloadFabric,
		getLoaderVersionCatalog,
		downloadForge,
		downloadNeoForge,
		getQuiltVersions,
		getQuiltLoaderVersions,
		downloadQuilt,
		refreshAvailableVersions,
		downloadOptiFine,
	} from "$lib/api/cubicApi";
	import {
		versionsState,
		loadInstalledVersions,
	} from "$lib/state/versionsState.svelte";
	import type {
		MinecraftVersion,
		LoaderDisplayVersion,
	} from "$lib/types/types";
	import { isVersionDownloading } from "$lib/state/downloadState.svelte";
	import {
		loaderVersionId,
		pendingVersionDownloads as enqueueing,
		downloads,
		trackConnecting,
	} from "$lib/state/downloadQueueState.svelte";
	import { launcherStore } from "$lib/state/state.svelte";
	import { t } from "$lib/i18n";
	import Select from "../Select.svelte";
	import ModalBase from "../ModalBase.svelte";
	import VirtualList from "../VirtualList.svelte";
	import VersionDownloaderTabs from "./VersionDownloaderTabs.svelte";
	import Icon from "$lib/icons/Icon.svelte";
	import Loading from "$lib/icons/Loading.svelte";
	import ToggleSwitch from "$lib/components/ui/ToggleSwitch.svelte";
	import { installedVersionInfo } from "$lib/utils/versionUtils";
	import {
		catalogToGrouped,
		compareVersions,
		createCatalogCache,
		sortLoaderVersions,
		type GroupedLoaderVersions,
	} from "./versionCatalog";

	let { open = $bindable(false) }: { open: boolean } = $props();
	const id = $props.id();
	const LOADERS = [
		{ value: "vanilla", label: "Vanilla", iconName: "brand:vanilla" },
		{ value: "fabric", label: "Fabric", iconName: "brand:fabric" },
		{ value: "forge", label: "Forge", iconName: "brand:forge" },
		{ value: "neoforge", label: "NeoForge", iconName: "brand:neoforged" },
		{ value: "quilt", label: "Quilt", iconName: "brand:quilt" },
		{ value: "optifine", label: "OptiFine", iconName: "brand:optifine" },
	];
	interface CatalogItem {
		id: string;
		title: string;
		subtitle: string;
		badge: string;
		stable: boolean;
		searchText: string;
		loader?: LoaderDisplayVersion;
	}

	let loaderTab = $state("vanilla");
	const activeLoader = $derived(
		LOADERS.find((loader) => loader.value === loaderTab)!,
	);
	const isForge = $derived(loaderTab === "forge" || loaderTab === "neoforge");
	let refreshing = $state(false);
	let sourceError = $state<string | null>(null);
	let deleteError = $state("");
	let search = $state("");
	let onlyInstalled = $state(false);
	$effect(() => {
		if (!open) onlyInstalled = false;
	});
	const deleting = new SvelteSet<string>();
	const localVersions = $derived((versionsState.rawVersions ?? []).map(installedVersionInfo).filter((v) => v.loader === loaderTab));
	const showUnstable = $derived(launcherStore.settings.show_unstable_loaders);
	const showSnapshots = $derived(launcherStore.settings.show_snapshots);
	let loadingMojang = $state(false);
	let vanillaCache = $state<MinecraftVersion[] | null>(null);
	let allMcVersions = $state<string[]>([]);
	let activeCatalog = $state<GroupedLoaderVersions | null>(null);
	const remoteMcVersions = $derived(
		loaderTab === "optifine" && !showUnstable
			? (activeCatalog?.stableGameVersions ?? [])
			: allMcVersions,
	);
	const mcVersions = $derived([...new Set([
		...(onlyInstalled ? [] : remoteMcVersions),
		...localVersions.map((v) => v.game),
	])].sort(compareVersions));
	let selectedMcVersion = $state("");
	let loaderItems = $state<LoaderDisplayVersion[]>([]);
	let loadingMinecraft = $state(false);
	let loadingLoader = $state(false);
	let mcLoadId = 0;
	let loaderLoadId = 0;
	let loadedLoaderKey = "";
	const groupedCache = createCatalogCache<GroupedLoaderVersions>();
	const gameCache = createCatalogCache<string[]>();
	const remoteLoaderCache = {
		fabric: createCatalogCache<LoaderDisplayVersion[]>(),
		quilt: createCatalogCache<LoaderDisplayVersion[]>(),
	};
	const loading = $derived(onlyInstalled ? versionsState.loading : (
		loaderTab === "vanilla"
			? loadingMojang || versionsState.loading
			: loadingMinecraft || loadingLoader || versionsState.loading),
	);

	function formatSourceError(error: unknown): string {
		const message = String(error);
		try {
			const parsed = JSON.parse(message);
			if (typeof parsed.code === "string") {
				return t(`errors.${parsed.code}`, parsed.params ?? {});
			}
		} catch {
			// Network/runtime errors may not use the backend's JSON error format.
		}
		return message;
	}

	async function switchTab(tab: string, preferredMc = "", refresh = false) {
		if (tab === loaderTab && !refresh) return;
		if (loaderTab !== tab) search = "";
		loaderTab = tab;
		sourceError = null;
		const request = ++mcLoadId;
		++loaderLoadId;
		if (tab === "vanilla") {
			if (!vanillaCache && !loadingMojang) await loadVanilla();
			return;
		}
		loadingMinecraft = true;
		loadingLoader = true;
		allMcVersions = [];
		activeCatalog = null;
		loaderItems = [];
		loadedLoaderKey = "";
		selectedMcVersion = "";
		try {
			await loadInstalledVersions();
			if (request !== mcLoadId) return;
			if (tab === "forge" || tab === "neoforge" || tab === "optifine") {
				const catalog = await groupedCache.get(
					tab,
					async () =>
						catalogToGrouped(
							await getLoaderVersionCatalog(tab, refresh),
						),
					refresh,
				);
				if (request !== mcLoadId) return;
				activeCatalog = catalog;
				allMcVersions = catalog.gameVersions;
			} else {
				const mcList = await gameCache.get(
					tab,
					async () => {
						const list =
							tab === "fabric"
								? await getFabricVersions()
								: await getQuiltVersions();
						return [
							...new SvelteSet(
								list
									.filter((v) => v.stable && v.version)
									.map((v) => v.version),
							),
						].sort(compareVersions);
					},
					refresh,
				);
				if (request !== mcLoadId) return;
				allMcVersions = mcList;
			}
			if (request !== mcLoadId) return;
			if (mcVersions.length) {
				const preferred = selectedMcVersion || preferredMc;
				selectedMcVersion = mcVersions.includes(preferred)
					? preferred
					: mcVersions[0];
				if (!onlyInstalled) await loadLoaderVersions(selectedMcVersion, tab);
				else loadingLoader = false;
			} else loadingLoader = false;
		} catch (error) {
			if (request !== mcLoadId) return;
			sourceError = formatSourceError(error);
			loadingLoader = false;
		} finally {
			if (request === mcLoadId) loadingMinecraft = false;
		}
	}

	async function loadLoaderVersions(mc: string, loader: string) {
		loadedLoaderKey = `${loader}:${mc}`;
		const request = ++loaderLoadId;
		loadingLoader = true;
		loaderItems = [];
		sourceError = null;
		try {
			let items: LoaderDisplayVersion[] = [];
			if (loader === "fabric" || loader === "quilt") {
				items = await remoteLoaderCache[loader].get(mc, async () => {
					const list =
						loader === "fabric"
							? await getFabricLoaderVersions(mc)
							: await getQuiltLoaderVersions(mc);
					return sortLoaderVersions(
						list.map((v) => ({
							version_id: `${loader}-loader-${v.version}-${mc}`,
							display_version: v.version,
							game_version: mc,
							stable: v.stable,
						})),
					);
				});
			} else {
				items = activeCatalog?.byGame.get(mc) ?? [];
			}
			if (request !== loaderLoadId) return;
			loaderItems = items;
		} catch (error) {
			if (request === loaderLoadId)
				sourceError = formatSourceError(error);
		} finally {
			if (request === loaderLoadId) loadingLoader = false;
		}
	}

	$effect(() => {
		const tab = loaderTab;
		const local = onlyInstalled;
		if (tab === "vanilla" || (!local && loadingMinecraft)) return;
		const mc = mcVersions.includes(selectedMcVersion) ? selectedMcVersion : (mcVersions[0] ?? "");
		untrack(() => {
			selectedMcVersion = mc;
			if (!local && mc && loadedLoaderKey !== `${tab}:${mc}`)
				void loadLoaderVersions(mc, tab);
			else if (!mc) {
				loadedLoaderKey = "";
				++loaderLoadId;
				loaderItems = [];
				loadingLoader = false;
			}
		});
	});

	async function loadVanilla(refresh = false) {
		loadingMojang = true;
		try {
			vanillaCache = await (refresh
				? refreshAvailableVersions()
				: getAvailableVersions());
		} catch (error) {
			if (loaderTab === "vanilla") sourceError = formatSourceError(error);
		} finally {
			loadingMojang = false;
		}
	}

	async function refreshCurrentSource() {
		if (refreshing) return;
		refreshing = true;
		sourceError = null;
		const tab = loaderTab;
		const mc = selectedMcVersion;
		try {
			if (onlyInstalled) {
				await loadInstalledVersions(true);
				return;
			}
			if (tab === "vanilla") await loadVanilla(true);
			else {
				if (tab === "fabric" || tab === "quilt")
					remoteLoaderCache[tab].clear();
				await switchTab(tab, mc, true);
			}
			await loadInstalledVersions(true);
		} catch (error) {
			if (loaderTab === tab) sourceError = formatSourceError(error);
		} finally {
			refreshing = false;
		}
	}

	function downloadId(item: CatalogItem) {
		return item.loader
			? loaderVersionId(loaderTab, item.loader.game_version, item.loader.display_version) ?? item.id
			: item.id;
	}

	function isDownloading(id: string) {
		const task = downloads.get(id);
		return enqueueing.has(id) || isVersionDownloading(id) || !!(task && !task.done && !task.error);
	}

	async function handleDownload(item: CatalogItem) {
		const id = downloadId(item);
		if (isDownloading(id)) return;
		enqueueing.add(id);
		try {
			if (!item.loader) {
				await trackConnecting(item.id, () => addToQueue(item.id));
			} else {
				const { game_version: mc, display_version: version } =
					item.loader;
				await trackConnecting(
					id,
					async () => {
						if (loaderTab === "fabric") await downloadFabric(mc, version);
						else if (loaderTab === "quilt")
							await downloadQuilt(mc, version);
						else if (loaderTab === "forge")
							await downloadForge(mc, version);
						else if (loaderTab === "neoforge")
							await downloadNeoForge(mc, version);
						else if (loaderTab === "optifine")
							await downloadOptiFine(mc, version);
					},
				);
			}
			await loadInstalledVersions(true);
		} catch {
			// The API displays download errors and the button becomes available for retry.
		} finally {
			enqueueing.delete(id);
		}
	}

	async function handleDelete(item: CatalogItem) {
		if (deleting.has(item.id)) return;
		deleting.add(item.id);
		deleteError = "";
		try {
			if (!(await ask(t("versionDownloader.deleteConfirm", { version: item.id }), {
				title: t("versionDownloader.deleteVersion"), kind: "warning",
				okLabel: t("versionDownloader.deleteVersion"), cancelLabel: t("common.cancel"),
			}))) return;
			await invoke("delete_installed_version", { versionId: item.id });
			await loadInstalledVersions(true);
		} catch (error) {
			deleteError = formatSourceError(error);
		} finally {
			deleting.delete(item.id);
		}
	}

	const mcVersionOptions = $derived(
		mcVersions.map((v) => ({ value: v, label: v })),
	);
	const normalizedSearch = $derived(
		search.trim().toLowerCase().replaceAll("_", " "),
	);
	const catalogItems = $derived.by((): CatalogItem[] => {
		if (loaderTab === "vanilla") {
			return (vanillaCache ?? [])
				.filter((v) => {
					if (!showSnapshots && v.type === "snapshot") return false;
					return (
						launcherStore.settings.show_alpha ||
						(v.type !== "old_alpha" && v.type !== "old_beta")
					);
				})
				.map((v) => {
					const subtitle = new Date(
						v.releaseTime,
					).toLocaleDateString();
					return {
						id: v.id,
						title: v.id,
						subtitle,
						stable: v.type === "release",
						searchText: `${v.id} ${subtitle}`.toLowerCase(),
						badge: t(
							`versionDownloader.${v.type === "release" ? "stable" : v.type === "snapshot" ? "preview" : "historical"}`,
						),
					};
				});
		}
		return loaderItems
			.filter((item) => showUnstable || item.stable)
			.map((item) => ({
				id: item.version_id,
				title: item.display_version.replaceAll("_", " "),
				subtitle: `Minecraft ${item.game_version}`,
				stable: item.stable,
				searchText:
					`${item.display_version.replaceAll("_", " ")} Minecraft ${item.game_version}`.toLowerCase(),
				loader: item,
				badge: t(
					`versionDownloader.${item.stable ? (isForge ? "recommended" : "stable") : isForge ? "otherBuild" : "preview"}`,
				),
			}));
	});
	const visibleItems = $derived.by(() => {
		const remote = new Map(catalogItems.map((item) => [item.id, item]));
		const items = onlyInstalled ? new Map<string, CatalogItem>() : remote;
		for (const v of localVersions) {
			if (v.loader !== "vanilla" && v.game !== selectedMcVersion) continue;
			const title = v.version.replaceAll("_", " ");
			const subtitle = `Minecraft ${v.game}`;
			items.set(v.id, remote.get(v.id) ?? {
				id: v.id, title, subtitle, badge: "", stable: false,
				searchText: `${title} ${subtitle}`.toLowerCase(),
			});
		}
		return [...items.values()];
	});
	const filteredItems = $derived(
		visibleItems.filter((item) => item.searchText.includes(normalizedSearch)),
	);
	const installedVersions = $derived(
		new Set(versionsState.rawVersions ?? []),
	);
	const installedCount = $derived(
		filteredItems.reduce(
			(count, item) => count + Number(installedVersions.has(item.id)),
			0,
		),
	);

	onMount(() => {
		loadInstalledVersions();
		loadVanilla();
	});
</script>

<ModalBase bind:open title={t("versionDownloader.title")} width="800px">
	{#snippet headerActions()}
		<button
			type="button"
			class="refresh-btn"
			onclick={refreshCurrentSource}
			disabled={refreshing || loading}
			aria-label={t("versionDownloader.refreshBtn")}
			title={t("versionDownloader.refreshBtn")}
		>
			<span class:spin={refreshing}
				><Icon name="ui:refresh" size={17} /></span
			>
		</button>
	{/snippet}
	<div class="version-catalog">
		<VersionDownloaderTabs
			{loaderTab}
			{LOADERS}
			onswitch={switchTab}
			idPrefix={id}
			panelId={`${id}-panel`}
		/>
		<div
			class="catalog-panel"
			id={`${id}-panel`}
			role="tabpanel"
			aria-labelledby={`${id}-${loaderTab}`}
			tabindex="0"
		>
			<div class="source-heading">
				<div class="source-icon">
					<Icon name={activeLoader.iconName} size={38} />
				</div>
				<div>
					<h2>{activeLoader.label}</h2>
					<p>{t(`versionDownloader.descriptions.${loaderTab}`)}</p>
				</div>
			</div>

			<div class="catalog-filters">
				<label class="installed-filter" for={`${id}-installed`}>
					<ToggleSwitch id={`${id}-installed`} bind:checked={onlyInstalled} />
					{t("versionDownloader.onlyInstalled")}
				</label>
				<div class="filter-row">
					{#if loaderTab !== "vanilla"}
						<div
							class="mc-filter"
							role="group"
							aria-labelledby={`${id}-mc-label`}
						>
							<span class="filter-label" id={`${id}-mc-label`}
								>Minecraft</span
							>
							<Select
								bind:value={selectedMcVersion}
								options={mcVersionOptions}
								placeholder={t(
									"createInstance.selectMcVersion",
								)}
								loading={!onlyInstalled && loadingMinecraft}
								loadingPlaceholder={t("createInstance.loading")}
								disabled={(!onlyInstalled && loadingMinecraft) ||
									!mcVersions.length}
							/>
						</div>
					{/if}
					<label class="search-filter">
						<span class="filter-label"
							>{t("versionDownloader.searchLabel")}</span
						>
						<span class="search-field">
							<Icon name="ui:search" size={16} />
							<input
								type="search"
								bind:value={search}
								placeholder={t(
									"versionDownloader.searchPlaceholder",
								)}
							/>
						</span>
					</label>
				</div>
			</div>

			{#if deleteError}<p class="error-detail" role="alert">{deleteError}</p>{/if}
			<div class="results-heading" aria-live="polite">
				<span>{t("versionDownloader.availableVersions")}</span>
				{#if !loading && !sourceError}
					<span class="result-count">{filteredItems.length}</span>
					{#if installedCount > 0}<span class="installed-count"
							>{`${installedCount} ${t(installedCount === 1 ? "versionDownloader.installedCountSingular" : "versionDownloader.installedCount")}`}</span
						>{/if}
				{/if}
			</div>

			<div class="catalog-results" aria-busy={loading}>
				{#if sourceError && !onlyInstalled && !filteredItems.length}
					<div class="empty-state" role="alert">
						<Icon name="ui:error" size={30} />
						<strong>{t("versionDownloader.loadError")}</strong>
						<p class="error-detail">{sourceError}</p>
						<button
							type="button"
							class="secondary-btn"
							onclick={refreshCurrentSource}
							disabled={refreshing}
							>{t("versionDownloader.retry")}</button
						>
					</div>
				{:else if loading}
					<div class="empty-state" role="status">
						<Loading />
						<p>{t("versionDownloader.loading")}</p>
					</div>
				{:else if !filteredItems.length}
					<div class="empty-state">
						<Icon name="ui:search" size={30} />
						<strong>{t("versionDownloader.notFound")}</strong>
						<p>{t("versionDownloader.emptyHint")}</p>
						{#if search}<button
								type="button"
								class="secondary-btn"
								onclick={() => (search = "")}
								>{t("versionDownloader.clearSearch")}</button
							>{/if}
					</div>
				{:else}
					{#key `${loaderTab}:${selectedMcVersion}:${onlyInstalled}:${normalizedSearch}:${showSnapshots}:${showUnstable}:${launcherStore.settings.show_alpha}`}
						<VirtualList
							items={filteredItems}
							itemHeight={78}
							itemHeightVar="--version-row-height"
							keyFn={(item) => item.id}
							class="catalog-list"
							padding={0}
							hideScrollbar={false}
						>
							{#snippet children(item)}
								{@const installed = installedVersions.has(
									item.id,
								)}
								{@const downloading = isDownloading(downloadId(item))}
								<div class="version-row" class:installed>
									<div class="row-icon">
										<Icon
											name={activeLoader.iconName}
											size={23}
										/>
									</div>
									<div class="version-info">
										<div class="version-title">
											<strong title={item.title}
												>{item.title}</strong
											>
											{#if item.badge}<span
												class="version-badge"
												class:stable={item.stable}
												>{item.badge}</span>{/if}
										</div>
										<span class="version-subtitle"
											>{item.subtitle}</span
										>
									</div>
									{#if installed}
										<span class="installed-status"
											><Icon
												name="ui:check-circle"
												size={16}
											/><span
												>{t(
													"versionDownloader.installedTag",
												)}</span
											></span
										>
										<button type="button" class="secondary-btn delete-btn" disabled={downloading || deleting.has(item.id)}
											onclick={() => void handleDelete(item)} title={t("versionDownloader.deleteVersion")}
											aria-label={`${t("versionDownloader.deleteVersion")} ${item.id}`}>
											<Icon name="ui:trash" size={16} />
										</button>
									{:else if downloading}
										<button
											type="button"
											class="download-btn"
											disabled
											><Loading size={12} />{t(
												"versionDownloader.downloading",
											)}</button
										>
									{:else}
										<button
											type="button"
											class="download-btn"
											onclick={() => handleDownload(item)}
											aria-label={`${t("versionDownloader.downloadBtn")} ${activeLoader.label} ${item.title}`}
										>
											<Icon
												name="ui:download"
												size={15}
											/>{t(
												"versionDownloader.downloadBtn",
											)}
										</button>
									{/if}
								</div>
							{/snippet}
						</VirtualList>
					{/key}
				{/if}
			</div>
			<div class="catalog-footer">
				<Icon name="instance:check-square" size={15} /><span
					>{t("versionDownloader.installHint")}</span
				>
			</div>
		</div>
	</div>
</ModalBase>

<style>
	.installed-filter {
		display: flex;
		align-items: center;
		gap: 8px;
		width: fit-content;
		cursor: pointer;
		font-size: 0.82rem;
	}
	.version-catalog {
		display: flex;
		flex-direction: column;
		height: min(640px, 72vh);
		min-height: 0;
		gap: 22px;
	}
	.catalog-panel {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 0;
		gap: 18px;
	}
	.catalog-panel:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 4px;
		border-radius: var(--border-radius-sm);
	}
	.refresh-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
	}
	.refresh-btn:hover:not(:disabled) {
		color: var(--text-primary);
		background: var(--surface-hover);
	}
	.refresh-btn:disabled {
		opacity: 0.45;
		cursor: default;
	}
	.refresh-btn > span {
		display: flex;
	}
	.source-heading {
		display: flex;
		align-items: center;
		gap: 16px;
		flex-shrink: 0;
	}
	.source-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 64px;
		height: 64px;
		flex-shrink: 0;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: 16px;
	}
	h2 {
		margin: 0 0 5px;
		color: var(--text-primary);
		font-size: 1.3rem;
		font-weight: 700;
	}
	.source-heading p {
		margin: 0;
		color: var(--text-muted);
		font-size: 0.8rem;
		line-height: 1.5;
	}
	.catalog-filters {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 16px;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--border-radius);
		flex-shrink: 0;
	}
	.filter-row {
		display: flex;
		gap: 14px;
		align-items: flex-end;
	}
	.mc-filter {
		width: 190px;
		flex-shrink: 0;
	}
	.filter-label {
		display: block;
		color: var(--text-secondary);
		font-size: 0.72rem;
		font-weight: 600;
		margin-bottom: 7px;
	}
	.search-filter {
		flex: 1;
		min-width: 0;
	}
	.search-field {
		display: flex;
		align-items: center;
		gap: 9px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: var(--bg-input);
		color: var(--text-muted);
		padding: 0 12px;
		height: 38px;
	}
	.search-field:focus-within {
		border-color: var(--accent);
		outline: 1px solid var(--accent);
	}
	.search-field input {
		min-width: 0;
		width: 100%;
		height: 100%;
		border: 0;
		outline: none;
		background: transparent;
		color: var(--text-primary);
		font: inherit;
		font-size: 0.8rem;
	}
	.search-field input::placeholder {
		color: var(--text-muted);
	}
	.results-heading {
		display: flex;
		align-items: center;
		gap: 9px;
		flex-shrink: 0;
		font-size: 0.66rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--text-muted);
	}
	.result-count {
		padding: 3px 7px;
		border-radius: 6px;
		background: var(--surface-hover);
		color: var(--text-secondary);
		letter-spacing: normal;
	}
	.installed-count {
		margin-left: auto;
		letter-spacing: normal;
		text-transform: none;
		font-weight: 400;
	}
	.catalog-results {
		display: flex;
		flex-direction: column;
		min-height: 100px;
		flex: 1;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: var(--border-radius);
		background: var(--bg-card);
	}
	:global(.catalog-list) {
		flex: 1;
		min-height: 0;
	}
	.version-row {
		display: flex;
		align-items: center;
		gap: 12px;
		height: var(--virtual-row-height, var(--version-row-height, 78px));
		box-sizing: border-box;
		padding: 12px 16px;
		border-bottom: 1px solid var(--border);
		transition: background-color 0.15s;
	}
	.version-row:hover {
		background: var(--surface-hover);
	}
	.row-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 38px;
		height: 38px;
		flex-shrink: 0;
		border-radius: 10px;
		background: var(--surface-input);
	}
	.version-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 5px;
	}
	.version-title {
		display: flex;
		align-items: center;
		gap: 9px;
		min-width: 0;
	}
	.version-title strong {
		font-size: 0.84rem;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--text-primary);
	}
	.version-subtitle {
		color: var(--text-muted);
		font-size: 0.7rem;
	}
	.version-badge {
		font-size: 0.6rem;
		font-weight: 500;
		color: var(--text-muted);
		border: 1px solid var(--border);
		border-radius: 5px;
		padding: 2px 6px;
		white-space: nowrap;
	}
	.version-badge.stable {
		color: var(--accent);
		background: rgba(var(--accent-rgb), 0.07);
		border-color: rgba(var(--accent-rgb), 0.2);
	}
	.download-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 7px;
		flex-shrink: 0;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: var(--surface-input);
		color: var(--text-primary);
		padding: 8px 11px;
		font: inherit;
		font-size: 0.72rem;
		font-weight: 600;
		cursor: pointer;
		transition:
			background-color 0.15s,
			border-color 0.15s;
	}
	.download-btn:hover:not(:disabled) {
		background: var(--accent);
		color: var(--accent-text);
		border-color: var(--accent);
	}
	.download-btn:disabled {
		color: var(--text-muted);
		background: transparent;
		cursor: default;
	}
	.installed-status {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--color-version-installed);
		font-size: 0.72rem;
		flex-shrink: 0;
	}
	.installed .row-icon {
		opacity: 0.65;
	}
	.empty-state {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		overflow-y: auto;
		padding: 22px;
		text-align: center;
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.empty-state strong {
		color: var(--text-secondary);
		font-weight: 600;
	}
	.empty-state p {
		margin: 0;
		line-height: 1.5;
	}
	.error-detail {
		overflow-wrap: anywhere;
		font-size: 0.7rem;
	}
	.secondary-btn {
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: var(--surface-input);
		color: var(--text-primary);
		padding: 7px 12px;
		cursor: pointer;
		font: inherit;
		font-size: 0.75rem;
	}
	.secondary-btn.delete-btn {
		display: grid;
		place-items: center;
		flex-shrink: 0;
		padding: 7px;
		color: var(--color-error, #ef4444);
	}
	.delete-btn:disabled { opacity: 0.5; cursor: default; }
	.catalog-footer {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		flex-shrink: 0;
		color: var(--text-muted);
		font-size: 0.7rem;
		line-height: 1.4;
	}
	.spin {
		animation: spin 0.8s linear infinite;
	}
	button:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.spin {
			animation-duration: 2s;
		}
	}
	@media (max-width: 600px) {
		.version-catalog {
			gap: 14px;
			height: 72vh;
		}
		.catalog-panel {
			gap: 12px;
		}
		.source-icon {
			width: 50px;
			height: 50px;
			border-radius: 12px;
		}
		.source-heading {
			gap: 12px;
		}
		h2 {
			font-size: 1.1rem;
		}
		.source-heading p {
			font-size: 0.72rem;
		}
		.catalog-filters {
			padding: 12px;
		}
		.mc-filter {
			width: 130px;
		}
		.filter-row {
			gap: 10px;
		}
		.version-row {
			padding: 10px;
			gap: 8px;
		}
		.row-icon {
			display: none;
		}
		.version-title {
			flex-wrap: wrap;
			gap: 4px 8px;
		}
		.version-title strong {
			max-width: 100%;
		}
		.download-btn {
			padding: 8px;
		}
		.catalog-footer {
			font-size: 0.65rem;
		}
	}
	@media (max-width: 380px) {
		.filter-row {
			flex-direction: column;
			align-items: stretch;
		}
		.mc-filter {
			width: 100%;
		}
	}
</style>
