<script lang="ts">
	import type { InstanceDto } from "$lib/types/types";
	import { t } from "$lib/i18n";
	import Icon from "$lib/icons/Icon.svelte";
	import Trash from "$lib/icons/Trash.svelte";
	import { getDisplayIconSrc } from "$lib/icons/logos";
	import { appearance } from "$lib/state/appearanceState.svelte";

	let {
		instance,
		selected,
		onselect,
		onedit,
		ondelete,
		onpointerdown,
		dragging = false,
		rightDock = false,
	}: {
		instance: InstanceDto;
		selected: boolean;
		onselect: () => void;
		onedit: () => void;
		ondelete: () => void;
		onpointerdown?: (event: PointerEvent) => void;
		/** Esta fila es la que se está arrastrando: queda como hueco translúcido. */
		dragging?: boolean;
		rightDock?: boolean;
	} = $props();

	/**
	 * Длинный текст прокручивается у выбранной карточки, при наведении или фокусе.
	 */
	type MarqueeOptions = { text: string; active: boolean; rightDock: boolean; quickActions: boolean };

	function marquee(
		node: HTMLElement,
		params: MarqueeOptions,
	) {
		const inner = node.querySelector<HTMLElement>(".marquee-inner");
		const row = node.closest<HTMLElement>(".instance-item");
		if (!inner || !row) return;
		const textRange = document.createRange();
		const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
		let animation: Animation | undefined;
		let animatedText = "";
		let offset = 0;
		let startInset = 0;
		let destroyed = false;
		const stop = () => {
			animation?.cancel();
			animation = undefined;
			node.classList.remove("scrolling");
			node.style.removeProperty("--marquee-start-inset");
		};
		const measure = () => {
			if (destroyed) return;
			const { active, rightDock, text, quickActions } = params;
			if ((!active && !row.matches(":hover, :focus-within")) || reducedMotion.matches) {
				stop();
				return;
			}
			const box = node.getBoundingClientRect();
			const actions = quickActions
				? row.querySelector<HTMLElement>(".instance-actions")
				: null;
			const actionBox = actions?.getBoundingClientRect();
			const textBox = node.parentElement?.getBoundingClientRect();
			let insetStart = 0;
			let insetEnd = 0;
			if (actionBox?.width && textBox) {
				// Measure to the fully visible edge of the fade, not behind the buttons.
				const fade = parseFloat(getComputedStyle(node).getPropertyValue("--item-fade-start")) || 0;
				if (rightDock) {
					insetStart = Math.ceil(Math.max(0, actionBox.right + 6 - box.left, textBox.left + fade - box.left));
				} else {
					insetEnd = Math.max(0, box.right - actionBox.left + 6, box.right - textBox.right + fade);
				}
			}
			const free = box.width - insetStart - insetEnd;
			textRange.selectNodeContents(inner);
			// Ignore subpixel rounding from sampling a moving glyph rectangle.
			const dist = Math.ceil(Math.round((textRange.getBoundingClientRect().width - free) * 64) / 64);
			if (free <= 0 || dist <= 2) {
				stop();
				return;
			}

			const nextOffset = -dist;
			if (animation && offset === nextOffset && startInset === insetStart && animatedText === text) return;
			// Keep the current phase if the same title is remeasured during a resize.
			const progress = rightDock && animatedText === text ? animation?.effect?.getComputedTiming().progress : null;
			animation?.cancel();
			offset = nextOffset;
			startInset = insetStart;
			animatedText = text;
			node.style.setProperty("--marquee-start-inset", `${startInset}px`);
			node.classList.add("scrolling");

			const previousCycle = Math.max(4000, (dist / 28 + 3) * 1000);
			const startPause = previousCycle * 0.18;
			const travel = previousCycle * 0.31 * 1.15;
			const endPause = previousCycle * 0.07;
			const duration = startPause + 2 * travel + endPause;
			const start = "translateX(0)";
			const end = `translateX(${offset}px)`;
			animation = inner.animate([
				{ transform: start, offset: 0 },
				{ transform: start, offset: startPause / duration, easing: "ease-in-out" },
				{ transform: end, offset: (startPause + travel) / duration },
				{ transform: end, offset: (startPause + travel + endPause) / duration, easing: "ease-in-out" },
				{ transform: start, offset: 1 },
			], { duration, iterations: Infinity });
			if (progress != null) animation.currentTime = progress * duration;
		};
		const observer = new ResizeObserver(measure);
		observer.observe(node);
		const refreshInteraction = () => queueMicrotask(measure);
		const interactionEvents = ["pointerenter", "pointerleave", "focusin", "focusout"];
		for (const event of interactionEvents) row.addEventListener(event, refreshInteraction);
		reducedMotion.addEventListener("change", measure);
		measure();
		void document.fonts?.ready.then(measure);
		return {
			update(next: MarqueeOptions) {
				params = next;
				measure();
			},
			destroy() {
				destroyed = true;
				observer.disconnect();
				for (const event of interactionEvents) row.removeEventListener(event, refreshInteraction);
				reducedMotion.removeEventListener("change", measure);
				stop();
			},
		};
	}

	let isRunning = $derived(instance.status === "started");
	let isPinned = $derived(instance.pinned);
	let iconUrl = $derived(getDisplayIconSrc(instance.icon));
	let formattedLoader = $derived(
		instance.loader.charAt(0).toUpperCase() + instance.loader.slice(1),
	);
	let subtitle = $derived(
		[instance.version, formattedLoader].filter(Boolean).join(" • "),
	);
