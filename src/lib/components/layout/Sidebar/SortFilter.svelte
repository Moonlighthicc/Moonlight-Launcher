<script lang="ts">
	import { t } from "$lib/i18n";
	import {
		SORT_KEYS,
		chooseSort,
		sidebarLayout,
		type SortKey,
	} from "$lib/state/sidebarLayoutState.svelte";

	let { compact = false }: { compact?: boolean } = $props();

	let open = $state(false);
	let button: HTMLButtonElement | undefined = $state();
	let menu: HTMLDivElement | undefined = $state();
	let pos = $state({ left: 0, top: 0, up: false });

	const MENU_W = 214;
	const MENU_H = 4 * 36 + 16;

	function label(key: SortKey): string {
		switch (key) {
			case "lastPlayed":
				return t("sidebar.sort.lastPlayed");
			case "name":
				return t("sidebar.sort.name");
			case "playtime":
				return t("sidebar.sort.playtime");
			case "created":
				return t("sidebar.sort.created");
		}
	}

	const currentLabel = $derived(
		sidebarLayout.sortMode === "manual"
			? t("sidebar.sort.manual")
			: label(sidebarLayout.sortMode),
	);

	function arrowTitle(dir: "asc" | "desc"): string {
		return dir === "asc"
			? t("sidebar.sort.ascending")
			: t("sidebar.sort.descending");
	}

	/** Nombre: A → Z (asc). Resto: de menor a mayor (asc) = más antiguo / menos tiempo primero. */
	function toggle() {
		if (open) {
			open = false;
			return;
		}
		const rect = button?.getBoundingClientRect();
		if (rect) {
			const up = rect.bottom + MENU_H + 8 > window.innerHeight;
			pos = {
				left: Math.max(
					8,
					Math.min(window.innerWidth - MENU_W - 8, rect.right - MENU_W),
				),
				top: up ? rect.top - 6 : rect.bottom + 6,
				up,
			};
		}
		open = true;
	}

	function pick(key: SortKey) {
		chooseSort(key);
	}

	$effect(() => {
		if (!open) return;
		const onDown = (event: PointerEvent) => {
			const target = event.target as Node;
			if (menu?.contains(target) || button?.contains(target)) return;
			open = false;
		};
		const onKey = (event: KeyboardEvent) => {
			if (event.key === "Escape") open = false;
		};
		window.addEventListener("pointerdown", onDown, true);
		window.addEventListener("keydown", onKey);
		const onBlur = () => (open = false);
		window.addEventListener("blur", onBlur);
		return () => {
			window.removeEventListener("pointerdown", onDown, true);
			window.removeEventListener("keydown", onKey);
			window.removeEventListener("blur", onBlur);
		};
	});
</script>

{#snippet arrow(dir: "asc" | "desc")}
	<svg
		class="arrow"
		class:asc={dir === "asc"}
		width="12"
		height="12"
		viewBox="0 0 12 12"
		fill="none"
		stroke="currentColor"
		stroke-width="1.8"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d="M6 2v8M2.5 6.5 6 10l3.5-3.5" />
	</svg>
{/snippet}

<button
	bind:this={button}
	type="button"
	class="sort-btn"
	class:open
	class:compact
	data-no-panel-drag
	aria-haspopup="menu"
	aria-expanded={open}
	title={t("sidebar.sort.title")}
	onclick={toggle}
>
	<span class="sort-current">{currentLabel}</span>
	{#if sidebarLayout.sortMode !== "manual"}
		{@render arrow(sidebarLayout.sortDir)}
	{:else}
		<svg
			class="arrow manual"
			width="12"
			height="12"
			viewBox="0 0 12 12"
			fill="none"
			stroke="currentColor"
			stroke-width="1.8"
			stroke-linecap="round"
			aria-hidden="true"
		>
			<path d="M2 4h8M2 8h8" />
		</svg>
	{/if}
</button>

{#if open}
	<div
		bind:this={menu}
		class="sort-menu"
		role="menu"
		aria-label={t("sidebar.sort.title")}
		style:left="{pos.left}px"
		style:top="{pos.top}px"
		style:width="{MENU_W}px"
		style:transform={pos.up ? "translateY(-100%)" : undefined}
	>
		{#each SORT_KEYS as key (key)}
			{@const active = sidebarLayout.sortMode === key}
			<button
				type="button"
				role="menuitemradio"
				aria-checked={active}
				class="sort-item"
				class:active
				onclick={() => pick(key)}
			>
				<span class="sort-name">{label(key)}</span>
				<span
					class="sort-dir"
					title={active ? arrowTitle(sidebarLayout.sortDir) : ""}
				>
					{@render arrow(active ? sidebarLayout.sortDir : "desc")}
				</span>
			</button>
		{/each}
	</div>
{/if}

<style>
	.sort-btn {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		max-width: 100%;
		padding: 2px 6px;
		margin: -2px -6px -2px 0;
		border: 1px solid transparent;
		border-radius: var(--border-radius-sm);
		background: transparent;
		color: var(--text-secondary);
		font: inherit;
		font-size: 0.62rem;
		font-weight: 600;
		letter-spacing: 0.4px;
		text-transform: none;
		cursor: pointer;
		transition:
			background 0.15s ease,
			color 0.15s ease,
			border-color 0.15s ease;
	}

	.sort-btn:hover,
	.sort-btn.open,
	.sort-btn:focus-visible {
		background: var(--surface-hover);
		color: var(--text-primary);
		border-color: var(--border);
		outline: none;
	}

	.sort-current {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.arrow {
		flex-shrink: 0;
		transition: transform 0.18s ease;
	}

	/* La flecha apunta abajo para "mayor → menor" y arriba para "menor → mayor". */
	.arrow.asc {
		transform: rotate(180deg);
	}

	.sort-menu {
		position: fixed;
		z-index: 3000;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 6px;
		border: 1px solid var(--border);
		border-radius: var(--border-radius);
		background: var(--bg-card, var(--bg-sidebar));
		box-shadow: var(--shadow-lg, 0 10px 30px rgba(0, 0, 0, 0.4));
		animation: sort-pop 0.12s ease-out;
	}

	@keyframes sort-pop {
		from {
			opacity: 0;
			margin-top: 4px;
		}
		to {
			opacity: 1;
			margin-top: 0;
		}
	}

	.sort-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		height: 32px;
		padding: 0 10px;
		border: none;
		border-radius: var(--border-radius-sm);
		background: transparent;
		color: var(--text-secondary);
		font: inherit;
		font-size: 0.8rem;
		font-weight: 500;
		text-align: left;
		cursor: pointer;
	}

	.sort-item:hover {
		background: var(--surface-hover);
		color: var(--text-primary);
	}

	.sort-item.active {
		color: var(--text-primary);
		font-weight: 700;
		background: color-mix(in srgb, var(--accent) 16%, transparent);
	}

	.sort-dir {
		display: inline-flex;
		opacity: 0;
		transition: opacity 0.12s ease;
	}

	.sort-item.active .sort-dir {
		color: var(--accent);
		opacity: 1;
	}

	.sort-item:hover .sort-dir {
		opacity: 0.6;
	}

	.sort-item.active:hover .sort-dir {
		opacity: 1;
	}
</style>
