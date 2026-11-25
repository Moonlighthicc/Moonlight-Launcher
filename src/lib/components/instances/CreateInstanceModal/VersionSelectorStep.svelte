<script lang="ts">
	import { onMount } from "svelte";
	import { SvelteSet } from "svelte/reactivity";
	import {
		versionsState,
		isVersionInstalled,
		loadInstalledVersions,
	} from "$lib/state/versionsState.svelte";
	import Select from "$lib/components/layout/Select.svelte";
	import ToggleSwitch from "$lib/components/ui/ToggleSwitch.svelte";
	import Icon from "$lib/icons/Icon.svelte";
	import { t } from "$lib/i18n";
	import {
		getAvailableVersions,
		getFabricLoaderVersions,
		getQuiltLoaderVersions,
	} from "$lib/api/cubicApi";
	import {
		createCatalogCache,
		type GroupedLoaderVersions,
	} from "$lib/components/layout/VersionDownloader/versionCatalog";
	import { getSelectionCatalog, instanceVersionId, minecraftVersionVisible } from "./availableVersions";
	import { launcherStore } from "$lib/state/state.svelte";
	import { downloads, getOverallPct } from "$lib/state/downloadQueueState.svelte";
	import { isVersionDownloading } from "$lib/state/downloadState.svelte";
	import type { LoaderVersion } from "$lib/types/types";

	let {
		selectedLoader = $bindable<string>("vanilla"),
		selectedMcVersion = $bindable<string>(""),
		selectedLoaderVersion = $bindable<string>(""),
		compact = false,
		includeAvailable = false,
		showInstalledFilter = false,
		onDownload,
		pendingDownloads,
		downloadDisabled = false,
		loading = $bindable(false),
	}: {
		selectedLoader: string;
		selectedMcVersion: string;
		selectedLoaderVersion: string;
		compact?: boolean;
		includeAvailable?: boolean;
		showInstalledFilter?: boolean;
		onDownload?: (loader: string, mc: string, version: string) => void;
		pendingDownloads?: ReadonlySet<string>;
		downloadDisabled?: boolean;
		loading?: boolean;
	} = $props();
	const id = $props.id();
	let onlyInstalled = $state(false);
	const includeRemote = $derived(includeAvailable && !onlyInstalled);

	const catalogCache = createCatalogCache<GroupedLoaderVersions>();
	const loaderCache = createCatalogCache<LoaderVersion[]>();
	const minecraftCatalog = createCatalogCache<Awaited<ReturnType<typeof getAvailableVersions>>>();
	let minecraftTypes = $state<Map<string, string>>(new Map());
	let catalog = $state<GroupedLoaderVersions | null>(null);
	let catalogLoader = $state("");
	let remoteLoaderVersions = $state<LoaderVersion[]>([]);
	let remoteLoaderKey = $state("");
	let sourceError = $state("");
	const loadingMc = $derived(
		(versionsState.loading && !versionsState.rawVersions) ||
			(includeRemote && catalogLoader !== selectedLoader),
	);
	const loadingLoader = $derived(
		loadingMc ||
			(includeAvailable &&
				selectedLoader !== "vanilla" &&
				!!selectedMcVersion &&
				remoteLoaderKey !== `${selectedLoader}:${selectedMcVersion}`),
	);

	$effect(() => {
		loading = loadingMc || loadingLoader;
	});

	$effect(() => {
		if (!includeAvailable) return;
		const loader = selectedLoader;
		let cancelled = false;
		sourceError = "";
		catalogLoader = "";
		catalog = null;
		const manifest = minecraftCatalog.get("minecraft", getAvailableVersions);
		const typesReady = manifest.then((list) => {
			if (!cancelled) minecraftTypes = new Map(list.map((v) => [v.id, v.type]));
		}).catch(() => {});
		catalogCache
			.get(loader, async () => {
				if (loader !== "vanilla") return getSelectionCatalog(loader);
				const list = await manifest;
				return { byGame: new Map(), gameVersions: list.map((v) => v.id), stableGameVersions: [] };
			})
			.then((result) => {
				if (!cancelled) catalog = result;
			})
			.catch((error) => {
				if (!cancelled) sourceError = String(error);
			})
			.finally(async () => {
				await typesReady;
				if (!cancelled) catalogLoader = loader;
			});
		return () => {
			cancelled = true;
		};
	});

	$effect(() => {
		if (!includeAvailable || catalogLoader !== selectedLoader) return;
		const loader = selectedLoader;
		const mc = selectedMcVersion;
		const key = `${loader}:${mc}`;
		remoteLoaderKey = "";
		remoteLoaderVersions = [];
		if (!mc || loader === "vanilla") return;
		let cancelled = false;
		const groupedVersions = catalog?.byGame.get(mc)?.map((v) => ({ version: v.display_version, stable: v.stable })) ?? [];
		loaderCache
			.get(key, async () => {
				if (loader === "fabric")
					return getFabricLoaderVersions(mc);
				if (loader === "quilt")
					return getQuiltLoaderVersions(mc);
				return groupedVersions;
			})
			.then((result) => {
				if (!cancelled) remoteLoaderVersions = result;
			})
			.catch((error) => {
				if (!cancelled) sourceError = String(error);
			})
			.finally(() => {
				if (!cancelled) remoteLoaderKey = key;
			});
		return () => {
			cancelled = true;
		};
	});

	$effect(() => {
		if (selectedLoader == null || selectedLoader === "") {
			selectedLoader = "vanilla";
		}
	});

	$effect(() => {
		if (selectedMcVersion == null) {
			selectedMcVersion = "";
		}
	});

	$effect(() => {
		if (selectedLoaderVersion == null) {
			selectedLoaderVersion = "";
		}
	});

	const LOADERS = [
		{
			value: "vanilla",
			label: "Vanilla",
			iconName: "brand:vanilla",
		},
		{
			value: "fabric",
			label: "Fabric",
			iconName: "brand:fabric",
		},
		{
			value: "forge",
			label: "Forge",
			iconName: "brand:forge",
		},
		{
			value: "neoforge",
			label: "NeoForge",
			iconName: "brand:neoforged",
		},
		{
			value: "quilt",
			label: "Quilt",
			iconName: "brand:quilt",
		},
		{ value: "optifine", label: "OptiFine", iconName: "brand:optifine" },
	];

	function compareVersions(a: string, b: string): number {
		const aParts = a.split(".").map((n) => parseInt(n, 10) || 0);
		const bParts = b.split(".").map((n) => parseInt(n, 10) || 0);
		for (let i = 0; i < Math.max(aParts.length, bParts.length); i++) {
			const av = aParts[i] ?? 0;
			const bv = bParts[i] ?? 0;
			if (av !== bv) return bv - av;
		}
		return b.localeCompare(a, undefined, { numeric: true });
	}

	function getMcVersionsForLoader(loader: string): string[] {
		const parsed = versionsState.mcVersions;
		if (!parsed) return [];
		if (showInstalledFilter && loader !== "vanilla") {
			const prefix = `${loader}:`;
			return [...(versionsState.loaderVersions?.keys() ?? [])]
				.filter((key) => key.startsWith(prefix))
				.map((key) => key.slice(prefix.length)).sort(compareVersions);
		}

		let baseVersions: string[] = [];
		if (loader === "vanilla") {
			baseVersions = Array.from(parsed.vanilla);
		} else if (loader === "fabric") {
			baseVersions = Array.from(
				new SvelteSet([...parsed.fabric, ...parsed.vanilla]),
			);
		} else if (loader === "quilt") {
			baseVersions = Array.from(
				new SvelteSet([...parsed.quilt, ...parsed.vanilla]),
			);
		} else if (loader === "forge") {
			baseVersions = Array.from(parsed.forge).map((v) => {
				const idx = v.indexOf("-forge-");
				return idx >= 0 ? v.substring(0, idx) : v;
			});
		} else if (loader === "optifine") {
			baseVersions = Array.from(parsed.optifine).map(
				(v) => v.split("-OptiFine_")[0],
			);
		} else if (loader === "neoforge") {
			baseVersions = Array.from(parsed.neoforge).map((v) => {
				const idx = v.indexOf("-neoforge-");
				return idx >= 0 ? v.substring(0, idx) : v;
			});
		}

		return Array.from(new SvelteSet(baseVersions)).sort(compareVersions);
	}

	const availableMcVersions = $derived.by(() => {
		const local = getMcVersionsForLoader(selectedLoader);
		const remote = catalogLoader === selectedLoader && catalog
			? (!launcherStore.settings.show_unstable_loaders && catalog.byGame.size
				? catalog.stableGameVersions : catalog.gameVersions) : [];
		const merged = includeRemote
			? Array.from(
					new SvelteSet([
						...remote,
						...local,
					]),
				)
			: local;
		if (!includeAvailable) return merged;
		const settings = launcherStore.settings;
		return merged.filter((v) => minecraftVersionVisible(minecraftTypes.get(v), settings.show_snapshots, settings.show_alpha) &&
			(settings.show_unstable_loaders || catalogLoader !== selectedLoader ||
				!catalog?.byGame.has(v) || catalog.byGame.get(v)?.some((build) => build.stable)));
	});

	function versionAction(loader: string, mc: string, build: string) {
		if (!onDownload) return {};
		const version = instanceVersionId(loader, mc, build);
		const task = downloads.get(version);
		if ((task && !task.done && !task.error) || pendingDownloads?.has(version) || isVersionDownloading(version)) {
			return { progress: task && !task.error ? getOverallPct(task) : 0 };
		}
		if (isVersionInstalled(version) || (task?.done && !task.error)) {
			return { badge: t("versionDownloader.installedTag"), badgeTone: "success" as const };
		}
		if (downloadDisabled) return {};
		return { download: () => onDownload?.(loader, mc, build) };
	}

	const mcVersionOptions = $derived(
		availableMcVersions.map((v) => ({ value: v, label: v, ...(selectedLoader === "vanilla" ? versionAction("vanilla", v, "") : {}) })),
	);

	const mcPlaceholder = $derived(
		!loadingMc && availableMcVersions.length === 0
			? t(
					includeAvailable
						? "versionDownloader.notFound"
						: "createInstance.noVersionsErr",
				)
			: t("createInstance.selectMcVersion"),
	);

	const availableLoaderVersions = $derived.by(() => {
		if (!selectedMcVersion || selectedLoader === "vanilla") return [];
		const key = `${selectedLoader}:${selectedMcVersion}`;
		const installed =
			versionsState.loaderVersions?.get(key) ?? new Set<string>();
		const remote = remoteLoaderKey === key ? remoteLoaderVersions : [];
		const unstable = new Set(remote.filter((v) => !v.stable).map((v) => v.version));
		const showUnstable = !includeAvailable || launcherStore.settings.show_unstable_loaders;
		return Array.from(
			new SvelteSet([
				...[...installed].filter((v) => showUnstable || !unstable.has(v)),
				...(includeRemote ? remote.filter((v) => showUnstable || v.stable).map((v) => v.version) : []),
			]),
		).sort(compareVersions);
	});

	const loaderVersionOptions = $derived(
		availableLoaderVersions.map((v) => ({ value: v, label: v, ...versionAction(selectedLoader, selectedMcVersion, v) })),
	);

	$effect(() => {
		if (loadingMc) return;
		if (availableMcVersions.length === 0) {
			selectedMcVersion = "";
			return;
		}
		if (
			!selectedMcVersion ||
			!availableMcVersions.includes(selectedMcVersion)
		) {
			selectedMcVersion = availableMcVersions[0];
		}
	});

	$effect(() => {
		if (loadingLoader) return;
		if (selectedLoader === "vanilla") {
			selectedLoaderVersion = "";
			return;
		}
		if (availableLoaderVersions.length === 0) {
			selectedLoaderVersion = "";
			return;
		}
		if (
			!selectedLoaderVersion ||
			!availableLoaderVersions.includes(selectedLoaderVersion)
		) {
			selectedLoaderVersion = availableLoaderVersions[0];
		}
	});

	onMount(() => {
		if (!versionsState.loaded && !versionsState.loading) {
			loadInstalledVersions();
		}
	});
