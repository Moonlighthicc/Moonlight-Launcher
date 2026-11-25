<script lang="ts">
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	const modes = ["list", "cards"] as const;

	let {
		view,
		onChange,
	}: {
		view: "list" | "cards";
		onChange: (view: "list" | "cards") => void;
	} = $props();
</script>

<div class="view-switch" role="group" aria-label={t("market.manage.view")}>
	{#each modes as value (value)}
		<button
			type="button"
			aria-pressed={view === value}
			onclick={() => onChange(value)}
		>
			<Icon
				name={value === "list" ? "log:logs" : "instance:grid"}
				size={15}
			/>
			{t(`market.manage.${value}`)}
		</button>
	{/each}
</div>

<style>
	.view-switch {
		display: inline-flex;
		gap: 2px;
		padding: 2px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius);
		background: var(--surface-input);
	}
	button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		min-height: 28px;
		padding: 4px 8px;
		border: 1px solid transparent;
		border-radius: var(--border-radius-sm);
		background: transparent;
		color: var(--text-secondary);
		font: inherit;
		font-size: 0.75rem;
		white-space: nowrap;
		cursor: pointer;
	}
	button:hover {
		color: var(--text-primary);
		background: var(--surface-hover);
	}
	button[aria-pressed="true"] {
		color: var(--text-primary);
		background: var(--surface-selected);
		border-color: var(--border-hover);
	}
	button:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}
</style>