</script>

<div
	class="instance-item"
	class:active={selected}
	class:right-dock={rightDock}
	class:dragging
	class:has-quick-actions={!appearance.quickActionsDisabled}
	{onpointerdown}
	ondragstart={(e) => e.preventDefault()}
	data-instance-uuid={instance.uuid}
	onclick={onselect}
	onkeydown={(e) => {
		if (e.key === "Enter" || e.key === " ") onselect();
	}}
	role="button"
	tabindex="0"
	title={instance.name}
>
	<div class="instance-icon">
		{#if instance.icon}
			<img
				src={iconUrl}
				alt={instance.name}
				loading="lazy"
				decoding="async"
				width="24"
				height="24"
				draggable="false"
				ondragstart={(e) => e.preventDefault()}
			/>
		{:else}
			<Icon name="brand:cubic" size={24} />
		{/if}
		{#if isRunning}
			<span
				class="status-dot"
				aria-label={t("instanceView.status.started")}
			></span>
		{/if}
	</div>

	<div class="instance-text">
		<span
			class="instance-name"
			use:marquee={{ text: instance.name, active: selected, rightDock, quickActions: !appearance.quickActionsDisabled }}
			><span class="marquee-viewport"><span class="marquee-inner">{instance.name}</span></span></span
		>
		<span
			class="instance-subtitle"
			use:marquee={{ text: subtitle, active: selected, rightDock, quickActions: !appearance.quickActionsDisabled }}
			><span class="marquee-viewport"><span class="marquee-inner">{subtitle}</span></span></span
		>
	</div>

	{#if !appearance.quickActionsDisabled}
	<div class="instance-actions" data-no-drag>
		<button
			type="button"
			class="action-btn"
			onclick={(e) => {
				e.stopPropagation();
				onedit();
			}}
			title={t("sidebar.rename")}
		>
			<Icon name="nav:edit" size={14} />
		</button>
		<button
			type="button"
			class="action-btn delete"
			onclick={(e) => {
				e.stopPropagation();
				ondelete();
			}}
			title={t("sidebar.delete")}
		>
			<Trash width="14" height="14" />
		</button>
	</div>
	{/if}

	{#if isPinned}
		<span class="pin-badge" aria-hidden="true"
			><Icon name="ui:pin" size={9} /></span
		>
	{/if}
</div>

<style>
	.instance-item.dragging {
		opacity: 0.35;
		pointer-events: none;
	}

	.instance-item {
		/* Color opaco por estado; alimenta el fondo y el degradado de las
		   acciones sin provocar reflujo al hacer hover. */
		--item-bg: var(--bg-sidebar);
		--item-fade-angle: 90deg;
		--item-fade-start: 104px;

		position: relative;
		display: flex;
		align-items: center;
		border-radius: var(--launcher-control-radius);
		cursor: pointer;
		transition:
			background 0.15s ease,
			border-color 0.15s ease,
			box-shadow 0.15s ease;
		border: 1px solid transparent;
		background: transparent;
		color: var(--text-primary);
		width: 100%;
		text-align: left;
		outline: none;
		height: calc(
			var(--virtual-row-height, var(--sidebar-row-height, 52px)) -
				var(--sidebar-row-gap, 8px)
		);
		min-height: 0;
		/* Высота карточки уменьшена на gap; нижний отступ заполняет остаток строки. */
		margin-bottom: var(--sidebar-row-gap, 8px);
		box-sizing: border-box;
		overflow: hidden;
		contain: layout style;
	}

	.instance-item.right-dock {
		--item-fade-angle: 270deg;
		flex-direction: row-reverse;
		text-align: right;
	}

	.instance-item:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: -2px;
	}

	.instance-item.active {
		--item-bg: var(--launcher-selected);
		background: var(--item-bg);
		border-color: var(--selected-border);
		box-shadow: inset var(--instance-selection-offset, 3px) 0 var(--accent);
	}

	.instance-item:not(.dragging):is(:hover, :focus-within) {
		--item-bg: var(--launcher-item-hover);
		background: var(--item-bg);
		border-color: var(--launcher-hover-border);
	}

	.instance-icon {
		position: relative;
		flex-shrink: 0;
		width: 24px;
		height: 24px;
		margin-left: var(--instance-icon-inset, 12px);
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--border-radius-sm);
		color: var(--text-primary);
	}

	.instance-icon img {
		-webkit-user-drag: none;
		user-select: none;
		display: block;
		width: 100%;
		height: 100%;
		border-radius: inherit;
		object-fit: contain;
	}

	.right-dock .instance-icon { margin: 0 var(--instance-icon-inset, 12px) 0 0; }

	.pin-badge {
		/* Esquina superior derecha de la tarjeta, fuera del flujo para que no
		   quede bajo los botones de editar/eliminar. */
		position: absolute;
		top: 2px;
		right: 2px;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;
		background: var(--bg-sidebar);
		border: 1px solid var(--border);
		border-radius: 50%;
		color: var(--color-warning);
		pointer-events: none;
		box-shadow: var(--shadow-sm);
	}

	.instance-text {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 1px;
		min-width: 0;
		padding: 0 10px 0 12px;
	}

	/* Когда быстрые кнопки отключены, текст занимает всю ширину плитки. */
	.instance-item:not(.has-quick-actions) .instance-text {
		padding-right: 2px;
	}

	.right-dock .instance-text { padding: 0 12px 0 10px; }
	.right-dock:not(.has-quick-actions) .instance-text { padding: 0 12px 0 2px; }

	/* Фейд начинается заметно раньше кнопок, а не прямо под ними. */
	.instance-item.has-quick-actions:hover .instance-text,
	.instance-item.has-quick-actions.active .instance-text,
	.instance-item.has-quick-actions:focus-within .instance-text {
		-webkit-mask-image: linear-gradient(
			var(--item-fade-angle),
			#000 calc(100% - var(--item-fade-start)),
			transparent calc(100% - 62px)
		);
		mask-image: linear-gradient(
			var(--item-fade-angle),
			#000 calc(100% - var(--item-fade-start)),
			transparent calc(100% - 62px)
		);
	}

	.instance-name {
		display: block;
		font-weight: 600;
		font-size: var(--sidebar-item-font-size, var(--font-size-control));
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	/* Обычный inline сохраняет троеточие, когда прокрутка выключена. */
	.marquee-viewport {
		display: contents;
	}

	/* The dock positions the viewport; the text always animates from its origin. */
	:is(.instance-name, .instance-subtitle):global(.scrolling) {
		text-overflow: clip;
	}

	:global(.scrolling) .marquee-viewport {
		display: flex;
		justify-content: flex-start;
		margin-left: var(--marquee-start-inset, 0px);
		overflow: clip;
	}

	:global(.scrolling) .marquee-inner {
		flex: none;
		width: max-content;
	}

	.instance-subtitle {
		display: block;
		font-size: 0.7rem;
		color: var(--text-tertiary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		transition: color 0.15s ease;
	}

	.instance-item:hover .instance-subtitle,
	.instance-item.active .instance-subtitle {
		color: var(--text-secondary);
	}

	.instance-actions {
		position: absolute;
		top: 50%;
		right: 4px;
		transform: translateY(-50%);
		display: flex;
		align-items: center;
		gap: 4px;
		padding-left: 16px;
		background: linear-gradient(var(--item-fade-angle), transparent, var(--item-bg) 45%);
		opacity: 0;
		pointer-events: none;
		transition: opacity 0.15s ease;
	}

	.right-dock .instance-actions {
		right: auto;
		left: 4px;
		flex-direction: row-reverse;
		padding: 0 16px 0 0;
	}

	.right-dock .pin-badge { right: auto; left: 2px; }
	.right-dock .status-dot { right: auto; left: -1px; }

	.instance-item:hover .instance-actions,
	.instance-item.active .instance-actions,
	.instance-item:focus-within .instance-actions {
		opacity: 1;
		pointer-events: auto;
	}

	.action-btn {
		width: 22px;
		height: 22px;
		border-radius: var(--border-radius-sm);
		border: none;
		background: transparent;
		color: var(--text-tertiary);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		transition:
			background 0.15s ease,
			color 0.15s ease,
			transform 0.12s ease;
	}

	.action-btn:hover {
		background: var(--launcher-hover-overlay);
		color: var(--text-primary);
	}

	.action-btn:active {
		transform: scale(0.94);
	}

	.action-btn.delete:hover {
		background: rgba(var(--color-error-rgb), 0.12);
		color: var(--color-error);
	}

	.status-dot {
		position: absolute;
		bottom: -1px;
		right: -1px;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-status-started);
		border: 1.5px solid var(--bg-sidebar);
		box-shadow: 0 0 4px rgba(var(--color-success-rgb), 0.5);
	}

	@media (max-width: 650px) {
		:global(.sidebar:not(.horizontal) .instance-text),
		:global(.sidebar:not(.horizontal) .instance-actions) {
			display: none;
		}

	}
</style>