</script>

<div class="version-selector" class:compact aria-busy={loading}>
	<div class="loader-unified">
		{#each LOADERS as loader (loader.value)}
			<button
				type="button"
				class="loader-btn"
				class:active={selectedLoader === loader.value}
				aria-label={loader.label}
				aria-pressed={selectedLoader === loader.value}
				onclick={() => (selectedLoader = loader.value)}
			>
				<Icon name={loader.iconName} size={compact ? 16 : 20} />
				<span>{loader.label}</span>
			</button>
		{/each}
	</div>

	<div class="linked-selects">
		<Select
			bind:value={selectedMcVersion}
			options={mcVersionOptions}
			placeholder={mcPlaceholder}
			loading={loadingMc}
			loadingPlaceholder={t("createInstance.loading")}
			disabled={loadingMc || mcVersionOptions.length === 0}
		/>

		<Select
			bind:value={selectedLoaderVersion}
			options={loaderVersionOptions}
			placeholder={selectedLoader === "vanilla"
				? t("createInstance.noLoader")
				: t("createInstance.selectLoaderVersion")}
			loading={loadingLoader}
			loadingPlaceholder={t("createInstance.loading")}
			disabled={selectedLoader === "vanilla" ||
				loadingLoader ||
				loaderVersionOptions.length === 0}
		/>
	</div>
	{#if showInstalledFilter}
		<label class="installed-filter" for={`${id}-installed`}>
			<ToggleSwitch id={`${id}-installed`} bind:checked={onlyInstalled} />
			{t("versionDownloader.onlyInstalled")}
		</label>
	{/if}
	{#if includeRemote && sourceError}
		<p role="alert">{sourceError}</p>
	{/if}
</div>

<style>
	.installed-filter {
		display: flex;
		align-items: center;
		gap: 8px;
		width: fit-content;
		cursor: pointer;
		font-size: 0.82rem;
	}
	.version-selector {
		display: flex;
		flex-direction: column;
		gap: 16px;
		height: 100%;
	}

	.loader-unified {
		display: flex;
		width: 100%;
	}

	.loader-btn {
		--btn-bg: rgba(var(--accent-rgb, 255, 255, 255), 0.03);
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 12px 8px;
		background: var(--btn-bg);
		border: 1px solid var(--border);
		color: var(--text-secondary);
		font-family: inherit;
		font-size: 0.8rem;
		font-weight: 600;
		cursor: pointer;
		position: relative;
		margin-left: -1px;
		z-index: 0;
		transition:
			background-color 0.15s,
			color 0.15s,
			border-color 0.15s,
			box-shadow 0.15s;
	}

	.loader-btn:first-child {
		margin-left: 0;
		border-radius: var(--border-radius-sm) 0 0 var(--border-radius-sm);
	}

	.loader-btn:last-child {
		border-radius: 0 var(--border-radius-sm) var(--border-radius-sm) 0;
	}

	.loader-btn:hover {
		background: rgba(var(--accent-rgb, 255, 255, 255), 0.06);
		color: var(--text-primary);
		z-index: 1;
	}

	.loader-btn.active {
		background: rgba(var(--accent-rgb, 255, 255, 255), 0.1);
		border-color: var(--accent);
		color: var(--text-primary);
		z-index: 2;
	}

	.loader-btn :global(.icon-svg) {
		width: 20px;
		height: 20px;
	}

	.loader-btn span {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	@media (max-width: 500px) {
		.loader-btn span {
			display: none;
		}
	}

	.linked-selects {
		display: flex;
		width: 100%;
	}

	.linked-selects > :global(.custom-select-container) {
		flex: 1;
		min-width: 0;
	}

	/* Left select: right border and radii removed */
	.linked-selects
		> :global(.custom-select-container:first-child .select-trigger) {
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
		border-right: none;
	}

	/* Right select: left radii removed */
	.linked-selects
		> :global(.custom-select-container:last-child .select-trigger) {
		border-top-left-radius: 0;
		border-bottom-left-radius: 0;
	}

	.version-selector.compact {
		gap: 8px;
	}

	.version-selector.compact .loader-unified {
		justify-content: center;
	}

	.version-selector.compact .loader-btn {
		flex: 0 0 auto;
		padding: 6px 8px;
		font-size: 0.7rem;
		gap: 0;
	}

	.version-selector.compact .loader-btn :global(.icon-svg) {
		width: 16px;
		height: 16px;
		flex-shrink: 0;
	}

	.version-selector.compact .loader-btn span {
		display: inline-block;
		min-width: 0;
		max-width: 0;
		margin-left: 0;
		opacity: 0;
		transition:
			max-width 0.4s cubic-bezier(0.4, 0, 0.2, 1),
			margin-left 0.4s cubic-bezier(0.4, 0, 0.2, 1),
			opacity 0.4s cubic-bezier(0.4, 0, 0.2, 1);
		pointer-events: none;
	}

	.version-selector.compact .loader-btn:hover span,
	.version-selector.compact .loader-btn:focus-visible span {
		max-width: 120px;
		margin-left: 4px;
		opacity: 1;
	}

	.version-selector.compact
		:global(.custom-select-container .select-trigger) {
		padding: 4px 8px;
		font-size: 0.75rem;
		min-height: 0;
	}
</style>
