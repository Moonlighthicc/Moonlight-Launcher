<script lang="ts">
	import { fade } from "svelte/transition";
	import { MediaQuery } from "svelte/reactivity";
	import { animDuration } from "$lib/utils/animations";
	import {
		createInstance,
		uploadCustomIcon,
		addToQueue,
		getDownloadSnapshot,
		downloadFabric,
		downloadForge,
		downloadNeoForge,
		downloadQuilt,
		downloadOptiFine,
	} from "$lib/api/cubicApi";
	import { launcherStore } from "$lib/state/state.svelte";
	import { isVersionDownloading } from "$lib/state/downloadState.svelte";
	import {
		downloads,
		getOverallPct,
		pendingVersionDownloads as pendingDownloads,
		trackConnecting,
	} from "$lib/state/downloadQueueState.svelte";
	import {
		isVersionInstalled,
		loadInstalledVersions,
	} from "$lib/state/versionsState.svelte";
	import ModalBase from "$lib/components/layout/ModalBase.svelte";
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	import Loading from "$lib/icons/Loading.svelte";
	import { getIconPath } from "$lib/icons/registry";
	import IconPicker from "./IconPicker.svelte";
	import VersionSelectorStep from "./VersionSelectorStep.svelte";
	import { instanceVersionId } from "./availableVersions";
	import ModrinthModpackBrowser from "./ModrinthModpackBrowser.svelte";
	import CurseForgeModpackBrowser from "./CurseForgeModpackBrowser.svelte";
	import ModpackImportStep from "./ModpackImportStep.svelte";
	import InstanceImportStep from "./InstanceImportStep.svelte";
	import LauncherMigrationStep from "./LauncherMigrationStep.svelte";
	import {
		MAX_INSTANCE_NAME_LEN,
		isValidInstanceName,
	} from "$lib/utils/instanceName";

	let {
		open = $bindable(),
		mrpackPath = $bindable<string | null>(null),
		instanceZipPath = $bindable<string | null>(null),
		oncreated,
		oninstallstarted,
		oninstallfailed,
	}: {
		open: boolean;
		mrpackPath?: string | null;
		instanceZipPath?: string | null;
		oncreated?: () => void;
		oninstallstarted?: (name: string) => void;
		oninstallfailed?: (name: string) => void;
	} = $props();

	type Tab = "manual" | "modrinth" | "curseforge" | "local";
	let tab = $state<Tab>("manual");
	type CreationMode = "choose" | "manual" | "pack" | "instance" | "migration";
	let mode = $state<CreationMode>("choose");
	let versionLoading = $state(false);
	let creatingVersion = $state("");
	const createProgress = $derived(downloads.get(creatingVersion));
	const reducedMotion = new MediaQuery("(prefers-reduced-motion: reduce)");
	const contentDuration = $derived(
		reducedMotion.current ? 0 : animDuration(180),
	);

	const TABS = $derived<{ id: Tab; label: string; iconName: string }[]>([
		{
			id: "modrinth",
			label: "Modrinth",
			iconName: "brand:modrinth",
		},
		{
			id: "curseforge",
			label: t("createInstance.curseforgeTab"),
			iconName: "brand:curseforge",
		},
		{
			id: "local",
			label: t("createInstance.localTab"),
			iconName: "instance:folder",
		},
	]);

	let name = $state("");
	let selectedIcon = $state<string | null>(null);
	let customIconPath = $state<string | null>(null);

	let selectedLoader = $state("vanilla");
	let selectedMcVersion = $state("");
	let selectedLoaderVersion = $state("");

	const finalVersionId = $derived(instanceVersionId(selectedLoader, selectedMcVersion, selectedLoaderVersion));

	let loading = $state(false);
	let modrinthBrowser = $state<ReturnType<typeof ModrinthModpackBrowser>>();
	let curseForgeBrowser = $state<ReturnType<typeof CurseForgeModpackBrowser>>();
	let error = $state<string | null>(null);

	function chooseMethod() {
		mode = "choose";
		error = null;
	}

	function goBackOnEscape() {
		if (mode === "choose") return false;
		if (mode === "pack") {
			const browser = tab === "modrinth" ? modrinthBrowser : tab === "curseforge" ? curseForgeBrowser : undefined;
			if (browser?.goBackOnEscape()) return true;
		}
		chooseMethod();
		return true;
	}
	let existingNames = $derived(
		launcherStore.loadedInstances.map((i) => i.name),
	);
	let nameMsg = $state<string | null>(null);

	function validateName(): boolean {
		const trimmed = name.trim();
		if (!trimmed) {
			nameMsg = "createInstance.emptyNameErr";
			return false;
		}
		if (trimmed.length > MAX_INSTANCE_NAME_LEN) {
			nameMsg = "createInstance.nameTooLong";
			return false;
		}
		if (existingNames.includes(trimmed)) {
			nameMsg = "createInstance.nameExists";
			return false;
		}
		if (!isValidInstanceName(trimmed)) {
			nameMsg = "createInstance.nameInvalidChars";
			return false;
		}
		nameMsg = null;
		return true;
	}

	// ── Effects ─────────────────────────────────────────────────────────────────
	$effect(() => {
		if (open) {
			nameMsg = null;
			if (mrpackPath || instanceZipPath) {
				tab = "local";
				mode = instanceZipPath ? "instance" : "pack";
			} else {
				tab = "manual";
				mode = "choose";
			}
		}
	});

	$effect(() => {
		if (open && tab === "manual" && selectedLoader) {
			const icon = selectIconForLoader(selectedLoader);
			if (icon && !selectedIcon) {
				selectedIcon = icon;
			}
		}
	});

	// ── Helpers ─────────────────────────────────────────────────────────────────
	function selectIconForLoader(loader: string | null): string | null {
		if (!loader) return null;
		const l = loader.toLowerCase();
		if (l === "fabric") return getIconPath("brand:fabric");
		if (l === "forge") return getIconPath("brand:forge");
		if (l === "neoforge" || l === "neo")
			return getIconPath("brand:neoforged");
		if (l === "quilt") return getIconPath("brand:vanilla");
		if (l === "optifine") return getIconPath("brand:optifine");
		return null;
	}

	function updateIconForLoader() {
		const icon = selectIconForLoader(selectedLoader);
		if (icon && !selectedIcon) {
			selectedIcon = icon;
		}
	}

	function handleIconUpload(filePath: string) {
		customIconPath = filePath;
		selectedIcon = filePath;
	}

	async function handleManualCreate() {
		if (loading || versionLoading) return;
		if (!validateName()) return;
		if (!finalVersionId) {
			error = t("createInstance.noVersionsErr");
			return;
		}
		loading = true;
		error = null;
		const id = finalVersionId;
		const instanceName = name.trim();
		const icon = selectedIcon;
		const customIcon = customIconPath;
		try {
			if (!isVersionInstalled(id)) {
				creatingVersion = id;
				await enqueueVersion(selectedLoader, selectedMcVersion, selectedLoaderVersion);
				await waitForInstallation(id);
				creatingVersion = "";
			}
			const uuid = await new Promise<string>((resolve, reject) => {
				void createInstance(
					instanceName,
					id,
					customIcon ? null : icon,
					resolve,
					reject,
				).catch(reject);
			});
			if (customIcon) await uploadCustomIcon(uuid, customIcon);
			reset();
			oncreated?.();
		} catch (err) {
			error = `${t("createInstance.createErr")}: ${String(err)}`;
		} finally {
			creatingVersion = "";
			loading = false;
		}
	}

	async function enqueueVersion(loader: string, mc: string, version: string) {
		const id = instanceVersionId(loader, mc, version);
		const task = downloads.get(id);
		if (!id || isVersionInstalled(id) || pendingDownloads.has(id) || isVersionDownloading(id) || (task && !task.done && !task.error)) return;
		pendingDownloads.add(id);
		try {
			await trackConnecting(id, async () => {
				if (loader === "vanilla") await addToQueue(id);
				else if (loader === "fabric") await downloadFabric(mc, version);
				else if (loader === "quilt") await downloadQuilt(mc, version);
				else if (loader === "forge") await downloadForge(mc, version);
				else if (loader === "neoforge") await downloadNeoForge(mc, version);
				else if (loader === "optifine") await downloadOptiFine(mc, version);
			});
		} finally {
			pendingDownloads.delete(id);
		}
	}

	async function waitForInstallation(id: string) {
		let missingSince: number | undefined;
		while (!isVersionInstalled(id)) {
			if (!pendingDownloads.has(id)) {
				const task = downloads.get(id);
				const snapshot = await getDownloadSnapshot();
				const status = snapshot?.find((entry) => entry.version === id)?.status;
				if (typeof status === "object") throw new Error(status.error);
				if (!snapshot && task?.error) throw new Error(task.error);
				const active = status === "pending" || status === "downloading" ||
					(!snapshot && task && !task.done && !task.error);
				if (active) missingSince = undefined;
				else missingSince ??= Date.now();
				if (status === "done" || (!snapshot && task?.done) ||
					(missingSince !== undefined && Date.now() - missingSince >= 15_000)) {
					await loadInstalledVersions(true);
					if (!isVersionInstalled(id)) throw new Error(t("modpack.downloadFailed"));
					return;
				}
			}
			await new Promise((resolve) => setTimeout(resolve, 500));
		}
	}

	function resetState() {
		creatingVersion = "";
		name = "";
		selectedLoader = "vanilla";
		selectedMcVersion = "";
		selectedLoaderVersion = "";
		selectedIcon = null;
		customIconPath = null;
		error = null;
		loading = false;
		mrpackPath = null;
		instanceZipPath = null;
		tab = "manual";
		mode = "choose";
	}

	function reset() {
		open = false;
		mrpackPath = null;
		instanceZipPath = null;
		resetState();
	}

	function close() {
		open = false;
		if (!creatingVersion) resetState();
	}

	function handleModpackInstallStarted(instanceName: string) {
		oninstallstarted?.(instanceName);
	}

	function handleModpackInstallFailed(instanceName: string) {
		oninstallfailed?.(instanceName);
	}

	function handleModpackInstalled() {
		reset();
	}

	$effect(() => {
		if (open && tab === "manual") {
			updateIconForLoader();
		}
	});
