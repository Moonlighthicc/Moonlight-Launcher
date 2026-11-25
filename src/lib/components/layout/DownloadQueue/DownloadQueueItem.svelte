<script lang="ts">
	import { t } from "$lib/i18n";
	import CheckIcon from "$lib/icons/CheckIcon.svelte";
	import { launcherStore } from "$lib/state/state.svelte";
	import CloseIcon from "$lib/icons/CloseIcon.svelte";
	import { formatBytes } from "$lib/utils/formatBytes";

	let {
		version,
		overall,
		done,
		error,
		statusLabel,
		bytesCurrent = 0,
		bytesTotal = 0,
		speedBytesPerSecond = 0,
		currentItem = null,
		itemProgress,
		cancellable = true,
		cancelling = false,
		onCancel,
	}: {
		version: string;
		overall: number;
		done: boolean;
		error: string | null;
		statusLabel: string | null;
		bytesCurrent?: number;
		bytesTotal?: number;
		speedBytesPerSecond?: number;
		currentItem?: string | null;
		itemProgress?: { current: number; total: number };
		cancellable?: boolean;
		cancelling?: boolean;
		onCancel?: () => void;
	} = $props();

	const KIND_LABELS = [
		["mods-", "sidebar.kindMods"],
		["resourcepacks-", "sidebar.kindResourcepacks"],
		["shaderpacks-", "sidebar.kindShaderpacks"],
	] as const;

	function formatSpeed(bytesPerSecond: number): string {
		const bits = Math.max(0, bytesPerSecond * 8);
		if (bits >= 1_000_000_000) {
			return t("downloadProgress.speedGbps", {
				value: (bits / 1_000_000_000).toFixed(1),
			});
		}
		if (bits >= 1_000_000) {
			return t("downloadProgress.speedMbps", {
				value: (bits / 1_000_000).toFixed(1),
			});
		}
		if (bits >= 1_000) {
			return t("downloadProgress.speedKbps", {
				value: Math.round(bits / 1_000),
			});
		}
		return t("downloadProgress.speedBps", { value: Math.round(bits) });
	}

	function instanceName(uuid: string): string | undefined {
		return launcherStore.loadedInstances.find((i) => i.uuid === uuid)?.name;
	}

	const downloadLabel = $derived.by(() => {
		if (version.startsWith("modpack-")) {
			return (
				instanceName(version.slice("modpack-".length)) ??
				t("sidebar.downloadingMods")
			);
		}
		if (version === "mods") return t("sidebar.downloadingMods");
		if (
			version.startsWith("mrpack-dl-") ||
			version.startsWith("cfpack-dl-")
		) {
			return t("sidebar.modpackFile");
		}
		for (const [prefix, key] of KIND_LABELS) {
			if (version.startsWith(prefix)) {
				const name = instanceName(version.slice(prefix.length));
				return name ? `${t(key)} · ${name}` : t(key);
			}
		}
		return version;
	});
</script>

