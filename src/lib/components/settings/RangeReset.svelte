<script lang="ts">
	import { t } from "$lib/i18n";
	let {
		label,
		disabled = false,
		onclick,
	}: {
		label: string;
		disabled?: boolean;
		onclick: () => unknown;
	} = $props();
	const name = $derived(
		`${t("settings.launcher.performance.reset")}: ${label}`,
	);
</script>

<button type="button" aria-label={name} title={name} {disabled} {onclick}>
	<svg
		viewBox="0 0 24 24"
		width="16"
		height="16"
		fill="none"
		stroke="currentColor"
		stroke-width="1.8"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d="M3 4v6h6M3.4 10a9 9 0 1 1 1.5 8" />
	</svg>
</button>

<style>
	button {
		display: inline-grid;
		place-items: center;
		flex: none;
		width: 30px;
		height: 30px;
		padding: 0;
		color: var(--text-secondary);
		background: var(--bg-input);
		border: 1px solid var(--border-color);
		border-radius: var(--border-radius-sm);
		cursor: pointer;
	}
	button:hover:not(:disabled) {
		color: var(--accent);
		border-color: var(--accent);
	}
	button:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
	button:disabled {
		opacity: 0.45;
		cursor: default;
	}
</style>
