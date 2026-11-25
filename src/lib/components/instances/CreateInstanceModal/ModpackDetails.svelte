<script lang="ts">
	import { onDestroy, tick, untrack, type Snippet } from "svelte";
	import { slide } from "svelte/transition";
	import { t } from "$lib/i18n";
	import { openUrl } from "$lib/api/cubicApi";
	import { launcherStore } from "$lib/state/state.svelte";
	import { downloads, getOverallPct } from "$lib/state/downloadQueueState.svelte";
	import Loading from "$lib/icons/Loading.svelte";
	import GalleryViewer from "$lib/components/ui/GalleryViewer.svelte";
	import ScrollToTopButton from "$lib/components/ui/ScrollToTopButton.svelte";
	import ModpackProperties from "./ModpackProperties.svelte";
	import PackLoaderBadge from "./PackLoaderBadge.svelte";
	import { PACK_VERSION_PAGE_SIZE, PackVersionPager } from "$lib/utils/packVersionPager.svelte";
	import Icon from "$lib/icons/Icon.svelte";
	import {
		isValidInstanceName,
		MAX_INSTANCE_NAME_LEN,
		sanitizeInstanceName,
	} from "$lib/utils/instanceName";
	import {
		searchPackVersions,
		visiblePackVersions,
		type PackLoaders,
		type PackGalleryImage,
		type PackVersion,
		type ReleaseFilter,
	} from "$lib/utils/modpackVersions";
	import type { ModpackItem } from "./ModpackBrowser.svelte";

	let {
		item,
		versions,
		loading,
		installing,
		installTask,
		error,
		gallery,
		website,
		description,
		onBack,
		onInstall,
		loaders,
	}: {
		item: ModpackItem;
		versions: PackVersion[];
		loading: boolean;
		installing: boolean;
		installTask?: { archive: string; name: string };
		error: string | null;
		gallery: PackGalleryImage[];
		website?: string;
		description?: Snippet<[ModpackItem]>;
		onBack?: () => void;
		onInstall?: (version: string, name: string) => void;
		loaders?: PackLoaders;
	} = $props();
	const installProgress = $derived.by(() => {
		if (!installTask) return undefined;
		const instance = launcherStore.loadedInstances.find((item) => item.name === installTask?.name);
		const archive = downloads.get(installTask.archive);
		return (instance && downloads.get(`modpack-${instance.uuid}`)) ?? (archive?.done ? undefined : archive);
	});

	let selection = $state<string | null>();
	let tab = $state<"description" | "versions" | "gallery">("description");
	const versionPage = new PackVersionPager();
	const pickerPage = new PackVersionPager();
	let versionsScroller = $state<HTMLDivElement>();
	let pickerScroller = $state<HTMLDivElement>();
	let picker = $state(false);
	let pickerPanel = $state<HTMLElement>();
	let pickerTrigger: HTMLButtonElement | undefined;
	let showAlpha = $state(false);
	let releaseFilter = $state<ReleaseFilter>("all");
	let name = $state<string | null>(null);
	const generatedName = $derived(sanitizeInstanceName(item.title));
	let editing = $state(false);
	let draft = $state("");
	let nameInput = $state<HTMLInputElement>();
	let editNameButton = $state<HTMLButtonElement>();
	let nameEditor = $state<HTMLDivElement>();
	let nameError = $state<string | null>(null);
	let nameSaved = $state(false);
	let composingName = $state(false);
	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	onDestroy(clearNameSave);
	let properties = $state<PackVersion>();
	let galleryIndex = $state(-1);
	let dialogTrigger: HTMLButtonElement | undefined;

	function restoreDialogFocus() {
		dialogTrigger?.focus({ preventScroll: true });
	}

	const pickerList = $derived(visiblePackVersions(versions, showAlpha));
	const selectedId = $derived(selection === undefined ? pickerList.latest : selection);
	const selected = $derived(versions.find((v) => v.value === selectedId));
	const fileList = $derived(
		visiblePackVersions(versions, showAlpha, releaseFilter),
	);
	const locale = $derived(launcherStore.settings.language === "ru" ? "ru-RU" : "en-US");
	const matchingVersions = $derived(pagedVersions(fileList.versions, versionPage));
	const matchingPickerVersions = $derived(pagedVersions(pickerList.versions, pickerPage));
	$effect(() => {
		const visible = [
			...(selected ? [selected] : []),
			...(picker ? matchingPickerVersions : []),
			...(tab === "versions" ? matchingVersions : []),
		];
		const notify = loaders?.onVisibleVersions;
		untrack(() => notify?.(visible));
	});

	function pagedVersions(files: PackVersion[], page: PackVersionPager) {
		if (page.searching) return searchPackVersions(files, page.query, locale);
		return page.expanded ? files : files.slice(0, page.limit);
	}

	function searchVersions(page: PackVersionPager, query: string, scroller?: HTMLElement) {
		page.setQuery(query);
		if (scroller) scroller.scrollTop = 0;
	}
	const filters = $derived<ReleaseFilter[]>(
		showAlpha
			? ["all", "release", "beta", "alpha"]
			: ["all", "release", "beta"],
	);
	const tabs = ["description", "versions", "gallery"] as const;
	const controlId = $props.id();

	async function editName() {
		closePicker(false);
		draft = name ?? generatedName;
		nameError = null;
		nameSaved = false;
		editing = true;
		await tick();
		nameInput?.focus({ preventScroll: true });
		nameInput?.select();
	}

	function validateName(value: string) {
		if (!value) return t("createInstance.emptyNameErr");
		if (value.length > MAX_INSTANCE_NAME_LEN)
			return t("createInstance.nameTooLong");
		if (!isValidInstanceName(value))
			return t("createInstance.nameInvalidChars");
		if (
			launcherStore.loadedInstances.some(
				(instance) =>
					instance.name.toLowerCase() === value.toLowerCase(),
			)
		)
			return t("createInstance.nameExists");
		return null;
	}

	function finishNameEdit() {
		const focusWasInEditor = nameEditor?.contains(document.activeElement);
		clearNameSave();
		editing = false;
		composingName = false;
		draft = "";
		if (focusWasInEditor) {
			void tick().then(() => editNameButton?.focus({ preventScroll: true }));
		}
	}

	function cancelNameEdit() {
		name = null;
		nameError = null;
		nameSaved = false;
		finishNameEdit();
	}

	function clearNameSave() {
		clearTimeout(saveTimer);
		saveTimer = undefined;
	}

	function updateDraft(value: string) {
		clearNameSave();
		draft = value;
		nameError = null;
		nameSaved = false;
		if (!composingName) saveTimer = setTimeout(saveName, 500);
	}

	function saveName() {
		clearNameSave();
		if (!editing || composingName) return null;
		const value = draft.trim();
		nameError = validateName(value);
		if (nameError) return null;
		name = value;
		nameSaved = true;
		return value;
	}

	function installName() {
		if (editing) {
			const value = saveName();
			if (value === null && !composingName) nameInput?.focus({ preventScroll: true });
			return value;
		}
		const value = name ?? generatedName;
		const error = validateName(value);
		if (!error) return value;
		void editName();
		nameError = error;
		return null;
	}

	async function openPicker(trigger: HTMLButtonElement) {
		if (picker) {
			closePicker();
			return;
		}
		pickerTrigger = trigger;
		picker = true;
		await tick();
		pickerPanel?.focus({ preventScroll: true });
	}

	function closePicker(restoreFocus = true) {
		if (!picker) return;
		picker = false;
		pickerPage.reset();
		if (restoreFocus) pickerTrigger?.focus({ preventScroll: true });
	}

	function installClick() {
		if (!selected) return;
		const instanceName = installName();
		if (instanceName) onInstall?.(selected.value, instanceName);
	}

	function chooseVersion(version: PackVersion, fromPicker = false) {
		selection = version.value;
		closePicker(fromPicker);
	}

	function setShowAlpha(value: boolean) {
		showAlpha = value;
		if (!value) {
			if (releaseFilter === "alpha") releaseFilter = "all";
			if (selected?.channel === "alpha") selection = undefined;
		}
	}

	function changeTab(id: (typeof tabs)[number]) {
		if (tab === "versions" && id !== "versions") versionPage.reset();
		tab = id;
		closePicker(false);
	}

	export function goBackOnEscape() {
		if (installing || composingName) return;
		if (picker) closePicker();
		else if (editing) cancelNameEdit();
		else if (tab !== "description") changeTab("description");
		else onBack?.();
	}
