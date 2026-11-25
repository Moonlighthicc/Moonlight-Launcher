<script lang="ts">
	import { onDestroy, onMount } from "svelte";
	import { t } from "$lib/i18n";
	import {
		searchModrinth,
		getModrinthProjectVersions,
		getModrinthProject,
		getAvailableVersions,
		downloadMrpack,
		installMrpackWithUpstream,
		openUrl,
	} from "$lib/api/cubicApi";
	import { trackConnecting } from "$lib/state/downloadQueueState.svelte";
	import type {
		ModrinthProject,
		ModrinthVersion,
		MinecraftVersion,
	} from "$lib/types/types";
	import MarkdownRenderer from "$lib/components/ui/MarkdownRenderer.svelte";
	import { showErrorParsed } from "$lib/state/state.svelte";
	import { invokeMarket } from "$lib/api/marketRequest";
	import {
		releaseChannel,
		packVersionLabel,
		type PackLoaders,
		type PackGalleryImage,
	} from "$lib/utils/modpackVersions";
	import Loading from "$lib/icons/Loading.svelte";
	import ModpackBrowser, {
		type ModpackItem,
		type ModpackFilters,
	} from "./ModpackBrowser.svelte";
	import PackDescriptionError from "./PackDescriptionError.svelte";
	import { packLoaders, versionPackLoaders } from "$lib/utils/modpackLoaders";

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
		{ value: "adventure", label: "Adventure" },
		{ value: "magic", label: "Magic" },
		{ value: "utility", label: "Utility" },
		{ value: "optimization", label: "Optimization" },
		{ value: "equipment", label: "Equipment" },
		{ value: "worldgen", label: "Worldgen" },
		{ value: "food", label: "Food" },
		{ value: "library", label: "Library" },
		{ value: "decoration", label: "Decoration" },
		{ value: "storage", label: "Storage" },
	];

	let filters = $state<ModpackFilters>({
		sort: "downloads",
		category: null,
		gameVersion: null,
		loader: null,
	});
	let gameVersions = $state<MinecraftVersion[]>([]);
	let query = $state("");
	let projects = $state<ModrinthProject[]>([]);
	let items = $state<ModpackItem[]>([]);
	let totalHits = $state(0);
	let searching = $state(false);
	let loadingMore = $state(false);
	let offset = $state(0);
	let selectedItem = $state<ModpackItem | null>(null);
	let selectedPack = $state<ModrinthProject | null>(null);
	let versions = $state<ModrinthVersion[]>([]);
	let loadingVersions = $state(false);
	let installing = $state(false);
	let installTask = $state<{ archive: string; name: string }>();
	let installError = $state<string | null>(null);
	let browser = $state<ReturnType<typeof ModpackBrowser>>();

	let gallery = $state<PackGalleryImage[]>([]);
	let fullProject = $state<string>("");
	let loadingFullProject = $state(false);
	let descriptionError = $state<string | null>(null);

	let searchGen = 0;
	let alive = true;
	let detailRequest: AbortController | undefined;
	let searchRequest: AbortController | undefined;
	onDestroy(() => {
		alive = false;
		searchGen++;
		detailRequest?.abort();
		searchRequest?.abort();
	});

	const versionOptions = $derived(
		versions
			.filter((v) =>
				v.files.some((file) =>
					file.filename.toLowerCase().endsWith(".mrpack"),
				),
			)
			.map((v) => ({
				value: v.id,
				label: packVersionLabel(v.name || v.version_number, v.game_versions[0]),
				date: v.date_published,
				url: readmeBaseUrl ? `${readmeBaseUrl}/version/${encodeURIComponent(v.id)}` : undefined,
				channel: releaseChannel(v.version_type),
				packLoaders: versionPackLoaders(v.loaders ?? [], ...v.files
					.filter((file) => file.filename.toLowerCase().endsWith(".mrpack"))
					.map((file) => file.filename)),
			})),
	);

	const readmeBaseUrl = $derived(
		selectedPack?.slug
			? `https://modrinth.com/modpack/${encodeURIComponent(selectedPack.slug)}`
			: undefined,
	);

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

	function projectToItem(pack: ModrinthProject): ModpackItem {
		return {
			id: pack.project_id,
			title: pack.title,
			description: pack.description,
			iconUrl: pack.icon_url,
			categories: pack.categories.filter((category) => !packLoaders([category]).length),
			packLoaders: packLoaders(pack.categories),
			downloads: pack.downloads,
			author: pack.author,
		};
	}

	async function doSearch(reset?: boolean) {
		if (!reset && (searching || loadingMore)) return;

		const gen = ++searchGen;
		searchRequest?.abort();
		searchRequest = new AbortController();

		if (reset) {
			searching = true;
		} else {
			loadingMore = true;
		}
		installError = null;
		if (reset) {
			offset = 0;
			projects = [];
			items = [];
			fullProject = "";
		}
		try {
			const result = await searchModrinth(
				query,
				filters.loader ?? "",
				filters.gameVersion ?? undefined,
				filters.category,
				filters.sort,
				limit,
				reset ? 0 : offset,
				"modpack",
				searchRequest.signal,
			);
			if (gen !== searchGen) return;
			if (result) {
				projects = reset ? result.hits : [...projects, ...result.hits];
				items = projects.map(projectToItem);
				totalHits = result.total_hits;
				offset = reset ? result.limit : offset + result.limit;
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
		fullProject = "";
		selectedPack = projects.find((p) => p.project_id === item.id) ?? null;

		if (!selectedPack) {
			loadingVersions = false;
			loadingFullProject = false;
			return;
		}

		const request = new AbortController();
		detailRequest = request;
		void loadDescription();
		try {
			const fetchedVersions = await getModrinthProjectVersions(
				selectedPack.project_id, undefined, undefined, request.signal,
			);
			if (!alive || request.signal.aborted) return;
			versions = fetchedVersions;
			if (selectedItem) selectedItem = {
				...selectedItem,
				packLoaders: packLoaders([
					...(selectedItem.packLoaders ?? []), ...fetchedVersions.flatMap((version) => version.loaders ?? []),
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
		const projectId = selectedPack?.project_id;
		if (!signal || signal.aborted || !projectId || loadingFullProject) return;
		loadingFullProject = true;
		descriptionError = null;
		try {
			const project = await getModrinthProject(projectId, signal);
			if (!alive || signal.aborted) return;
			if (!project) throw new Error("Modrinth returned no project data");
			fullProject = project.body ?? "";
			gallery = (project.gallery ?? []).map((image) => ({
				...image,
				url: image.raw_url || image.url,
				thumbnail: image.url,
			}));
			if (selectedItem)
				selectedItem = {
					...selectedItem,
					categories: project.categories,
					downloads: project.downloads,
					packLoaders: packLoaders([...(selectedItem.packLoaders ?? []), ...(project.loaders ?? [])]),
				};
		} catch (err) {
			if (alive && !signal.aborted) descriptionError = String(err);
		} finally {
			if (alive && !signal.aborted) loadingFullProject = false;
		}
	}

	function handleBack() {
		detailRequest?.abort();
		detailRequest = undefined;
		loadingVersions = false;
		loadingFullProject = false;
		descriptionError = null;
		selectedItem = null;
		selectedPack = null;
		gallery = [];
		versions = [];
		fullProject = "";
	}

	export function goBackOnEscape() {
		return installing || (browser?.goBackOnEscape() ?? false);
	}

	async function doInstall(versionId: string, name: string) {
		if (!selectedPack || !versionId || loadingVersions || installing)
			return;
		installing = true;
		installTask = { archive: `mrpack-dl-${versionId}`, name };
		installError = null;
		const pack = selectedPack;
		onInstallStarted?.(name);
		try {
			const ver = versions.find((v) => v.id === versionId);
			if (!ver) throw new Error("Version not found");
			const archiveFiles = ver.files.filter((file) =>
				file.filename.toLowerCase().endsWith(".mrpack"),
			);
			const primaryFile =
				archiveFiles.find((f) => f.primary) ?? archiveFiles[0];
			if (!primaryFile) throw new Error("No file found in version");

			const mrpackPath = await trackConnecting(
				`mrpack-dl-${versionId}`,
				() => downloadMrpack(primaryFile.url, versionId),
			);
			if (!mrpackPath) throw new Error(t("modpack.downloadFailed"));

			const result = await installMrpackWithUpstream(
				mrpackPath,
				name,
				pack.project_id,
				versionId,
				pack.icon_url ?? undefined,
				undefined,
				(err) => {
					installError = String(err);
				},
			);
			if (!result && !installError) {
				installError = t("modpack.installFailed");
			}
		} catch (e) {
			installError = String(e);
			showErrorParsed(e);
		} finally {
			installing = false;
			installTask = undefined;
			if (installError) onInstallFailed?.(name);
			else onInstalled?.();
		}
	}

	const loaders: PackLoaders = {
		format: "markdown",
		changelog(version, signal) {
			return invokeMarket<string>(
				"get_modrinth_pack_changelog",
				{ versionId: version },
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
	{totalHits}
	{searching}
	{loadingMore}
	bind:selectedItem
	{versionOptions}
	{loadingVersions}
	{installing}
	{installTask}
	{installError}
	{gallery}
	website={readmeBaseUrl}
	{loaders}
	bind:filters
	categoryOptions={categories}
	{gameVersionOptions}
	searchPlaceholder={t("createInstance.modpackSearchPlaceholder")}
	emptySearchingText={t("createInstance.searchingModpacks")}
	emptyNoResultsText={t("createInstance.noModpacksFound")}
	onSearch={handleSearch}
	onLoadMore={handleLoadMore}
	onFilterChange={handleFilterChange}
	onSelect={handleSelect}
	onBack={handleBack}
	onInstall={doInstall}
>
	{#snippet detailExtra(_item)}
		{#if loadingFullProject}
			<div class="readme-loading" role="status" aria-label={t("createInstance.loading")}>
				<Loading />
			</div>
		{:else if descriptionError}
			<PackDescriptionError error={descriptionError} onretry={loadDescription} />
		{:else if fullProject}
			<div class="detail-readme">
				<div class="readme-content">
					<MarkdownRenderer
						source={fullProject}
						baseUrl={readmeBaseUrl}
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
