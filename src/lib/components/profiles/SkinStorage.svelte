<script lang="ts">
	import type { Snippet } from "svelte";
	import { t } from "$lib/i18n";
	import { showSuccess } from "$lib/state/state.svelte";
	import ModalBase from "$lib/components/layout/ModalBase.svelte";
	import SkinCloset from "./SkinCloset.svelte";
	import { bumpAvatarVersion } from "$lib/state/avatarCache.svelte";
	import { uploadSkinFile } from "$lib/api/cubicApi";

	let {
		uuid,
		provider,
		server,
	}: {
		uuid: string;
		provider: "premium" | "ely" | "offline" | "authinject";
		server: Snippet<
		[() => void, (trigger: HTMLButtonElement) => void]
		>;
	} = $props();

	let localRevision = $state(0);
	let serverRevision = $state(0);
	let wardrobeOpen = $state(false);
	let wardrobeTrigger: HTMLButtonElement | null = null;
	let storageSection = $state<HTMLElement>();
	function openWardrobe(trigger: HTMLButtonElement) {
		wardrobeTrigger = trigger;
		wardrobeOpen = true;
	}
	function restoreFocus() {
		const trigger = wardrobeTrigger?.isConnected
			? wardrobeTrigger
			: storageSection?.querySelector<HTMLButtonElement>(
					'button[aria-haspopup="dialog"]',
				);
		trigger?.focus({ preventScroll: true });
	}
	const canEquip = $derived(!!uuid && provider === "premium");

	function localSaved() {
		localRevision++;
		showSuccess(
			t("userMenu.skinCape.skinCloset.title"),
			t("userMenu.skinCape.skinCloset.addedDesc"),
		);
	}

	async function equip(file: string, model: "classic" | "slim") {
		if (!canEquip) return;
		await uploadSkinFile(uuid, file, model);
		bumpAvatarVersion(uuid);
		serverRevision++;
		showSuccess(
			t("userMenu.skinCape.skinCloset.equipSkin"),
			t("userMenu.skinCape.skinCloset.equippedDesc"),
		);
	}
</script>

<section
	class="storage"
	bind:this={storageSection}
	aria-label={t("userMenu.skinCape.title")}
	inert={wardrobeOpen}
>
	{#if provider !== "offline"}
		{#key serverRevision}
			{@render server(localSaved, openWardrobe)}
		{/key}
	{:else}
		<button
			type="button"
			class="btn-secondary wardrobe-trigger"
			aria-haspopup="dialog"
			onclick={(e) => openWardrobe(e.currentTarget)}
		>
			{t("userMenu.skinCape.skinCloset.title")}
		</button>
	{/if}
</section>

<ModalBase
	bind:open={wardrobeOpen}
	title={t("userMenu.skinCape.skinCloset.title")}
	width="800px"
	focusOnOpen
	onclosed={restoreFocus}
>
	<SkinCloset
		{uuid}
		revision={localRevision}
		equipRequiresLicense={provider !== "premium"}
		onEquip={canEquip ? equip : undefined}
	/>
</ModalBase>

<style>
	.storage {
		display: flex;
		flex-direction: column;
		gap: 12px;
		min-width: 0;
	}
	.wardrobe-trigger {
		align-self: flex-start;
	}
</style>