<div class="sd-item" class:done class:error>
	<div class="sd-item-header">
		<span class="sd-item-left">
			{#if error}
				<span class="sd-error-icon">!</span>
			{:else if done}
				<CheckIcon size={8} />
			{:else}
				<span class="sd-spinner-sm"></span>
			{/if}
			<div class="sd-version-wrap">
				<span class="sd-version">{downloadLabel}</span>
				{#if statusLabel}
					<span class="sd-status-label">{statusLabel}</span>
				{/if}
			</div>
		</span>
		{#if error}
			<span class="sd-pct error">{t("sidebar.failed")}</span>
		{:else}
			<span class="sd-pct" class:done>{overall}%</span>
		{/if}
		{#if !done && !error && !cancelling && cancellable && onCancel}
			<button
				type="button"
				class="sd-cancel"
				title={t("sidebar.cancelDownload")}
				aria-label={t("sidebar.cancelDownload")}
				onclick={onCancel}
			>
				<CloseIcon size={10} />
			</button>
		{/if}
	</div>
	{#if error}
		<div class="sd-error-msg">{error}</div>
	{:else}
		<div class="sd-progress-track">
			<div
				class="sd-progress-fill"
				class:done
				style:width="{Math.min(100, Math.max(0, overall))}%"
			></div>
		</div>
		{#if !done && (bytesTotal > 0 || currentItem || (itemProgress?.total ?? 0) > 0)}
			<div class="sd-transfer-meta">
				{#if bytesTotal > 0}
					<span>{formatBytes(Math.min(Math.max(0, bytesCurrent), bytesTotal))} / {formatBytes(bytesTotal)}</span>
					{#if speedBytesPerSecond > 0}<span>{formatSpeed(speedBytesPerSecond)}</span>{/if}
				{:else if itemProgress && itemProgress.total > 0}
					<span>{Math.min(itemProgress.current, itemProgress.total)} / {itemProgress.total}</span>
				{/if}
				{#if currentItem}
					<span class="sd-current-file" title={currentItem}>
						{currentItem}
					</span>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style>
	.sd-item {
		padding: 8px 10px;
		border-bottom: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.sd-item:last-child {
		border-bottom: none;
	}

	.sd-item-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.sd-item-left {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
		flex: 1;
	}

	.sd-version-wrap {
		display: flex;
		flex-direction: column;
		gap: 1px;
		min-width: 0;
	}

	.sd-version {
		font-size: 0.72rem;
		font-weight: 700;
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.sd-status-label {
		font-size: 0.6rem;
		color: var(--text-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.sd-transfer-meta {
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.6rem;
		line-height: 1.25;
		font-variant-numeric: tabular-nums;
		color: var(--text-muted);
	}

	.sd-current-file {
		min-width: 0;
		flex: 1;
		color: var(--text-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.sd-pct {
		font-size: 0.65rem;
		font-weight: 700;
		color: var(--text-muted);
		flex-shrink: 0;
	}

	.sd-pct.done {
		color: var(--color-success);
	}

	.sd-pct.error {
		color: var(--color-error);
	}

	.sd-cancel {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		padding: 3px;
		margin-left: 2px;
		background: none;
		border: none;
		border-radius: var(--border-radius-sm);
		color: var(--text-muted);
		cursor: pointer;
		transition:
			background 0.15s ease,
			color 0.15s ease;
	}

	.sd-cancel:hover {
		color: var(--color-error);
		background: color-mix(in srgb, var(--color-error) 12%, transparent);
	}

	.sd-error-icon {
		width: 8px;
		height: 8px;
		background: var(--color-error);
		color: var(--color-on-error);
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 6px;
		font-weight: 900;
		line-height: 1;
		flex-shrink: 0;
	}

	.sd-error-msg {
		font-size: 0.65rem;
		color: var(--color-error);
		word-break: break-word;
		line-height: 1.3;
	}

	.sd-item.error {
		background: color-mix(in srgb, var(--color-error) 5%, transparent);
	}

	.sd-progress-track {
		width: 100%;
		height: 3px;
		background: var(--bg-input);
		border-radius: 2px;
		overflow: hidden;
	}

	.sd-progress-fill {
		height: 100%;
		background: var(--accent);
		border-radius: 2px;
		transition: width 0.35s cubic-bezier(0.4, 0, 0.2, 1);
	}

	.sd-progress-fill.done {
		background: var(--color-success);
	}

	.sd-spinner-sm {
		width: 8px;
		height: 8px;
		border: 1.5px solid var(--border);
		border-top-color: var(--text-muted);
		border-radius: 50%;
		animation: sd-spin 0.7s linear infinite;
		will-change: transform;
		flex-shrink: 0;
		display: block;
	}

	@keyframes sd-spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (max-width: 650px) {
		.sd-item {
			padding: 6px 4px;
			align-items: center;
		}

		.sd-item-header {
			justify-content: center;
		}

		.sd-item .sd-spinner-sm {
			width: 10px;
			height: 10px;
		}
	}
</style>
