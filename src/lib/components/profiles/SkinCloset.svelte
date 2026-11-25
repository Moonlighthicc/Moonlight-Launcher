<script lang="ts">
	import { t } from "$lib/i18n";
	import { showError } from "$lib/state/state.svelte";
	import { open } from "@tauri-apps/plugin-dialog";
	import {
		getSkinCloset,
		getSkinPreviewData,
		importSkinToCloset,
		openSkinClosetDir,
		removeSkinFromCloset,
		renameSkinInCloset,
	} from "$lib/api/cubicApi";
	import { detectSkinModelFromUrl } from "$lib/utils/skinModel";
	import type { SkinClosetEntry } from "$lib/types/types";
	import SkinClosetItem from "./SkinClosetItem.svelte";
	import Icon from "$lib/icons/Icon.svelte";

	interface Props {
		uuid: string;
		revision: number;
		equipRequiresLicense: boolean;
		onEquip?: (file: string, model: "classic" | "slim") => Promise<void>;
	}

	let { uuid, revision, equipRequiresLicense, onEquip }: Props = $props();

	let entries = $state<(SkinClosetEntry & { scope: string })[]>([]);
	let loading = $state(true);
	let processing = $state(false);
	let mutation = $state(0);
	let loadError = $state(false);

	async function change(action: () => Promise<unknown>, reload = true) {
		if (processing) return;
		processing = true;
		try {
			await action();
			if (reload) mutation++;
		} catch (err) {
			showError(t("errors.title"), String(err));
		} finally {
			processing = false;
		}
	}

	function importPng() {
		void change(async () => {
			const file = await open({
				multiple: false,
				filters: [{ name: "Minecraft Skin", extensions: ["png"] }],
			});
			if (typeof file !== "string") return;
			const preview = await getSkinPreviewData(file);
			const model = (await detectSkinModelFromUrl(preview)) ?? "classic";
			await importSkinToCloset(file, model);
		});
	}

	$effect(() => {
		const scopes = uuid ? ["library", uuid] : ["library"];
		// Refresh after imports, edits or a copy made in the server section.
		void [revision, mutation];
		let current = true;
		loading = true;
		loadError = false;
		void Promise.all(
			scopes.map(async (scope) =>
				(await getSkinCloset(scope)).map((entry) => ({
					...entry,
					scope,
				})),
			),
		)
			.then((groups) => {
				if (current)
					entries = groups
						.flat()
						.sort((a, b) => b.saved_at - a.saved_at);
			})
			.catch(() => {
				if (current) loadError = true;
			})
			.finally(() => {
				if (current) loading = false;
			});
		return () => {
			current = false;
		};
	});
</script>

<div class="closet-section">
	<h5 class="subsection-title">
		{t("userMenu.skinCape.skinCloset.savedSkins")}
		{#if entries.length > 0}
			<span class="closet-count">{entries.length}</span>
		{/if}
	</h5>
	<div class="closet-actions">
		<button
			type="button"
			class="btn-secondary folder-button"
			onclick={() => void change(openSkinClosetDir, false)}
			disabled={processing}
		>
			<Icon name="instance:folder" size={15} />
			{t("userMenu.skinCape.skinCloset.openFolder")}
		</button>
		<button type="button" class="btn-secondary" onclick={importPng} disabled={processing}>
			{t("userMenu.skinCape.skinCloset.importPng")}
		</button>
	</div>

	{#if loadError}
		<p class="closet-empty" role="alert">
			{t("userMenu.skinCape.skinCloset.localError")}
		</p>
		<button type="button" class="btn-secondary" onclick={() => mutation++}
			>{t("common.retry")}</button
		>
	{:else if loading && entries.length === 0}
		<div class="closet-loading">
			<span class="spinner"></span>
		</div>
	{:else if entries.length === 0}
		<p class="closet-empty">
			{t("userMenu.skinCape.skinCloset.noSavedSkins")}
		</p>
	{:else}
		<div class="closet-grid" role="list">
			{#each entries as entry (`${entry.scope}:${entry.id}`)}
				<div role="listitem">
					<SkinClosetItem
						{entry}
						scope={entry.scope}
						processing={processing || loading}
						{equipRequiresLicense}
						onEquip={onEquip &&
							(() =>
								change(
									() =>
										onEquip!(
											entry.local_path,
											entry.variant.toLowerCase() ===
												"slim"
												? "slim"
												: "classic",
										),
									false,
								))}
						onRemove={() =>
							change(() =>
								removeSkinFromCloset(entry.scope, entry.id),
							)}
						onRename={(alias) =>
							change(() =>
								renameSkinInCloset(
									entry.scope,
									entry.id,
									alias,
								),
							)}
					/>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.closet-section {
		display: flex;
		flex-direction: column;
		gap: 14px;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--border-radius);
		padding: 14px;
	}

	.subsection-title {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 700;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.6px;
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.closet-count {
		background: var(--accent);
		color: var(--accent-text);
		padding: 2px 8px;
		border-radius: 999px;
		font-size: 0.65rem;
	}

	.closet-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.folder-button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	.closet-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
	}

	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid var(--border);
		border-top-color: var(--accent);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.closet-empty {
		margin: 0;
		font-size: 0.8rem;
		color: var(--text-muted);
		padding: 12px 0;
	}

	.closet-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(128px, 1fr));
		gap: 12px;
	}

	@media (max-width: 520px) {
		.closet-grid {
			grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
			gap: 10px;
		}
	}
</style>
