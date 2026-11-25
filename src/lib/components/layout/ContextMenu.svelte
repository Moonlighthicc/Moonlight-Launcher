<script lang="ts">
	import { fly } from "svelte/transition";
	import { onMount } from "svelte";
	import Icon from "$lib/icons/Icon.svelte";
	import { t } from "$lib/i18n";
	import { animDuration } from "$lib/utils/animations";
	import { clampPopoverStart, horizontalPopover } from "$lib/utils/popoverPlacement";
	import { hasOpenModal } from "./ModalBase.svelte";

	export interface ContextMenuItem {
		label: string;
		iconName?: string;
		action?: () => void | Promise<void>;
		variant?: "default" | "danger" | "accent";
		separator?: boolean;
		disabled?: boolean;
		prominent?: boolean;
		split?: boolean;
		splitLabel?: string;
		badge?: number | string;
		children?: ContextMenuItem[];
	}

	let {
		open = $bindable(false),
		x = 0,
		y = 0,
		items = [] as ContextMenuItem[],
	}: {
		open: boolean;
		x: number;
		y: number;
		items: ContextMenuItem[];
	} = $props();

	let containerEl = $state<HTMLDivElement>();
	let adjustedX = $state(0);
	let adjustedY = $state(0);
	let submenuOpen = $state<string | null>(null);
	let submenuPinned = $state(false);
	let hoverTimer: ReturnType<typeof setTimeout> | undefined;
	let opensLeft = $state(false);
	let submenuOffsetX = $state(0);
	function positionMenu() {
		if (!open || !containerEl) return;
		const width = containerEl.offsetWidth;
		const left = clampPopoverStart(x, width, window.innerWidth);
		adjustedX = left;
		adjustedY = clampPopoverStart(y, containerEl.offsetHeight, window.innerHeight);
		const side = horizontalPopover({ left, right: left + width }, window.innerWidth, Math.min(180, window.innerWidth - 16), false, -2);
		opensLeft = side.leftward;
		submenuOffsetX = side.left - left;
	}

	$effect(() => {
		if (!open) return;
		positionMenu();
	});

	$effect(() => {
		// Reset when the menu is reopened, including for a different instance.
		if (open && items) closeSubmenu();
		return clearHoverTimer;
	});

	function clearHoverTimer() {
		clearTimeout(hoverTimer);
		hoverTimer = undefined;
	}

	function closeSubmenu() {
		clearHoverTimer();
		submenuOpen = null;
		submenuPinned = false;
	}

	function enterRow(item: ContextMenuItem, event: PointerEvent) {
		if (event.pointerType === "touch" || submenuPinned) return;
		clearHoverTimer();
		if (item.children && !item.split && !item.disabled) submenuOpen = item.label;
		else if (submenuOpen !== item.label) submenuOpen = null;
	}

	function previewSubmenu(label: string, event: PointerEvent) {
		if (event.pointerType === "touch" || submenuPinned) return;
		clearHoverTimer();
		if (submenuOpen === label) return;
		hoverTimer = setTimeout(() => {
			hoverTimer = undefined;
			if (open && !submenuPinned) submenuOpen = label;
		}, 220);
	}

	function leaveRow(label: string, event: PointerEvent) {
		if (event.pointerType === "touch" || submenuPinned) return;
		clearHoverTimer();
		if (submenuOpen !== label) return;
		hoverTimer = setTimeout(closeSubmenu, 180);
	}

	function togglePinnedSubmenu(label: string) {
		clearHoverTimer();
		if (submenuPinned && submenuOpen === label) {
			closeSubmenu();
		} else {
			submenuOpen = label;
			submenuPinned = true;
		}
	}

	function positionSubmenu(node: HTMLElement) {
		function update() {
			const row = node.parentElement;
			if (!row) return;
			const rowY = adjustedY + row.offsetTop;
			node.style.top = `${clampPopoverStart(rowY, node.offsetHeight, window.innerHeight) - rowY}px`;
		}
		$effect(() => {
			update();
			const observer = new ResizeObserver(update);
			observer.observe(node);
			window.addEventListener("resize", update);
			return () => { observer.disconnect(); window.removeEventListener("resize", update); };
		});
	}

	function portal(el: HTMLElement) {
		document.body.appendChild(el);
		return {
			destroy() {
				el.remove();
			},
		};
	}

	function handleClickOutside(event: MouseEvent) {
		if (event.defaultPrevented) return;
		if (containerEl && !containerEl.contains(event.target as Node)) {
			open = false;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key !== "Escape" || event.defaultPrevented || !open || hasOpenModal()) return;
		event.preventDefault();
		if (event.repeat) return;
		if (submenuOpen !== null) closeSubmenu();
		else { closeSubmenu(); open = false; }
	}

	onMount(() => {
		window.addEventListener("click", handleClickOutside);
		window.addEventListener("contextmenu", handleClickOutside);
		window.addEventListener("keydown", handleKeydown);
		return () => {
			window.removeEventListener("click", handleClickOutside);
			window.removeEventListener("contextmenu", handleClickOutside);
			window.removeEventListener("keydown", handleKeydown);
		};
	});

	const flyDuration = $derived(animDuration(120));
