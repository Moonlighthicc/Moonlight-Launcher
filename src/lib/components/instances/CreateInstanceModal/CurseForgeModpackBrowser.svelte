<script lang="ts">
	import { onDestroy, onMount } from "svelte";
	import { t } from "$lib/i18n";
	import {
		searchCurseForgeModpacks,
		getCurseForgeProjectDescription,
		getCurseForgeFileDownloadUrl,
		getAvailableVersions,
		downloadCurseForgeModpack,
		installCurseForgeModpack,
		isCancelError,
		openUrl,
	} from "$lib/api/cubicApi";
	import { trackConnecting } from "$lib/state/downloadQueueState.svelte";
	import { dismissModpackInstall, modpackInstalls, retryModpackInstall, runModpackInstall } from "$lib/state/modpackInstalls.svelte";
	import type {
		CurseForgeProject,
		CurseForgeFile,
		MinecraftVersion,
	} from "$lib/types/types";
	import HtmlRenderer from "$lib/components/ui/HtmlRenderer.svelte";
	import Loading from "$lib/icons/Loading.svelte";
	import { invokeMarket } from "$lib/api/marketRequest";
	import {
		releaseChannel,
		minecraftVersion,
		packVersionLabel,
		type PackLoaders,
		type PackVersion,
		type PackGalleryImage,
	} from "$lib/utils/modpackVersions";
	import ModpackBrowser, {
		type ModpackItem,
		type ModpackFilters,
	} from "./ModpackBrowser.svelte";
	import PackDescriptionError from "./PackDescriptionError.svelte";
	import { curseForgeVersionLoaders, matchesCurseForgePack, packLoaders } from "$lib/utils/modpackLoaders";

	let {
		onInstallStarted,
		onInstallFailed,
		onInstalled,
	}: {
		onInstallStarted?: (name: string) => void;
		onInstallFailed?: (name: string) => void;
		onInstalled?: () => void;
	} = $props();

	const limit = 10;

	const categories = [
		{ value: "422", label: "Adventure" },
		{ value: "419", label: "Magic" },
		{ value: "5191", label: "Utility" },
		{ value: "6814", label: "Optimization" },
		{ value: "434", label: "Equipment" },
		{ value: "406", label: "Worldgen" },
		{ value: "436", label: "Food" },
		{ value: "421", label: "Library" },
		{ value: "424", label: "Decoration" },
		{ value: "420", label: "Storage" },
	];

	let filters = $state<ModpackFilters>({
		sort: "downloads",
		category: null,
		gameVersion: null,
		loader: null,
	});
	let gameVersions = $state<MinecraftVersion[]>([]);
	let query = $state("");
	let projects = $state<CurseForgeProject[]>([]);
	const items = $derived(projects.map(projectToItem));
	let hasMore = $state(false);
	let searching = $state(false);
	let loadingMore = $state(false);
	let offset = $state(0);
	let selectedItem = $state<ModpackItem | null>(null);
	let selectedPack = $state<CurseForgeProject | null>(null);
	let versions = $state<CurseForgeFile[]>([]);
	let loadingVersions = $state(false);
	const task = $derived(selectedItem ? modpackInstalls.get(`curseforge:${selectedItem.id}`) : undefined);
	let installError = $state<string | null>(null);
	let browser = $state<ReturnType<typeof ModpackBrowser>>();

	const website = $derived(
		selectedPack
			? `https://www.curseforge.com/minecraft/modpacks/${encodeURIComponent(selectedPack.slug)}`
			: undefined,
	);
	const gallery = $derived<PackGalleryImage[]>(
		(selectedPack?.screenshots ?? []).map((image) => ({
			...image,
			thumbnail: image.thumbnailUrl,
		})),
	);
	let description = $state<string>("");
	let loadingDescription = $state(false);
	let descriptionError = $state<string | null>(null);

	let searchGen = 0;
	let alive = true;
	let detailRequest: AbortController | undefined;
	type ManifestFile = { fileId: number; fileName: string };
	type ManifestMetadata = { gameVersion: string; loader: string | null };
	let manifestMetadata = $state<Record<number, ManifestMetadata>>({});
	let searchLoaders = $state<Record<number, string>>({});
	let searchManifests: {
		controller: AbortController;
		queued: Set<number>;
		jobs: { projectId: number; file: ManifestFile }[];
		active: number;
	} | undefined;
	const manifestCache = new Map<number, ManifestMetadata>();
	let manifestRequests = new Set<number>();
	let manifestQueue: number[] = [];
	let manifestsInFlight = 0;
	onDestroy(() => {
		alive = false;
		searchGen++;
		detailRequest?.abort();
		searchManifests?.controller.abort();
	});

	const versionOptions = $derived(
		versions
			.filter((v) => v.isAvailable && !v.isServerPack)
			.map((v) => {
				const metadata = manifestMetadata[v.id];
				const gameVersion = metadata?.gameVersion || minecraftVersion([
					...v.gameVersions,
					...(v.sortableGameVersions ?? []).flatMap((entry) => [entry.gameVersion, entry.gameVersionName]),
				]);
				return {
					value: String(v.id),
					label: packVersionLabel(v.displayName || v.fileName, gameVersion),
					date: v.fileDate,
					url: website ? `${website}/files/${v.id}` : undefined,
					channel: releaseChannel(v.releaseType),
					packLoaders: metadata
						? packLoaders([metadata.loader])
						: curseForgeVersionLoaders(v, selectedPack?.latestFilesIndexes, selectedPack?.latestFiles),
				};
			}),
	);

	async function readManifest(file: ManifestFile, signal: AbortSignal) {
		const cached = manifestCache.get(file.fileId);
		if (cached) return cached;
		const metadata = await invokeMarket<ManifestMetadata>("get_curseforge_pack_manifest", file, signal);
		if (manifestCache.size >= 256) manifestCache.clear();
		manifestCache.set(file.fileId, metadata);
		return metadata;
	}

	function loadVisibleManifests(visible: PackVersion[]) {
		const request = detailRequest;
		if (!request || request.signal.aborted) return;
		manifestQueue = [...new Set(visible.filter((version) => !version.packLoaders?.length)
			.map((version) => Number(version.value)))].filter((id) => !manifestRequests.has(id));
		drainManifestQueue(request);
	}

	function drainManifestQueue(request: AbortController) {
		while (!request.signal.aborted && manifestQueue.length && manifestsInFlight < 3) {
			const fileId = manifestQueue.shift()!;
			manifestRequests.add(fileId);
			const fileName = versions.find((file) => file.id === fileId)?.fileName;
			if (!fileName) continue;
			manifestsInFlight++;
			void readManifest({ fileId, fileName }, request.signal)
				.then((metadata) => {
					if (request.signal.aborted) return;
					manifestMetadata[fileId] = metadata;
				})
				.catch(() => { /* A missing manifest leaves the loader unknown; browsing still works. */ })
				.finally(() => {
					if (request.signal.aborted) return;
					manifestsInFlight--;
					drainManifestQueue(request);
				});
		}
	}

	const gameVersionOptions = $derived(
		gameVersions
			.filter((v) => v.type === "release")
			.map((v) => ({ value: v.id, label: v.id })),
	);

	async function loadGameVersions() {
		try {
			const result = await getAvailableVersions();
			if (alive) gameVersions = result;
		} catch {
			if (alive) gameVersions = [];
		}
	}

	function handleFilterChange() {
		handleBack();
		doSearch(true);
	}

	function projectToItem(pack: CurseForgeProject): ModpackItem {
		return {
			id: pack.id,
			title: pack.name,
			description: pack.summary,
			iconUrl: pack.logo?.url ?? null,
			categories: pack.categories.map((category) => category.name),
			packLoaders: packLoaders([
				searchLoaders[pack.id],
				...(pack.latestFilesIndexes ?? []).map((file) => file.modLoader),
				...(pack.latestFiles ?? []).filter((file) => !file.isServerPack).flatMap((file) => curseForgeVersionLoaders(file, pack.latestFilesIndexes)),
			]),
			downloads: Number(pack.downloadCount),
			author: pack.authors.map((a) => a.name).join(", ") || undefined,
		};
	}

	function searchManifestFile(pack: CurseForgeProject): ManifestFile | undefined {
		const latest = pack.latestFiles ?? [];
		const candidates = [
			...(pack.latestFilesIndexes ?? []).filter((index) =>
				!latest.some((file) => file.id === index.fileId && (file.isServerPack || !file.isAvailable)),
			).map((index) => ({ fileId: index.fileId, fileName: index.filename, releaseType: index.releaseType })),
			...latest.filter((file) => file.isAvailable && !file.isServerPack)
				.map((file) => ({ fileId: file.id, fileName: file.fileName, releaseType: file.releaseType })),
		].filter((file) => file.fileName?.toLowerCase().endsWith(".zip"));
		return candidates.find((file) => file.fileId === pack.mainFileId) ??
			candidates.sort((a, b) => a.releaseType - b.releaseType || b.fileId - a.fileId)[0];
	}

	function drainSearchManifests() {
		const batch = searchManifests;
		if (!batch) return;
		const { signal } = batch.controller;
		while (!signal.aborted && batch.active < 3 && batch.jobs.length) {
			const { projectId, file } = batch.jobs.shift()!;
			batch.active++;
			void readManifest({ fileId: file.fileId, fileName: file.fileName }, signal)
				.then(({ loader }) => {
					if (!signal.aborted && loader) searchLoaders[projectId] = loader;
				})
				.catch(() => { /* Leave the badge unset when the manifest is unavailable. */ })
				.finally(() => {
					batch.active--;
					if (!signal.aborted) drainSearchManifests();
				});
		}
	}

	function queueSearchManifests(packs: CurseForgeProject[]) {
		const batch = searchManifests;
		if (!batch) return;
		for (const pack of packs) {
			if (batch.queued.has(pack.id) || projectToItem(pack).packLoaders?.length) continue;
			const file = searchManifestFile(pack);
			if (!file) continue;
			batch.queued.add(pack.id);
			batch.jobs.push({ projectId: pack.id, file });
		}
		drainSearchManifests();
	}

	async function doSearch(reset?: boolean) {
		if (!reset && (searching || loadingMore)) return;

		const gen = ++searchGen;
		const { loader, gameVersion, category, sort } = filters;
		const searchQuery = query;
		const filtered = !!(loader || gameVersion);

		if (reset) {
			searching = true;
		} else {
			loadingMore = true;
		}
		installError = null;
		if (reset) {
			offset = 0;
			hasMore = false;
			projects = [];
			searchManifests?.controller.abort();
			searchManifests = { controller: new AbortController(), queued: new Set(), jobs: [], active: 0 };
			searchLoaders = {};
			description = "";
		}
		try {
			// CurseForge's modpack endpoint ignores loader/version parameters in our backend.
			// Scan bounded batches of catalog pages; track the raw offset, not the matches.
			const pageSize = filtered ? 50 : limit;
			for (let page = 0; page < (filtered ? 4 : 1); page++) {
				const size = Math.min(pageSize, 10_000 - offset);
				if (size <= 0) break;
				const result = await searchCurseForgeModpacks(searchQuery, "", undefined, category, sort, size, offset);
				if (!alive || gen !== searchGen || !result) return;
				const matches = result.data.filter((pack) => matchesCurseForgePack(pack, loader, gameVersion));
				projects = [...projects, ...matches];
				queueSearchManifests(matches);
				offset += result.pagination.pageSize;
				hasMore = result.data.length > 0 && offset < Math.min(10_000, result.pagination.totalCount);
				if (!hasMore || matches.length > 0) break;
			}
		} catch (err) {
			if (alive && gen === searchGen) installError = String(err);
		} finally {
			if (gen === searchGen) {
				searching = false;
				loadingMore = false;
			}
		}
	}

	function handleSearch() {
		handleBack();
		doSearch(true);
	}

	function handleLoadMore() {
		doSearch(false);
	}

	async function handleSelect(item: ModpackItem) {
		handleBack();
		installError = null;
		selectedItem = item;
		loadingVersions = true;
		versions = [];
		description = "";
		selectedPack = projects.find((p) => p.id === item.id) ?? null;

		if (!selectedPack) {
			loadingVersions = false;
			loadingDescription = false;
			return;
		}

		const request = new AbortController();
		detailRequest = request;
		void loadDescription();
		try {
			const fetchedVersions = await invokeMarket<CurseForgeFile[]>(
				"get_curseforge_modpack_versions", { modId: selectedPack.id }, request.signal,
			);
			if (!alive || request.signal.aborted) return;
			versions = fetchedVersions;
			if (selectedItem) selectedItem = {
				...selectedItem,
				packLoaders: packLoaders([
					...(selectedItem.packLoaders ?? []),
					...fetchedVersions.filter((file) => file.isAvailable && !file.isServerPack)
						.flatMap((file) => curseForgeVersionLoaders(file, selectedPack?.latestFilesIndexes, selectedPack?.latestFiles)),
				]),
			};
		} catch (err) {
			if (alive && !request.signal.aborted) installError = String(err);
		} finally {
			if (alive && !request.signal.aborted) loadingVersions = false;
		}
	}

	async function loadDescription() {
		const signal = detailRequest?.signal;
		const modId = selectedPack?.id;
		if (!signal || signal.aborted || modId === undefined || loadingDescription) return;
		loadingDescription = true;
		descriptionError = null;
		try {
			const result = await getCurseForgeProjectDescription(modId, signal);
			if (!alive || signal.aborted) return;
			if (result === null) throw new Error("CurseForge returned no description data");
			description = result;
		} catch (err) {
			if (alive && !signal.aborted) descriptionError = String(err);
		} finally {
			if (alive && !signal.aborted) loadingDescription = false;
		}
	}

	function handleBack() {
		detailRequest?.abort();
		detailRequest = undefined;
		manifestMetadata = {};
		manifestRequests = new Set();
		manifestQueue = [];
		manifestsInFlight = 0;
		loadingVersions = false;
		loadingDescription = false;
		descriptionError = null;
		selectedItem = null;
		selectedPack = null;
		versions = [];
		description = "";
	}

	export function goBackOnEscape() {
		return browser?.goBackOnEscape() ?? false;
	}

	async function doInstall(versionId: string, name: string) {
		const pack = selectedPack;
		const file = versions.find((v) => String(v.id) === versionId);
		if (!pack || !file || loadingVersions || task) return;
		const key = `curseforge:${pack.id}`;
		const archive = `cfpack-dl-${file.id}`;
		installError = null;
		const success = await runModpackInstall(key, { archive, name, version: versionId }, async () => {
			onInstallStarted?.(name);

			let url = file.downloadUrl;
			if (!url) {
				url = await getCurseForgeFileDownloadUrl(
					pack.id,
					file.id,
					file.fileName,
				);
			}
			if (!url) throw new Error("No download URL found for this modpack");

			const packPath = await trackConnecting(archive, () =>
				downloadCurseForgeModpack(url, file.id),
			);
			const result = await installCurseForgeModpack(
				packPath,
				name,
				pack.id,
				file.id,
				pack.logo?.url ?? undefined,
				undefined,
				(err) => { throw err; },
				false,
			);
			if (!result) throw new Error(t("modpack.installFailed"));
		}, isCancelError);
		if (!success) {
			onInstallFailed?.(name);
		} else if (alive && selectedPack?.id === pack.id) {
			onInstalled?.();
		}
	}

	async function retryInstall() {
		if (!task || task.error === null || !selectedItem) return;
		const { id } = selectedItem;
		const { name } = task;
		if (!(await retryModpackInstall(`curseforge:${id}`))) onInstallFailed?.(name);
		else if (alive && selectedItem?.id === id) onInstalled?.();
	}

	function dismissInstall() {
		if (selectedItem) dismissModpackInstall(`curseforge:${selectedItem.id}`);
	}

	const loaders: PackLoaders = {
		format: "html",
		onVisibleVersions: loadVisibleManifests,
		changelog(version, signal) {
			if (!selectedPack) return Promise.resolve("");
			return invokeMarket<string>(
				"get_curseforge_pack_changelog",
				{ modId: selectedPack.id, fileId: Number(version) },
				signal,
			);
		},
	};

	onMount(() => {
		void loadGameVersions();
		void doSearch(true);
	});