</script>

{#snippet manualFooter()}
	<div class="footer-actions">
		<button
			type="button"
			class="btn-secondary"
			onclick={close}
			disabled={loading && !creatingVersion}
		>
			{creatingVersion ? t("createInstance.hideDownload") : t("createInstance.cancel")}
		</button>
		<button
			type="button"
			class="btn-primary"
			onclick={handleManualCreate}
			disabled={loading ||
				versionLoading ||
				!name.trim() ||
				!finalVersionId}
		>
			{#if loading}<Loading size={16} />{:else}<Icon name="nav:create" size={16} />{/if}
			{creatingVersion
				? `${t("versionDownloader.downloading")} ${createProgress ? getOverallPct(createProgress) : 0}%`
				: loading
				? t("createInstance.creatingBtn")
				: t("createInstance.createBtn")}
		</button>
	</div>
{/snippet}

<ModalBase
	bind:open
	animateResize
	scrollBody={mode === "manual"}
	showScrollToTop={mode === "pack" && (tab === "modrinth" || tab === "curseforge")}
	title={t("createInstance.title")}
	width={mode === "pack" || mode === "manual" ? "800px" : "700px"}
	closeDisabled={loading && !creatingVersion}
	onescape={() => !creatingVersion && goBackOnEscape()}
	onclose={close}
	footer={mode === "manual" ? manualFooter : undefined}
>
	{#snippet headerActions()}
		{#if mode !== "choose"}
			<button
				type="button"
				class="change-method"
			disabled={loading}
				onclick={chooseMethod}
			>
				<Icon name="ui:chevron-left" size={14} />
				{t("modpack.changeMethod")}
			</button>
		{/if}
	{/snippet}

	{#if error}
		<div class="step-error">{error}</div>
	{/if}

	{#if mode === "choose"}
		<div class="creation-choices" in:fade|global={{ duration: contentDuration }}>
			<button
				type="button"
				class="choice"
				onclick={() => {
					mode = "manual";
					tab = "manual";
				}}
			>
				<span class="choice-heading">
					<span class="choice-icon"
						><Icon name="nav:create" size={24} /></span
					>
					<span class="choice-arrow"
						><Icon name="ui:chevron-right" size={18} /></span
					>
				</span>
				<span class="choice-copy">
					<strong>{t("modpack.fromScratch")}</strong>
					<span class="choice-description"
						>{t("modpack.fromScratchHint")}</span
					>
				</span>
			</button>
			<button
				type="button"
				class="choice"
				onclick={() => {
					mode = "pack";
					tab = "modrinth";
				}}
			>
				<span class="choice-heading">
					<span class="choice-icon"
						><Icon name="instance:puzzle" size={24} /></span
					>
					<span class="choice-arrow"
						><Icon name="ui:chevron-right" size={18} /></span
					>
				</span>
				<span class="choice-copy">
					<strong>{t("modpack.basedOnPack")}</strong>
					<span class="choice-description"
						>{t("modpack.basedOnPackHint")}</span
					>
				</span>
			</button>
		</div>
		<div class="secondary-imports" in:fade|global={{ duration: contentDuration }}>
			<button
				type="button"
				class="import-choice"
				onclick={() => (mode = "instance")}
			>
				<span class="import-icon"
					><Icon name="instance:folder" size={20} /></span
				>
				<span class="choice-copy">
					<strong>{t("createInstance.importInstanceTab")}</strong>
					<span class="choice-description"
						>{t("modpack.importInstanceHint")}</span
					>
				</span>
				<span class="choice-arrow"
					><Icon name="ui:chevron-right" size={16} /></span
				>
			</button>
			<button
				type="button"
				class="import-choice"
				onclick={() => (mode = "migration")}
			>
				<span class="import-icon"
					><Icon name="ui:copy" size={20} /></span
				>
				<span class="choice-copy">
					<strong>{t("migration.title")}</strong>
					<span class="choice-description"
						>{t("modpack.migrateInstanceHint")}</span
					>
				</span>
				<span class="choice-arrow"
					><Icon name="ui:chevron-right" size={16} /></span
				>
			</button>
		</div>
	{:else}
		{#if mode === "pack"}
			<div class="tab-bar" role="tablist" in:fade|global={{ duration: contentDuration }}>
				{#each TABS as tabItem (tabItem.id)}
					<button
						type="button"
						class="tab-btn"
						role="tab"
						aria-selected={tab === tabItem.id}
						class:active={tab === tabItem.id}
						disabled={loading}
						onclick={() => (tab = tabItem.id)}
					>
						<Icon name={tabItem.iconName} size={18} />
						<span>{tabItem.label}</span>
					</button>
				{/each}
			</div>
		{/if}

		{#key `${mode}:${tab}`}
			<div
				class="step-content"
				class:manual-content={mode === "manual"}
				in:fade|global={{ duration: contentDuration }}
			>
				{#if mode === "instance"}
					<InstanceImportStep
						onImported={reset}
						initialPath={instanceZipPath}
					/>
				{:else if mode === "migration"}
					<LauncherMigrationStep onImported={reset} />
				{:else if tab === "modrinth"}
					<ModrinthModpackBrowser
						bind:this={modrinthBrowser}
						onInstallStarted={handleModpackInstallStarted}
						onInstallFailed={handleModpackInstallFailed}
						onInstalled={handleModpackInstalled}
					/>
				{:else if tab === "curseforge"}
					<CurseForgeModpackBrowser
						bind:this={curseForgeBrowser}
						onInstallStarted={handleModpackInstallStarted}
						onInstallFailed={handleModpackInstallFailed}
						onInstalled={handleModpackInstalled}
					/>
				{:else if tab === "local"}
					<ModpackImportStep
						bind:loading
						bind:name
						onImported={reset}
						initialPath={mrpackPath}
					/>
				{:else}
					<div class="create-layout" inert={loading}>
						<div class="identity-row">
							<IconPicker
								bind:selectedIcon
								disabled={loading}
								onupload={handleIconUpload}
								onselect={() => (customIconPath = null)}
							/>
							<div class="input-group">
								<label class="input-label" for="instance-name">
									{t("createInstance.nameLabel")}
								</label>
								<input
									id="instance-name"
									type="text"
									class="text-input"
									class:error={nameMsg}
									maxlength={MAX_INSTANCE_NAME_LEN}
									placeholder={t(
										"createInstance.customNamePlaceholder",
									)}
									bind:value={name}
									disabled={loading}
									oninput={() => (nameMsg = null)}
									onkeydown={(e) =>
										e.key === "Enter" &&
										handleManualCreate()}
								/>
								{#if nameMsg}
									<span class="input-error">{t(nameMsg)}</span
									>
								{/if}
							</div>
						</div>
						<VersionSelectorStep
							includeAvailable
							showInstalledFilter
							onDownload={(loader, mc, build) => void enqueueVersion(loader, mc, build)}
							{pendingDownloads}
							downloadDisabled={loading}
							bind:loading={versionLoading}
							bind:selectedLoader
							bind:selectedMcVersion
							bind:selectedLoaderVersion
						/>
					</div>
				{/if}
			</div>
		{/key}
	{/if}
</ModalBase>

<style>
	.change-method {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 6px 8px;
		border: 1px solid transparent;
		border-radius: var(--border-radius-sm);
		background: transparent;
		color: var(--text-secondary);
		font: inherit;
		font-size: 0.75rem;
		cursor: pointer;
	}
	.change-method:hover:not(:disabled),
	.change-method:focus-visible {
		background: var(--surface-hover);
		color: var(--text-primary);
		border-color: var(--border);
	}
	.creation-choices {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 16px;
	}
	.choice {
		display: flex;
		flex-direction: column;
		gap: 16px;
		text-align: left;
		padding: 20px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-lg, 8px);
		background: var(--bg-card);
		color: var(--text-primary);
		cursor: pointer;
		font: inherit;
		min-width: 0;
		transition:
			border-color 0.15s,
			background-color 0.15s;
	}
	.choice:hover,
	.choice:focus-visible,
	.import-choice:hover,
	.import-choice:focus-visible {
		border-color: var(--accent);
		background-color: var(--surface-hover);
	}
	.choice:focus-visible,
	.import-choice:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
	.choice-heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.choice-icon {
		display: grid;
		place-items: center;
		width: 44px;
		height: 44px;
		border: 1px solid rgba(var(--accent-rgb), 0.15);
		border-radius: var(--border-radius-lg, 8px);
		background: rgba(var(--accent-rgb), 0.07);
		color: var(--accent);
	}
	.choice-arrow {
		display: flex;
		flex-shrink: 0;
		color: var(--text-muted);
	}
	.choice-copy {
		display: flex;
		flex-direction: column;
		gap: 6px;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.choice-copy strong {
		font-size: 0.9rem;
		font-weight: 600;
	}
	.choice-description {
		color: var(--text-secondary);
		font-size: 0.78rem;
		line-height: 1.5;
	}
	.secondary-imports {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 10px;
		padding-top: 16px;
		border-top: 1px solid var(--border);
	}
	.import-choice {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 12px;
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-lg, 8px);
		background: transparent;
		color: var(--text-primary);
		font: inherit;
		text-align: left;
		cursor: pointer;
		transition:
			border-color 0.15s,
			background-color 0.15s;
	}
	.import-icon {
		display: grid;
		place-items: center;
		flex-shrink: 0;
		width: 32px;
		height: 32px;
		color: var(--text-secondary);
	}
	.import-choice .choice-copy {
		flex: 1;
		gap: 3px;
	}
	.import-choice strong {
		font-size: 0.78rem;
	}
	.import-choice .choice-description {
		font-size: 0.7rem;
	}
	@media (max-width: 500px) {
		.creation-choices,
		.secondary-imports {
			grid-template-columns: 1fr;
		}
		.choice {
			flex-direction: row;
			align-items: center;
			gap: 12px;
			padding: 14px;
		}
		.choice-heading .choice-arrow {
			display: none;
		}
		.choice-icon {
			width: 36px;
			height: 36px;
		}
		.choice-copy {
			gap: 4px;
		}
	}
	.step-error {
		color: var(--color-error);
		font-size: 0.8rem;
		background: rgba(var(--color-error-rgb), 0.1);
		border: 1px solid rgba(var(--color-error-rgb), 0.2);
		border-radius: 6px;
		padding: 10px;
		text-align: center;
		font-weight: 500;
	}

	.tab-bar {
		display: flex;
		gap: 4px;
		padding: 4px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-lg, 8px);
		background: var(--bg-input);
	}

	.tab-btn {
		flex: 1;
		min-width: 0;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		padding: 8px;
		border: 1px solid transparent;
		border-radius: var(--border-radius-sm);
		background: transparent;
		color: var(--text-secondary);
		font-size: 0.78rem;
		font-weight: 600;
		cursor: pointer;
		transition:
			color 0.15s ease,
			border-color 0.15s ease,
			background 0.15s ease;
	}

	.tab-btn:hover {
		color: var(--text-primary);
		background: var(--surface-hover);
	}

	.tab-btn.active {
		color: var(--text-primary);
		border-color: var(--accent);
		background: rgba(var(--accent-rgb), 0.1);
	}

	.step-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
		min-height: 320px;
	}

	.create-layout {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.manual-content {
		min-height: 0;
	}

	.identity-row {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 16px;
	}

	.input-group {
		min-width: 0;
	}

	.input-group .text-input {
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
		min-height: 36px;
		padding: 8px 12px;
	}

	.input-group :global(.text-input.error) {
		border-color: var(--color-error) !important;
		box-shadow: 0 0 0 1px var(--color-error) !important;
	}

	.input-error {
		font-size: 0.7rem;
		color: var(--color-error);
		margin-top: 4px;
		display: block;
	}

	.input-label {
		font-size: 0.7rem;
		font-weight: 600;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin-bottom: 5px;
		display: block;
	}

	.footer-actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		width: 100%;
		gap: 10px;
	}

	.footer-actions button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
	}
</style>