</script>

<svelte:window onresize={positionMenu} />

{#if open}
	<div use:portal>
		<div
			bind:this={containerEl}
			class="ctx-menu"
			class:opens-left={opensLeft}
			style="left: {adjustedX}px; top: {adjustedY}px;"
			transition:fly={{ y: -4, duration: flyDuration }}
			role="menu"
		>
			{#each items as item, index (item.label + index)}
				{#if item.separator}
					<div class="ctx-separator"></div>
				{:else}
					<div class="ctx-submenu-wrap" role="none" onpointerenter={(event) => enterRow(item, event)} onpointerleave={(event) => leaveRow(item.label, event)}>
						{#if item.split && item.children}
							<div class="ctx-split-row" class:prominent={item.prominent}>
								<button type="button" class="ctx-item ctx-item--main" class:danger={item.variant === "danger"} class:accent={item.variant === "accent"} class:prominent={item.prominent} disabled={item.disabled} role="menuitem" onclick={() => { item.action?.(); open = false; }}>
									{#if item.iconName}<span class="ctx-icon"><Icon name={item.iconName} size={item.prominent ? 16 : 14} /></span>{/if}
									<span class="ctx-label">{item.label}</span>
									{#if item.badge !== undefined}<span class="ctx-badge">{item.badge}</span>{/if}
								</button>
								{#if item.prominent ? item.children.some((child) => !child.disabled) : true}
								<button type="button" class="ctx-item ctx-split-arrow" class:pinned={submenuPinned && submenuOpen === item.label} class:danger={item.variant === "danger"} class:accent={item.variant === "accent"} aria-label={item.splitLabel ?? t("instanceView.runningWindows", { count: item.children.filter((child) => !child.disabled).length })} title={item.splitLabel ?? t("instanceView.runningWindows", { count: item.children.filter((child) => !child.disabled).length })} aria-haspopup="menu" aria-expanded={submenuOpen === item.label}
									onpointerenter={(event) => previewSubmenu(item.label, event)}
									onpointerleave={() => { if (submenuOpen !== item.label) clearHoverTimer(); }}
									onclick={(event) => { event.stopPropagation(); togglePinnedSubmenu(item.label); }}>
									<span class="ctx-chevron">{opensLeft ? "‹" : "›"}</span>
								</button>
								{/if}
							</div>
						{:else}
							<button
								type="button"
								class="ctx-item"
								class:danger={item.variant === "danger"}
								class:accent={item.variant === "accent"}
								class:prominent={item.prominent}
								class:disabled={item.disabled}
								disabled={item.disabled}
								role="menuitem"
								aria-haspopup={item.children ? "menu" : undefined}
								aria-expanded={item.children ? submenuOpen === item.label : undefined}
								onclick={() => {
									if (item.children) { togglePinnedSubmenu(item.label); return; }
									item.action?.();
									open = false;
								}}
							>
								{#if item.iconName}<span class="ctx-icon"><Icon name={item.iconName} size={14} /></span>{/if}
								<span class="ctx-label">{item.label}</span>
								{#if item.children}<span class="ctx-chevron">{opensLeft ? "‹" : "›"}</span>{/if}
							</button>
						{/if}
						{#if item.children && submenuOpen === item.label}
							<div class="ctx-menu ctx-submenu" use:positionSubmenu style:left={`${submenuOffsetX}px`} transition:fly={{ x: opensLeft ? 6 : -6, duration: flyDuration }} role="menu">
								{#each item.children as child, childIndex (child.label + childIndex)}
									<button type="button" class="ctx-item" class:disabled={child.disabled} disabled={child.disabled} role="menuitem" onclick={() => { child.action?.(); open = false; }}>{#if child.iconName}<span class="ctx-icon"><Icon name={child.iconName} size={14} /></span>{/if}<span class="ctx-label">{child.label}</span></button>
								{/each}
							</div>
						{/if}
					</div>
				{/if}
			{/each}
		</div>
	</div>
{/if}

<style>
	.ctx-menu {
		position: fixed;
		z-index: 9999;
		min-width: 140px;
		max-width: 210px;
		background: var(--bg-surface, #1e1e1e);
		border-radius: var(--border-radius-sm, 6px);
		padding: 0;
		box-shadow: var(--shadow-floating);
		backdrop-filter: blur(var(--backdrop-blur-dropdown, 4px));
	}

	.ctx-item {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 5px 14px;
		background: transparent;
		border: none;
		color: var(--text-primary);
		font-size: 0.82rem;
		font-weight: 500;
		text-align: left;
		cursor: pointer;
		border-radius: 4px;
		transition:
			background 0.12s ease,
			color 0.12s ease;
	}

	.ctx-submenu-wrap { position: relative; }
	.ctx-split-row { display: flex; align-items: stretch; gap: 2px; }
	.ctx-split-row .ctx-item--main { flex: 1; min-width: 0; }
	.ctx-split-arrow { position: relative; width: 22px; justify-content: center; padding: 0; flex: 0 0 22px; }
	.ctx-split-arrow.pinned .ctx-chevron { opacity: 1; }
	.ctx-item.accent { background: var(--accent); color: var(--accent-text, #fff); }
	.ctx-item.accent:hover:not(:disabled) { background: var(--accent-hover, var(--accent)); filter: brightness(1.08); }
	.ctx-item.prominent { min-height: 28px; padding-top: 5px; padding-bottom: 5px; font-size: 0.92rem; font-weight: 700; }
	.ctx-split-row.prominent { margin-bottom: 3px; }
	/* «Играть»: стрелка окон встроена в саму кнопку, без отдельной плашки. */
	.ctx-split-row.prominent { gap: 0; position: relative; background: var(--accent); border-radius: 4px; overflow: hidden; }
	.ctx-split-row.prominent:hover { filter: brightness(1.08); }
	.ctx-split-row.prominent .ctx-item.accent,
	.ctx-split-row.prominent .ctx-item.accent:hover:not(:disabled) { background: transparent; filter: none; }
	.ctx-split-row.prominent .ctx-split-arrow { border-radius: 0; }
	.ctx-split-arrow::before { content: ""; position: absolute; left: 0; top: 6px; bottom: 6px; width: 1px; background: var(--border); transition: opacity 0.12s ease; }
	.ctx-split-row.prominent .ctx-split-arrow::before { background: color-mix(in srgb, white 40%, transparent); }
	.ctx-split-row.prominent .ctx-split-arrow .ctx-chevron { font-weight: 600; opacity: 1; }
	.ctx-split-row.prominent .ctx-item.ctx-split-arrow.accent:hover:not(:disabled),
	.ctx-split-row.prominent .ctx-item.ctx-split-arrow.accent[aria-expanded="true"] { background: color-mix(in srgb, white 18%, transparent); }
	.ctx-split-arrow:hover::before,
	.ctx-split-arrow[aria-expanded="true"]::before { opacity: 0; }
	.ctx-badge { margin-left: auto; min-width: 18px; height: 18px; padding: 0 5px; border-radius: 9px; background: color-mix(in srgb, white 24%, transparent); font-size: 0.7rem; font-weight: 800; line-height: 18px; text-align: center; }
	/* Подменю уходит влево: кнопка со стрелкой тоже переезжает на левую сторону. */
	.opens-left .ctx-split-row { flex-direction: row-reverse; }
	.opens-left .ctx-split-arrow::before { left: auto; right: 0; }
	.opens-left .ctx-item > .ctx-chevron { order: -1; margin: 0; }
	.ctx-submenu { position: absolute; width: min(180px, calc(100vw - 16px)); min-width: 0; max-height: calc(100vh - 16px); overflow-y: auto; overscroll-behavior: contain; }
	.ctx-chevron { margin-left: auto; font-size: 1rem; opacity: .7; }
	.ctx-split-arrow > .ctx-chevron { margin: 0; font-size: 1.5rem; line-height: 1; padding-bottom: 3px; }
	.ctx-icon {
		display: flex;
		align-items: center;
		flex-shrink: 0;
	}

	.ctx-label {
		min-width: 0;
	}

	.ctx-item:hover:not(:disabled) {
		background: var(--surface-active);
	}

	.ctx-item.danger {
		color: var(--color-error, #ef4444);
	}

	.ctx-item.danger:hover:not(:disabled) {
		background: rgba(var(--color-error-rgb), 0.12);
	}

	.ctx-item:disabled,
	.ctx-item.disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.ctx-separator {
		height: 1px;
		background: var(--border);
		margin: 4px 7px;
		opacity: 0.55;
	}
</style>