</script>

{#snippet versionPagination(page: PackVersionPager, total: number)}
	{#if total > PACK_VERSION_PAGE_SIZE && !page.showAll}
		<div class="version-pagination">
			{#if !page.searching && total > page.limit}
				<button type="button" class="secondary" onclick={() => page.loadMore()}>{t("createInstance.loadMore")}</button>
			{/if}
			<button type="button" class="secondary" onclick={() => (page.showAll = true)}>{t("createInstance.pack.showAllVersions")}</button>
		</div>
	{/if}
{/snippet}

{#snippet versionTags(version: PackVersion, latest: string | undefined)}
	{#if version.value === latest}<span class="tag latest"
			>{t("createInstance.pack.latest")}</span
		>{/if}
	<span class="tag" data-channel={version.channel}
		>{t(`createInstance.pack.${version.channel}`)}</span
	>
	{#each version.packLoaders ?? [] as loader (loader)}
		<PackLoaderBadge {loader} />
	{/each}
{/snippet}

{#snippet alphaToggle()}
	<label class="alpha-toggle"
		><input
			type="checkbox"
			checked={showAlpha}
			onchange={(e) => setShowAlpha(e.currentTarget.checked)}
		/>
		{t("createInstance.pack.showAlpha")}</label
	>
{/snippet}

<div class="pack-detail">
	<button
		type="button"
		class="secondary back"
		disabled={installing}
		onclick={onBack}
	>
		<Icon name="ui:chevron-left" size={16} />
		{t("createInstance.backBtn")}
	</button>
	<div class="heading">
		<div class="picture" aria-hidden="true">
			<Icon name="instance:box" size={40} />
			{#if item.iconUrl}<img
					src={item.iconUrl}
					alt=""
					decoding="async"
					onerror={(e) => e.currentTarget.setAttribute("hidden", "")}
				/>{/if}
		</div>
		<div class="summary">
			<h3>{item.title}</h3>
			{#if item.author}<span class="muted">{item.author}</span>{/if}
			<p>{item.description}</p>
			<span class="muted"
				>{item.downloads.toLocaleString()}
				{t("createInstance.downloads")}</span
			>
			{#if item.categories?.length || item.packLoaders?.length}
				<div
					class="tags"
				>
					{#each [...new Set(item.categories ?? [])] as category}<span
							class="tag">{category.replaceAll("-", " ")}</span
						>{/each}
					{#each item.packLoaders ?? [] as loader (loader)}
						<PackLoaderBadge {loader} />
					{/each}
				</div>
			{/if}
		</div>
	</div>

	<div class="install-card">
		<div class="install-row">
			<div class="name">
				{#if editing}
					<label for={`${controlId}-name`} class="muted"
						>{t("createInstance.nameLabel")}</label
					>
					<div class="name-row" bind:this={nameEditor}>
						<input
							class="text-input"
							id={`${controlId}-name`}
							bind:this={nameInput}
							value={draft}
							maxlength={MAX_INSTANCE_NAME_LEN}
							disabled={installing}
							aria-invalid={!!nameError}
							oninput={(e) => updateDraft(e.currentTarget.value)}
							oncompositionstart={() => {
								composingName = true;
								clearNameSave();
							}}
							oncompositionend={(e) => {
								composingName = false;
								updateDraft(e.currentTarget.value);
							}}
						/>
						<button
							type="button"
							class="text-action cancel-name"
							disabled={installing}
							onclick={cancelNameEdit}
							>{t("createInstance.pack.cancelName")}</button
						>
					</div>
				{:else}
					<span class="muted">{t("createInstance.nameLabel")}</span>
					<div class="name-row">
						<strong>{name ?? generatedName}</strong><button
							type="button"
							class="text-action"
							bind:this={editNameButton}
							onclick={editName}
							disabled={installing}
							>{t("createInstance.pack.editName")}</button
						>
					</div>
				{/if}
				<span class="muted" role="status">{nameSaved ? t("createInstance.pack.nameSaved") : ""}</span>
			</div>
			<div class="install-actions">
				<button
					type="button"
					class="btn-primary install-button"
					disabled={installing || loading || composingName || !selected}
					onclick={installClick}
				>
					{#if installing}
						<Loading size={16} />
						{t("createInstance.installingModpack")} {installProgress ? getOverallPct(installProgress) : 0}%
					{:else}
						{t("createInstance.installBtn")}
					{/if}
				</button>
				<div class="version-summary">
					<span class="version-label">{selected?.label ?? t("createInstance.pack.noVersionSelected")}</span>
				{#each selected?.packLoaders ?? [] as loader (loader)}
					<PackLoaderBadge {loader} />
				{/each}
					<button type="button" class="text-action"
						disabled={installing || loading || !versions.length}
						aria-expanded={picker}
						aria-controls={picker ? `${controlId}-picker` : undefined}
						onclick={(e) => openPicker(e.currentTarget)}>
						{t("createInstance.pack.changeVersion")}
					</button>
				</div>
			</div>
		</div>
		{#if nameError}<p class="error" role="alert">{nameError}</p>{/if}
		{#if error}<p class="error" role="alert">{error}</p>{/if}
		{#if picker}
			<section
				class="picker"
				transition:slide={{ duration: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 180 }}
				bind:this={pickerPanel}
				id={`${controlId}-picker`}
				tabindex="-1"
				aria-label={t("createInstance.pack.chooseVersion")}
			>
				<div class="filters">
					<input type="search" class="text-input version-search"
						aria-label={t("createInstance.pack.searchVersions")}
						placeholder={t("createInstance.pack.searchVersions")}
						value={pickerPage.query}
						oninput={(e) => searchVersions(pickerPage, e.currentTarget.value, pickerScroller)} />
					{@render alphaToggle()}
				</div>
				<div class="version-scroll" bind:this={pickerScroller} tabindex="-1" role="region" aria-label={t("createInstance.pack.chooseVersion")}>
				<div class="picker-list">
					{#each matchingPickerVersions as version (version.value)}
						<button
							class="version-row picker-item"
							class:chosen={selectedId === version.value}
							type="button"
							onclick={() => chooseVersion(version, true)}
						>
							<span>{version.label}</span><span class="tags"
								>{@render versionTags(
									version,
									pickerList.latest,
								)}</span
							>
						</button>
					{:else}<p class="muted">
							{t("createInstance.pack.noFiles")}
						</p>{/each}
					{@render versionPagination(pickerPage, pickerList.versions.length)}
				</div>
				<ScrollToTopButton scroller={pickerScroller} />
				</div>
			</section>
		{/if}
	</div>

	<nav class="tabs" aria-label={t("createInstance.detailsTitle")}>
		{#each tabs as id}
			<button
				class="secondary"
				class:chosen={tab === id}
				type="button"
				aria-pressed={tab === id}
				onclick={() => changeTab(id)}
			>
				{t(`createInstance.pack.${id}`)}
			</button>
		{/each}
		<button
			class="secondary site"
			type="button"
			disabled={!website}
			onclick={() => {
				if (website) void openUrl(website);
			}}
		>
			<svg
				width="13"
				height="13"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="1.8"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
				><path
					d="M13 5H6a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-7M14 4h6v6M20 4l-9 9"
				/></svg
			>
			{t("createInstance.pack.website")}
		</button>
	</nav>

	{#if tab === "description"}
		{#if description}{@render description(item)}{/if}
	{:else if tab === "gallery"}
		<div class="gallery">
			{#each gallery as image, index}
				<figure>
					<button
						class="gallery-picture"
						type="button"
						aria-haspopup="dialog"
						aria-label={image.title ||
							t("createInstance.pack.openImage")}
						onclick={(e) => {
							dialogTrigger = e.currentTarget;
							galleryIndex = index;
						}}
					>
						<img
							src={image.thumbnail || image.url}
							alt={image.title || ""}
							loading="lazy"
							decoding="async"
						/>
					</button>
					{#if image.title || image.description}<figcaption>
							<strong>{image.title}</strong>
							{image.description}
						</figcaption>{/if}
				</figure>
			{:else}<p class="muted">
					{t(
						loading
							? "createInstance.loading"
							: "createInstance.pack.noGallery",
					)}
				</p>{/each}
		</div>
	{:else}
		<div class="filters">
			<input type="search" class="text-input version-search"
				aria-label={t("createInstance.pack.searchVersions")}
				placeholder={t("createInstance.pack.searchVersions")}
				value={versionPage.query}
				oninput={(e) => searchVersions(versionPage, e.currentTarget.value, versionsScroller)} />
			{#each filters as filter}
				<button
					type="button"
					class="secondary"
					class:chosen={filter === releaseFilter}
					data-channel={filter === "all" ? undefined : filter}
					aria-pressed={filter === releaseFilter}
					onclick={() => (releaseFilter = filter)}
					>{t(`createInstance.pack.${filter}`)}</button
				>
			{/each}
			{@render alphaToggle()}
		</div>
		{#if loading}<div role="status" aria-label={t("createInstance.loading")}><Loading /></div>
		{:else}
			<div class="version-scroll full-version-scroll" bind:this={versionsScroller} tabindex="-1" role="region" aria-label={t("createInstance.pack.versions")}>
			<div class="files">
				{#each matchingVersions as version (version.value)}
					<div
						class="file"
						class:chosen={selectedId === version.value}
					>
						<div class="version-row">
							<div class="file-name">
								<strong>{version.label}</strong>
								<div class="tags">
									{@render versionTags(
										version,
										fileList.latest,
									)}
								</div>
							</div>
							<div class="file-actions">
								<button
									type="button"
									class="secondary"
									disabled={installing}
									onclick={() => {
									if (selectedId === version.value) selection = null;
										else chooseVersion(version);
									}}
									>{t(
										selectedId === version.value
											? "createInstance.pack.cancelSelection"
											: "createInstance.pack.select",
									)}</button
								>
								<button
									type="button"
									class="secondary"
									aria-haspopup="dialog"
									disabled={!loaders}
									onclick={(e) => {
										dialogTrigger = e.currentTarget;
										properties = version;
									}}
									>{t(
										"createInstance.pack.properties",
									)}</button
								>
							</div>
						</div>
					</div>
				{:else}<p class="muted">
						{t("createInstance.pack.noFiles")}
					</p>{/each}
				{@render versionPagination(versionPage, fileList.versions.length)}
			</div>
			<ScrollToTopButton scroller={versionsScroller} />
			</div>
		{/if}
	{/if}
</div>

{#if properties && loaders}
	<ModpackProperties
		version={properties}
		{loaders}
		{website}
		onclose={() => {
			properties = undefined;
			restoreDialogFocus();
		}}
	/>
{/if}
{#if galleryIndex >= 0}
	<GalleryViewer
		images={gallery}
		bind:index={galleryIndex}
		onclose={() => {
			galleryIndex = -1;
			restoreDialogFocus();
		}}
	/>
{/if}

<style>
	.pack-detail,
	.summary,
	.files {
		display: flex;
		flex-direction: column;
		gap: 10px;
		min-width: 0;
	}
	.heading,
	.install-row,
	.name-row,
	.install-actions,
	.tabs,
	.filters,
	.version-row,
	.file-actions,
	.tags {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.heading {
		align-items: flex-start;
		gap: 16px;
		flex-wrap: nowrap;
	}
	.picture {
		position: relative;
		flex: 0 0 auto;
		display: grid;
		place-items: center;
		width: clamp(64px, 17cqw, 132px);
		aspect-ratio: 1;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		overflow: hidden;
	}
	.picture img {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.summary {
		flex: 1;
		gap: 4px;
		overflow-wrap: anywhere;
	}
	h3,
	p,
	figure {
		margin: 0;
	}
	.summary p,
	.version-row {
		font-size: 0.84rem;
	}
	.muted {
		color: var(--text-secondary);
		font-size: 0.78rem;
	}
	.back {
		align-self: flex-start;
		display: flex;
		align-items: center;
		gap: 5px;
	}
	.secondary,
	.tag {
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		color: var(--text-primary);
		background: var(--bg-card);
		padding: 5px 9px;
	}
	.secondary:not(:disabled),
	.picker-item,
	.text-action,
	.gallery-picture {
		cursor: pointer;
	}
	.secondary:not(:disabled):hover,
	.picker-item:hover,
	.chosen {
		background: var(--surface-selected);
	}
	.site {
		margin-inline-start: auto;
		display: inline-flex;
		align-items: center;
		gap: 5px;
		font-size: 0.74rem;
		padding: 3px 7px;
	}
	.install-card,
	.file {
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: var(--bg-card);
		padding: 9px;
	}
	.install-row,
	.version-row {
		justify-content: space-between;
	}
	.name,
	.file-name {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.name strong {
		text-transform: none;
	}
	.name input {
		min-width: 0;
		max-width: 100%;
	}
	.text-action {
		color: var(--accent);
		padding: 3px;
		border: 0;
		background: none;
	}
	.text-action:hover {
		text-decoration: underline;
	}
	.cancel-name { color: var(--color-error); }
	.version-search { flex: 1 1 230px; min-width: 0; }
	.install-actions {
		margin-inline-start: auto;
		flex-direction: column;
		align-items: flex-end;
		min-width: 0;
		max-width: min(320px, 100%);
	}
	.install-button {
		min-height: calc(var(--button-height) + 4px);
		min-width: min(104px, 100%);
	}
	.version-summary {
		display: flex;
		justify-content: flex-end;
		text-align: end;
		font-size: 0.72rem;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
		overflow-wrap: anywhere;
	}
	.version-label {
		font-size: 0.67rem;
		font-weight: normal;
	}
	.picker {
		margin-top: 8px;
		padding-top: 8px;
		border-top: 1px solid var(--border);
	}
	.picker-list {
		display: grid;
		gap: 4px;
	}
	.version-scroll {
		max-height: min(230px, 35vh);
		overflow-y: auto;
		overscroll-behavior-y: contain;
		padding: 3px;
	}
	.full-version-scroll {
		max-height: min(270px, 40vh);
	}
	.version-pagination {
		display: flex;
		justify-content: center;
		gap: 6px;
		flex-wrap: wrap;
		padding-bottom: 46px;
	}
	.picker-item {
		width: 100%;
		background: transparent;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		padding: 6px;
		color: inherit;
		text-align: start;
		overflow-wrap: anywhere;
	}
	.alpha-toggle {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		font-size: 0.78rem;
	}
	.tag {
		font-size: 0.68rem;
		padding: 2px 5px;
	}
	.tag.latest {
		color: var(--accent);
	}
	[data-channel="release"] { --channel-bg: #8bd9a0; --channel-ink: #163821; }
	[data-channel="beta"] { --channel-bg: #efbf70; --channel-ink: #422900; }
	[data-channel="alpha"] { --channel-bg: #ed8e91; --channel-ink: #481719; }
	.tag[data-channel], .secondary[data-channel] {
		color: var(--channel-ink);
		background: var(--channel-bg);
		border-color: var(--channel-bg);
	}
	.secondary[data-channel]:is(:hover, .chosen) {
		border-color: var(--channel-ink);
	}
	.tabs {
		padding-block: 4px 8px;
		border-bottom: 1px solid var(--border);
	}
	.file.chosen, .picker-item.chosen {
		border-color: var(--accent);
		box-shadow: 0 0 8px color-mix(in srgb, var(--accent) 28%, transparent);
		background: var(--surface-selected);
	}
	.file-name {
		flex: 1 1 180px;
	}
	.file-name .tags {
		margin-top: 4px;
	}
	.error {
		color: var(--color-error);
		margin-top: 6px;
	}
	.gallery {
		display: grid;
		gap: 12px;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 220px), 1fr));
	}
	.gallery-picture {
		display: block;
		width: 100%;
		border: 0;
		padding: 0;
		background: none;
	}
	.gallery-picture img {
		display: block;
		width: 100%;
		aspect-ratio: 16 / 9;
		object-fit: contain;
		background: var(--bg-card);
		border-radius: var(--border-radius-sm);
	}
	figcaption {
		margin-block-start: 4px;
		color: var(--text-secondary);
		font-size: 0.78rem;
	}
</style>
