<script lang="ts">
	import type { Snippet } from "svelte";
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	import { openUrl } from "$lib/api/cubicApi";

	let { updatePreference, onCheckUpdates }: {
		updatePreference: Snippet;
		onCheckUpdates: () => void;
	} = $props();

	const RELEASES_URL = "https://github.com/Moonlighthicc/Moonlight-Launcher/releases";
	const ORIGINAL_REPO_URL = "https://github.com/CubicLauncherDevs/CubicLauncher";
</script>

<div class="authorship">
	<div class="card follow">
		<p class="strong">{t("settings.launcher.updatesTitle")}</p>
		{@render updatePreference()}
		<div class="update-actions">
			<button type="button" class="link-btn" onclick={onCheckUpdates}>
				{t("updater.check")}
			</button>
			<button type="button" class="link-btn" onclick={() => openUrl(RELEASES_URL)}>
				<Icon name="instance:external-link" size={16} />
				{t("settings.authorship.githubReleases")}
			</button>
		</div>
	</div>

	<div class="card credits">
		<p>{t("settings.authorship.forkNotice")}</p>
		<p class="strong">{t("settings.authorship.originalAuthors")}</p>
		<button
			type="button"
			class="link-btn"
			onclick={() => openUrl(ORIGINAL_REPO_URL)}
		>
			<Icon name="instance:external-link" size={16} />
			{t("settings.authorship.originalRepo")}
		</button>
		<p class="muted">{t("settings.authorship.referenceNote")}</p>
		<p class="muted">{t("settings.authorship.license")}</p>
	</div>
</div>

<style>
	.authorship {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.card {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 14px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		background: var(--surface-card);
	}
	.update-actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}
	p {
		margin: 0;
		font-size: 0.78rem;
		line-height: 1.5;
		color: var(--text-secondary);
		overflow-wrap: anywhere;
	}
	p.strong {
		color: var(--text-primary);
		font-weight: 600;
	}
	p.muted {
		font-size: 0.72rem;
		color: var(--text-tertiary);
	}
	.link-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		align-self: flex-start;
		padding: 8px 12px;
		background: var(--surface-input);
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		color: var(--text-primary);
		font: inherit;
		font-size: 0.78rem;
		cursor: pointer;
	}
	.link-btn:hover {
		background: var(--surface-hover);
		border-color: var(--accent);
	}
	.link-btn:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 3px;
	}
</style>