</script>

<ModpackBrowser
	bind:this={browser}
	bind:query
	{items}
	{hasMore}
	manualPagination={!!(filters.loader || filters.gameVersion)}
	{searching}
	{loadingMore}
	bind:selectedItem
	{versionOptions}
	{loadingVersions}
	installing={task?.running ?? false}
	installTask={task}
	installError={task?.error ?? installError}
	{gallery}
	{website}
	{loaders}
	bind:filters
	categoryOptions={categories}
	{gameVersionOptions}
	searchPlaceholder={t("createInstance.curseforgeModpackSearchPlaceholder")}
	emptySearchingText={t("createInstance.searchingModpacks")}
	emptyNoResultsText={t("createInstance.noModpacksFound")}
	onSearch={handleSearch}
	onLoadMore={handleLoadMore}
	onFilterChange={handleFilterChange}
	onSelect={handleSelect}
	onBack={handleBack}
	onInstall={doInstall}
	onRetryInstall={retryInstall}
	onDismissInstall={dismissInstall}
>
	{#snippet detailExtra(_item)}
		{#if loadingDescription}
			<div class="readme-loading" role="status" aria-label={t("createInstance.loading")}>
				<Loading />
			</div>
		{:else if descriptionError}
			<PackDescriptionError error={descriptionError} onretry={loadDescription} />
		{:else if description}
			<div class="detail-readme">
				<div class="readme-content">
					<HtmlRenderer
						source={description}
						class="markdown-body"
						onLinkClick={openUrl}
					/>
				</div>
			</div>
		{/if}
	{/snippet}
</ModpackBrowser>

<style>
	.readme-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 30px;
	}

	.detail-readme {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.readme-content {
		font-size: 0.78rem;
		line-height: 1.5;
		color: var(--text-primary);
	}
</style>
