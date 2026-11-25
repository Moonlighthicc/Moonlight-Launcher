<script lang="ts">
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	let { error, onretry }: { error: string; onretry: () => void } = $props();
	let copyStatus = $state<"copied" | "copyFailed">();

	async function copy() {
		try {
			await navigator.clipboard.writeText(error);
			copyStatus = "copied";
		} catch {
			copyStatus = "copyFailed";
		}
	}
</script>

<div class="description-error">
	<p role="status">{t("createInstance.pack.descriptionError")}</p>
	<button type="button" class="diagnostic" title={t("createInstance.pack.copyDescriptionError")} onclick={copy}>{error}</button>
	<span role="status" class="copy-status">{copyStatus ? t(`market.detail.${copyStatus}`) : ""}</span>
	<button type="button" class="btn-secondary refresh" onclick={onretry}>
		<Icon name="ui:refresh" size={16} />
		{t("createInstance.pack.refreshDescription")}
	</button>
</div>

<style>
	.description-error { display: grid; gap: 6px; justify-items: start; }
	p { margin: 0; }
	.diagnostic, .copy-status { color: var(--text-secondary); font-size: 0.72rem; }
	.diagnostic { text-align: start; overflow-wrap: anywhere; padding: 0; border: 0; background: transparent; text-decoration: underline; }
	.copy-status:empty { display: none; }
	.refresh { display: inline-flex; align-items: center; gap: 6px; }
</style>