<script lang="ts">
	import { fade } from "svelte/transition";
	import { t } from "$lib/i18n";
	import CheckIcon from "$lib/icons/CheckIcon.svelte";
	import DownloadIcon from "$lib/icons/DownloadIcon.svelte";
	import ChevronDownIcon from "$lib/icons/ChevronDownIcon.svelte";
	import { animDuration } from "$lib/utils/animations";

	let {
		open = false,
		activeCount = 0,
		doneCount = 0,
		progress = 0,
		ontoggle,
	}: {
		open?: boolean;
		activeCount?: number;
		doneCount?: number;
		progress?: number;
		ontoggle?: () => void;
	} = $props();
</script>

<button
	type="button"
	class="sd-header"
	class:expanded={open}
	onclick={ontoggle}
	aria-expanded={open}
	style:--queue-header-duration={`${animDuration(200)}ms`}
>
	<span class="sd-header-left">
		{#if activeCount > 0}
			<span class="sd-spinner"></span>
			<span class="sd-label">{t("sidebar.downloading")}</span>
		{:else if doneCount > 0}
			<CheckIcon size={12} color="var(--color-success)" />
			<span class="sd-label">{doneCount} {t("sidebar.completed")}</span>
		{:else}
			<DownloadIcon size={18} />
			<span class="sd-label">{t("sidebar.noDownloads")}</span>
		{/if}
	</span>
	{#if activeCount > 0}
		<span class="sd-count-badge">{activeCount}</span>
	{/if}
	<ChevronDownIcon size={16} class={"sd-chevron" + (open ? " open" : "")} />
	{#if activeCount > 0 && !open}
		<span class="sd-collapsed-progress" aria-label={`${progress}%`} transition:fade={{ duration: animDuration(130) }}>
			<span style:width={`${progress}%`}></span>
		</span>
	{/if}
</button>

<style>
	.sd-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		background: none;
		border: none;
		color: inherit;
		padding: 8px 10px;
		cursor: pointer;
		border-radius: var(--border-radius-sm);
		box-shadow: inset 0 -1px transparent;
		transition: background var(--queue-header-duration) ease,
			box-shadow var(--queue-header-duration) ease,
			border-radius var(--queue-header-duration) ease;
		user-select: none;
		position: relative;
		overflow: hidden;
		padding-bottom: 10px;
	}

	.sd-header:hover {
		background: var(--surface-selected);
	}

	.sd-header.expanded {
		box-shadow: inset 0 -1px var(--border);
		border-radius: var(--border-radius-sm) var(--border-radius-sm) 0 0;
	}

	.sd-header-left {
		display: flex;
		align-items: center;
		gap: 7px;
		min-width: 0;
		flex: 1;
	}

	.sd-count-badge {
		min-width: 17px;
		height: 17px;
		padding: 0 4px;
		border-radius: 9px;
		background: var(--accent);
		color: var(--accent-text);
		font-size: 0.62rem;
		font-weight: 800;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.sd-label {
		font-size: 0.75rem;
		font-weight: 700;
		color: var(--text-primary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		white-space: nowrap;
	}


	.sd-collapsed-progress {
		position: absolute;
		left: 10px;
		right: 10px;
		bottom: 3px;
		height: 2px;
		border-radius: 2px;
		background: var(--border);
		overflow: hidden;
		pointer-events: none;
	}

	.sd-collapsed-progress > span {
		display: block;
		height: 100%;
		border-radius: inherit;
		background: var(--accent);
		transition: width 220ms ease;
	}
	.sd-spinner {
		width: 10px;
		height: 10px;
		border: 1.5px solid var(--border);
		border-top-color: var(--text-muted);
		border-radius: 50%;
		animation: sd-spin 0.7s linear infinite;
		will-change: transform;
		flex-shrink: 0;
	}

	@keyframes sd-spin {
		to {
			transform: rotate(360deg);
		}
	}

	:global(.sd-chevron) {
		color: var(--accent);
		flex-shrink: 0;
		transition: transform var(--queue-header-duration) ease-in-out;
	}

	:global(.sd-chevron.open) {
		transform: rotate(180deg);
	}

	@media (max-width: 650px) {
		.sd-header {
			justify-content: center;
			padding: 8px 4px;
		}

		:global(.sd-header .sd-chevron) {
			display: none;
		}
	}
</style>
