<script lang="ts">
	import { t } from "$lib/i18n";
	import ModalBase from "../ModalBase.svelte";

	let {
		open = $bindable(false),
		instanceName,
		onconfirm,
	}: {
		open?: boolean;
		instanceName: string;
		onconfirm: () => void | Promise<void>;
	} = $props();

	let deleting = $state(false);

	async function confirmDelete() {
		if (deleting) return;
		deleting = true;
		try {
			await onconfirm();
		} finally {
			deleting = false;
		}
	}
</script>

<ModalBase bind:open title={t("sidebar.modals.deleteTitle")}>
	<p class="confirmation-message">
		{t("sidebar.modals.deleteDesc1")}
		<strong style="color: var(--text-primary);">"{instanceName}"</strong>{t(
			"sidebar.modals.deleteDesc2",
		)}
	</p>
	{#snippet footer()}
		<button
			type="button"
			class="btn-secondary"
			disabled={deleting}
			onclick={() => (open = false)}>{t("sidebar.modals.cancel")}</button
		>
		<button
			type="button"
			class="btn-primary delete-confirm-button"
			style="background: var(--color-error); color: var(--color-on-error);"
			disabled={deleting}
			aria-busy={deleting}
			onclick={confirmDelete}
		>
			{#if deleting}
				<span class="delete-spinner" aria-hidden="true"></span>
				<span>{t("sidebar.modals.deleting", { defaultValue: "Deleting…" })}</span>
			{:else}
				{t("sidebar.modals.deleteBtn")}
			{/if}
		</button
		>
	{/snippet}
</ModalBase>

<style>
	.delete-confirm-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 0.55rem;
		transition: opacity 140ms ease, filter 140ms ease;
	}
	.delete-confirm-button:disabled {
		opacity: 0.58;
		cursor: not-allowed;
		filter: saturate(0.72);
	}
	.delete-spinner {
		width: 0.9rem;
		height: 0.9rem;
		border: 2px solid currentColor;
		border-right-color: transparent;
		border-radius: 50%;
		animation: delete-spin 700ms linear infinite;
	}
	@keyframes delete-spin { to { transform: rotate(360deg); } }
	@media (prefers-reduced-motion: reduce) {
		.delete-spinner { animation-duration: 1.8s; }
	}
</style>
