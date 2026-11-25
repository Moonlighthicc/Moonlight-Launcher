<script lang="ts">
	import { t } from "$lib/i18n";
	import { launcherStore } from "$lib/state/state.svelte";
	import ModalBase from "$lib/components/layout/ModalBase.svelte";
	import HtmlRenderer from "$lib/components/ui/HtmlRenderer.svelte";
	import MarkdownRenderer from "$lib/components/ui/MarkdownRenderer.svelte";
	import Loading from "$lib/icons/Loading.svelte";
	import { openUrl } from "$lib/api/cubicApi";
	import type { PackLoaders, PackVersion } from "$lib/utils/modpackVersions";

	let { version, loaders, website, onclose }: {
		version: PackVersion;
		loaders: PackLoaders;
		website?: string;
		onclose: () => void;
	} = $props();

	let open = $state(true);
	let notes = $state<string>();
	let failed = $state(false);
	let attempt = $state(0);
	const releaseDate = $derived.by(() => {
		const date = new Date(version.date);
		return Number.isFinite(date.getTime())
			? new Intl.DateTimeFormat(launcherStore.settings.language === "ru" ? "ru-RU" : "en-US", {
				dateStyle: "long", timeStyle: "short",
			}).format(date)
			: null;
	});

	async function loadNotes(load: PackLoaders["changelog"], id: string, signal: AbortSignal) {
		notes = undefined;
		failed = false;
		try {
			const result = await load(id, signal);
			if (!signal.aborted) notes = result.trim();
		} catch (error) {
			if (!signal.aborted) {
				failed = true;
				console.warn("Modpack changelog:", error);
			}
		}
	}

	$effect(() => {
		if (!open) return;
		void attempt;
		const controller = new AbortController();
		const load = loaders.changelog;
		const id = version.value;
		void loadNotes(load, id, controller.signal);
		return () => controller.abort();
	});
</script>

<ModalBase bind:open width="800px"
	title={`${t("createInstance.pack.properties")} — ${version.label}`} scrollBody {onclose}>
	<div class="metadata">
		{#if releaseDate}
			<span>{t("createInstance.pack.releasedAt")}: <time datetime={version.date}>{releaseDate}</time></span>
		{/if}
		{#if version.url}
			{@const url = version.url}
			<button type="button" class="btn-secondary" onclick={() => openUrl(url)}>
				{t("createInstance.pack.versionWebsite")}
			</button>
		{/if}
	</div>
	<section>
		<h3>{t("createInstance.pack.changelog")}</h3>
		{#if failed}
			<p role="alert">{t("createInstance.pack.changelogError")}</p>
			<button type="button" class="btn-secondary" onclick={() => attempt++}>{t("createInstance.pack.retry")}</button>
		{:else if notes === undefined}
			<p class="loading-notes" role="status"><Loading size={18} />{t("createInstance.pack.loadingChangelog")}</p>
		{:else if !notes}
			<p>{t("createInstance.pack.noChangelog")}</p>
		{:else if loaders.format === "html"}
			<HtmlRenderer source={notes} class="markdown-body" onLinkClick={openUrl} />
		{:else}
			<MarkdownRenderer source={notes} baseUrl={website} class="markdown-body" onLinkClick={openUrl} />
		{/if}
	</section>
</ModalBase>

<style>
	.metadata {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 8px;
		font-size: 0.84rem;
		color: var(--text-secondary);
	}
	section { margin-top: 12px; }
	.loading-notes { display: flex; align-items: center; gap: 8px; }
	h3 { margin: 0 0 8px; }
</style>