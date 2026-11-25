<script lang="ts">
	import { fly, slide } from "svelte/transition";
	import { cubicInOut } from "svelte/easing";
	import { t } from "$lib/i18n";
	import {
		downloads,
		getActiveDownloadCount,
		getDoneDownloadCount,
		getOverallPct,
		getStatusLabel,
		cancelDownloadTask,
		retryDownloadTask,
		dismissDownloadError,
		clearModpackDownloadErrors,
		matchesModpackDownloadError,
		downloadQueueUi,
	} from "$lib/state/downloadQueueState.svelte";
	import DownloadQueueHeader from "./DownloadQueueHeader.svelte";
	import { dismissModpackInstall, modpackInstalls, retryModpackInstall } from "$lib/state/modpackInstalls.svelte";
	import DownloadQueueItem from "./DownloadQueueItem.svelte";
	import ChevronDownIcon from "$lib/icons/ChevronDownIcon.svelte";
	import { animDuration } from "$lib/utils/animations";
	import { sidebarPopover, sidebarPopoverStyle } from "$lib/utils/popoverPlacement";

	let {
		popover = false,
		popoverUp = false,
		popoverRight = false,
		popoverSide = "none",
		dismissOnOutsideClick = false,
		compactProgress = false,
	}: { popover?: boolean; popoverUp?: boolean; popoverRight?: boolean; popoverSide?: "right" | "left" | "none"; dismissOnOutsideClick?: boolean; compactProgress?: boolean } = $props();

	let rootEl = $state<HTMLDivElement>();
	let bodyEl = $state<HTMLDivElement>();
	let popoverStyle = $state("");
	let enterOffset = $state({ x: 0, y: 0 });
	function queueTransition(node: HTMLElement) {
		if (popover) return fly(node, { ...enterOffset, duration: animDuration(150) });
		return slide(node, { duration: animDuration(200), easing: cubicInOut });
	}
	function portalBody(node: HTMLElement, enabled: boolean) {
		const parent = node.parentNode;
		const marker = document.createComment("download queue");
		parent?.insertBefore(marker, node);
		function move(usePortal: boolean) {
			if (usePortal) document.body.appendChild(node);
			else marker.after(node);
		}
		move(enabled);
		return { update: move, destroy() { marker.remove(); node.remove(); } };
	}

	function updatePopoverPosition() {
		if (!popover || !rootEl || !bodyEl) return;
		const rect = (rootEl.querySelector(".sd-header") ?? rootEl).getBoundingClientRect();
		const width = Math.min(300, window.innerWidth - 16);
		const maxHeight = Math.min(window.innerHeight * 0.6, 420);
		const panelHeight = Math.min(bodyEl.scrollHeight + 2, maxHeight);
		const position = sidebarPopover(rect, { width: window.innerWidth, height: window.innerHeight },
			{ width, height: panelHeight, maxHeight }, { side: popoverSide, up: popoverUp, alignRight: popoverRight });
		popoverStyle = sidebarPopoverStyle(position);
		enterOffset = { x: position.x, y: position.y };
	}
	$effect(() => {
		if (open && popover && rootEl && bodyEl) {
			updatePopoverPosition();
			const observer = new ResizeObserver(updatePopoverPosition);
			observer.observe(rootEl);
			observer.observe(bodyEl);
			window.addEventListener("resize", updatePopoverPosition);
			window.addEventListener("scroll", updatePopoverPosition, true);
			return () => {
				observer.disconnect();
				window.removeEventListener("resize", updatePopoverPosition);
				window.removeEventListener("scroll", updatePopoverPosition, true);
			};
		}
	});

	function handleOutsidePointerDown(event: PointerEvent) {
		if (!dismissOnOutsideClick || !open || !rootEl) return;
		if (!(event.target instanceof Node)) return;
		const clickedHeader = rootEl.querySelector(".sd-header")?.contains(event.target) ?? false;
		const clickedPanel = bodyEl?.contains(event.target) ?? false;
		if (!clickedHeader && !clickedPanel) {
			collapseQueue();
		}
	}

	$effect(() => {
		if (open && popover && dismissOnOutsideClick) {
			document.addEventListener("pointerdown", handleOutsidePointerDown, true);
			return () => document.removeEventListener("pointerdown", handleOutsidePointerDown, true);
		}
	});

	let open = $derived(downloadQueueUi.expanded);
	let activeCount = $derived(getActiveDownloadCount());
	let doneCount = $derived(getDoneDownloadCount());
	let packFailures = $derived([...modpackInstalls].filter(([, task]) => task.error !== null));
	let queueItems = $derived([...downloads.values()].filter((item) =>
		!packFailures.some(([, task]) => matchesModpackDownloadError(item, task))));
	let errorCount = $derived(queueItems.filter((item) => item.error).length + packFailures.length);

	let activeProgress = $derived.by(() => {
		const active = [...downloads.values()].filter((item) => !item.done && !item.error);
		if (active.length === 0) return 0;
		return Math.round(
			active.reduce((sum, item) => sum + getOverallPct(item), 0) / active.length,
		);
	});

	// Новая задача может автоматически раскрыть очередь, но ручное сворачивание
	// сохраняется. Общий счётчик живёт вне компонента, поэтому remount при смене
	// compact/normal не воспринимается как появление новой задачи.
	$effect(() => {
		const count = activeCount;
		if (count > downloadQueueUi.lastActiveCount && !downloadQueueUi.manuallyCollapsed) {
			downloadQueueUi.expanded = true;
		}
		downloadQueueUi.lastActiveCount = count;
	});

	function collapseQueue(restoreFocus = false) {
		downloadQueueUi.expanded = false;
		downloadQueueUi.manuallyCollapsed = true;
		if (restoreFocus) rootEl?.querySelector<HTMLButtonElement>(".sd-header")?.focus({ preventScroll: true });
	}

	function toggleQueue() {
		if (downloadQueueUi.expanded) {
			collapseQueue();
		} else {
			downloadQueueUi.expanded = true;
			downloadQueueUi.manuallyCollapsed = false;
		}
	}
</script>

<div bind:this={rootEl} class="sd-root" class:popover>
	<DownloadQueueHeader
		{open}
		{activeCount}
		{doneCount}
		{errorCount}
		progress={activeProgress}
		ontoggle={toggleQueue}
	/>
        {#if compactProgress && popover && activeCount > 0}
                <span class="sd-compact-progress" aria-label={`${activeProgress}%`}>
                        <span style:width={`${activeProgress}%`}></span>
                </span>
        {/if}
	{#if open}
		<div
			bind:this={bodyEl}
			use:portalBody={popover}
			class="sd-body"
			class:sd-popover={popover}
			style={popover ? popoverStyle : undefined}
			transition:queueTransition>
			{#if popover}
				<div class="sd-popover-toolbar">
					<button type="button" class="sd-collapse" onclick={() => collapseQueue(true)}>
						<ChevronDownIcon size={14} />
						{t("sidebar.collapseDownloads")}
					</button>
				</div>
			{/if}
			{#if queueItems.length === 0 && packFailures.length === 0}
				<div class="sd-empty">{t("sidebar.noDownloadDesc")}</div>
			{:else}
				{#each queueItems as item (item.version)}
					{@const overall = getOverallPct(item)}
					{@const label = item.deleting ? t("sidebar.cancelDeleting") : item.cancelling
						? t("sidebar.cancelling")
						: !item.done && !item.error
							? getStatusLabel(item)
							: null}
					<DownloadQueueItem
						version={item.version}
						{overall}
						done={item.done}
						error={item.error}
						statusLabel={label}
						bytesCurrent={item.bytesCurrent ?? 0}
						bytesTotal={item.bytesTotal ?? 0}
						speedBytesPerSecond={item.speedBytesPerSecond ?? 0}
						speedHistory={item.speedHistory}
						currentItem={item.currentItem ?? null}
						itemProgress={item.activeType && !item.bytesTotal ? item.segs[item.activeType] : undefined}
						cancellable={item.cancellable !== false}
						cancelling={item.cancelling === true}
						onCancel={() => void cancelDownloadTask(item.version)}
						onRetry={item.error && item.retry ? () => void retryDownloadTask(item.version) : undefined}
						onDismiss={item.error ? () => dismissDownloadError(item.version) : undefined}
					/>
				{/each}
				{#each packFailures as [key, task] (key)}
					<DownloadQueueItem
						version={task.archive}
						name={task.name}
						overall={0}
						done={true}
						error={task.error}
						statusLabel={null}
						onRetry={() => { clearModpackDownloadErrors(task); void retryModpackInstall(key); }}
						onDismiss={() => { clearModpackDownloadErrors(task); dismissModpackInstall(key); }}
					/>
				{/each}
			{/if}
		</div>
	{/if}
</div>

<style>
	.sd-root {
		border-bottom: 1px solid var(--border-color);
	}

	.sd-body {
		overflow: hidden;
	}

	/* Floating queues share the same surface on every dock. */
	.sd-root.popover {
		position: relative;
		border-bottom: none;
	}

	.sd-popover {
		position: fixed;
		z-index: 200;
		box-sizing: border-box;
		width: min(300px, calc(100vw - 16px));
		max-height: min(60vh, 420px);
		overflow-y: auto;
		background: var(--bg-sidebar);
		left: auto;
		right: 0;
		border: 1px solid var(--border);
		border-radius: var(--border-radius);
		box-shadow: 0 4px 12px rgb(0 0 0 / 22%);
	}

	.sd-popover-toolbar {
		position: sticky;
		top: 0;
		z-index: 1;
		padding: 4px 6px;
		background: var(--bg-sidebar);
		border-bottom: 1px solid var(--border);
	}

	.sd-collapse {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 4px 6px;
		border: 0;
		border-radius: var(--border-radius-sm);
		background: none;
		color: var(--text-secondary);
		font-size: 0.75rem;
		cursor: pointer;
	}
	.sd-collapse:hover { background: var(--surface-selected); }

        .sd-root.popover .sd-compact-progress {
                position: absolute;
                left: 50%;
                bottom: 2px;
                transform: translateX(-50%);
                width: 19px;
                height: 2px;
                overflow: hidden;
                border-radius: 2px;
                background: var(--border);
                pointer-events: none;
                z-index: 2;
        }

        .sd-root.popover .sd-compact-progress > span {
                display: block;
                height: 100%;
                border-radius: inherit;
                background: var(--accent);
                transition: width 180ms ease;
        }

	.sd-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 130px;
		padding: 16px 10px;
		text-align: center;
		font-size: 0.68rem;
		color: var(--text-muted);
		line-height: 1.4;
	}
</style>
