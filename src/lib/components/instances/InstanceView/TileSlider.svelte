<script lang="ts">
	import { t } from "$lib/i18n";
	import { fly } from "svelte/transition";
	import { cubicInOut } from "svelte/easing";
	import { onAppEvent } from "$lib/api/launcherService";
	import {
		requestTileContent,
		tileItems,
		type TileItem,
	} from "$lib/state/tileContentState.svelte";
	import type { TileId } from "$lib/state/instanceNavState.svelte";
	import { launcherStore } from "$lib/state/state.svelte";
	import { appearance } from "$lib/state/appearanceState.svelte";
	import { tileSliderInsets } from "$lib/utils/tileSliderInsets";

	let {
		uuid,
		name,
		id,
		width,
		height,
		maxRows,
		forceRows = false,
		maxIcon,
		minIcon = 18,
		side,
		fadePx = 36,
		fadeBoth = false,
		inset = 0,
		galleryCaptionWidth,
		edgeInset = 0,
		scale = 1,
		stretch = false,
		plain = false,
		verticalAlign = "center",
	}: {
		uuid: string;
		name: string;
		id: TileId;
		/** Zona disponible para la cinta, en px. */
		width: number;
		height: number;
		/** Máximo de filas permitido por el nivel de tamaño del mosaico. */
		maxRows: number;
		/** En modo de número personalizado, usa las filas solicitadas siempre que quepan. */
		forceRows?: boolean;
		/** Tamaño máximo / mínimo del icono: la cinta se estira entre ambos según el contenido. */
		maxIcon: number;
		minIcon?: number;
		/** Lado donde está la cinta; el desvanecido queda junto al texto (lado contrario). */
		side: "left" | "right" | "center";
		fadePx?: number;
		/** Desvanece también el borde exterior (menú compacto, donde sigue el chevron). */
		fadeBoth?: boolean;
		/** Margen del borde exterior de la galería de capturas, en px. */
		inset?: number;
		/** Actual width of the glass caption, if this gallery covers the whole tile. */
		galleryCaptionWidth?: number;
		/** Inset for moving icon ribbons, in physical CSS pixels. */
		edgeInset?: number;
		/** Escala visual de la cinta: la disposición (filas, tamaños) se calcula a escala 1 y luego se escala todo junto. */
		scale?: number;
		/** Растянуть по высоте до верхнего и нижнего края (режим «Растянуть»). */
		stretch?: boolean;
		/** Стили «Стильный»/«Стекло»: поле на всю плитку, без затухания у края. */
		plain?: boolean;
		/** Вертикальное положение компактной группы иконок внутри плитки. */
		verticalAlign?: "start" | "center" | "end";
	} = $props();

	/** Ширина затухания (постоянная; граница на неё не влияет). */
	const fade = $derived(plain ? 0 : fadePx);

	const GAP = $derived(Math.max(0, Math.min(32, launcherStore.settings.tile_slider_gap ?? 8)));
	const GRID_GAP = $derived(Math.max(0, Math.min(24, launcherStore.settings.tile_slider_grid_gap ?? 8)));
	const ROW_GAP = $derived(Math.max(0, Math.min(24, launcherStore.settings.tile_slider_grid_gap ?? 8)));
	const hideText = $derived(appearance.sliderMotion !== "off" && (launcherStore.settings.tile_slider_hide_text ?? false));
	const BASE_SPEED = 22; // px/s, lento a propósito

	// Galería de capturas: una imagen a la vez, a todo el ancho de la zona.
	const SLIDE_MS = 520;
	/** Los ajustes guardan 80 % como la velocidad original. */
	const speedFactor = $derived(
		Math.max(0.1, launcherStore.settings.tile_slider_speed / 80),
	);
	/** Milisegundos entre capturas; 0 = cambio automático desactivado. */
	const holdMs = $derived(
		Math.max(0, launcherStore.settings.screenshot_interval) * 1000,
	);
	const reducedMotion =
		typeof matchMedia === "function" &&
		matchMedia("(prefers-reduced-motion: reduce)").matches;

	const packEvents: Partial<Record<TileId, string>> = {
		market: "ModsEnriched",
		resources: "ResourcepacksEnriched",
		shaderpacks: "ShaderpacksEnriched",
	};

	$effect(() => {
		const instance = uuid;
		const instanceName = name;
		const tile = id;
		const event = packEvents[tile];
		const unsubscribe = event
			? onAppEvent(event, (payload) => {
					if ((payload.data as { id?: string } | undefined)?.id === instance) {
						void requestTileContent(instance, instanceName, tile, true);
					}
				})
			: undefined;
		void requestTileContent(instance, instanceName, tile);
		return unsubscribe;
	});

	const items = $derived(tileItems(uuid, id));
	const isGallery = $derived(id === "screenshots");

	function clamp(value: number, min: number, max: number) {
		return Math.min(max, Math.max(min, value));
	}

	function hash(text: string): number {
		let h = 2166136261;
		for (let i = 0; i < text.length; i++) {
			h ^= text.charCodeAt(i);
			h = Math.imul(h, 16777619);
		}
		return h >>> 0;
	}

	function hue(item: TileItem): number {
		return hash(item.label ?? item.key) % 360;
	}

	function initial(item: TileItem): string {
		return (item.label ?? "?").trim().charAt(0).toUpperCase() || "?";
	}

	interface Row {
		index: number;
		items: TileItem[];
		spacer: number;
		duration: number;
		delay: number;
		reverse: boolean;
	}

	const layoutW = $derived(width / scale);
	const layoutH = $derived(height / scale);
	const motionInset = $derived(tileSliderInsets(edgeInset, width, scale, side));

	const plan = $derived.by(() => {
		if (isGallery || appearance.sliderMotion === "off") return null;
		// La disposición se calcula en el espacio sin escalar: así el número de filas no cambia.
		const width = layoutW;
		const height = layoutH;
		const floor = forceRows ? CUSTOM_MIN_ICON : Math.min(minIcon, Math.max(8, Math.floor(height)));
		if (!items.length || width < 24 || height < floor) return null;
		const fits = Math.max(
			1,
			Math.floor((height + ROW_GAP) / (floor + ROW_GAP)),
		);
		const sizeFor = (rows: number) =>
			clamp(
				Math.floor((height - (rows - 1) * ROW_GAP) / rows),
				floor,
				maxIcon,
			);
		const fontFor = (size: number) => clamp(size * 0.4, 10, 20);
		const widthOf = (item: TileItem, size: number, labelFont: number) =>
			!hideText && item.label !== null
				? size + 6 + Math.min(item.label.length, 22) * labelFont * 0.56 + 12
				: size;
		const split = (rows: number) => {
			const out: TileItem[][] = [];
			for (let i = 0; i < rows; i++)
				out.push(items.filter((_, k) => k % rows === i));
			return out;
		};
		const rowLength = (
			list: TileItem[],
			size: number,
			labelFont: number,
		) =>
			list.reduce(
				(sum, item) => sum + widthOf(item, size, labelFont) + GAP,
				0,
			);

		// En modo personalizado se respeta el número elegido, limitado por el
		// contenido y el espacio vertical real. Si no caben todas las filas,
		// `fits` reduce automáticamente su cantidad.
		let rows = forceRows ? Math.min(maxRows, items.length, fits) : 1;
		let scrolls = false;
		if (forceRows) {
			const size = sizeFor(rows);
			const lf = fontFor(size);
			const shortest = Math.min(
				...split(rows).map((list) => rowLength(list, size, lf)),
			);
			scrolls = shortest >= width;
		} else {
			// En modo automático solo se añaden filas cuando cada una puede desplazarse.
			for (let r = Math.min(maxRows, items.length, fits); r >= 1; r--) {
				const size = sizeFor(r);
				const lf = fontFor(size);
				const shortest = Math.min(
					...split(r).map((list) => rowLength(list, size, lf)),
				);
				if (shortest >= width) {
					rows = r;
					scrolls = true;
					break;
				}
			}
		}
		// Poco contenido: la cinta no se mueve y los iconos ocupan el máximo posible.
		const size = sizeFor(rows);
		const labelFont = fontFor(size);

		const built: Row[] = [];
		split(rows).forEach((rowItems, i) => {
			const cycle = rowLength(rowItems, size, labelFont);
			const widest = Math.max(
				...rowItems.map((item) => widthOf(item, size, labelFont)),
			);
			// El mismo elemento no vuelve a entrar hasta que el anterior salió del todo.
			const spacer = scrolls
				? Math.max(0, width + widest + GAP - cycle)
				: 0;
			const length = cycle + spacer;
			const seed = hash(`${id}:${uuid}:${i}`);
			const speed = BASE_SPEED * (0.85 + (seed % 4) * 0.1);
			const duration = Math.max(14, length / speed);
			built.push({
				index: i,
				items: rowItems,
				spacer,
				duration,
				delay: -((seed % 1000) / 1000) * duration,
				reverse: i % 2 === 1,
			});
		});
		return {
			rows: built,
			size,
			labelFont,
			scrolls,
			copies: scrolls ? [0, 1] : [0],
		};
	});

	/** Максимум иконок в статическом режиме («Выключено»). */
	const STATIC_MAX = 500;
	/** Зазор от верхнего и нижнего края при «Растянуть». */
	const STATIC_VPAD = 6;
	/**
	 * В режиме «Своё число» иконки могут уменьшаться до 6px, чтобы уместились все заданные ряды.
	 * Ряды сокращаются только тогда, когда даже при этом размере они физически не помещаются.
	 */
	const CUSTOM_MIN_ICON = 6;
	const iconFloor = $derived(forceRows ? CUSTOM_MIN_ICON : Math.min(minIcon, Math.max(8, Math.floor(layoutH))));
	/** Отступ со стороны затухания (там, где текст). */
	const edgePad = $derived(Math.round(fade * 0.6));
	/** Зазор от края плитки, к которому прижаты иконки. */
	const outerPad = 12;
	const padLeft = $derived(
		plain ? outerPad : side === "right" ? edgePad : outerPad,
	);
	const padRight = $derived(
		plain ? outerPad : side === "right" ? outerPad : side === "center" ? outerPad : edgePad,
	);

	/**
	 * Статическая сетка иконок («Выключено»). Раскладка считается в пространстве
	 * без масштаба и затем масштабируется тем же transform, что и бегущая лента.
	 *  - «По размеру»: самый крупный размер, при котором помещается максимум иконок;
	 *  - «Растянуть»: тот же размер, но ряды распределяются от верхнего до нижнего края;
	 *  - «Своё число»: ровно N рядов (1–10), размер подбирается под высоту.
	 */
	const staticPlan = $derived.by(() => {
		if (isGallery) return null;
		if (!items.length || layoutW < 24 || layoutH < iconFloor) return null;

		const shown = items.slice(0, STATIC_MAX);
		const total = items[0]?.totalCount ?? shown.length;
		const usableW = layoutW - padLeft - padRight;
		const vpad = stretch ? STATIC_VPAD : 0;
		const usableH = layoutH - 2 * vpad;
		if (usableW < iconFloor || usableH < iconFloor) return null;

		const colsAt = (size: number) =>
			Math.floor((usableW + GRID_GAP) / (size + GRID_GAP));
		const rowsAt = (size: number) =>
			Math.floor((usableH + GRID_GAP) / (size + GRID_GAP));
		if (colsAt(iconFloor) < 1 || rowsAt(iconFloor) < 1) return null;

		// Ячейка «+N» нужна, если часть элементов не влезла.
		const cellsFor = (n: number) => n + (total > n ? 1 : 0);
		const maxSize = Math.max(iconFloor, Math.min(maxIcon, usableW));

		let size = iconFloor;
		let count = 0;
		let cols = 1;
		/** Сколько ячеек реально помещается в поле. */
		let cap = Infinity;

		if (forceRows) {
			// Заданное число рядов держим, размер иконок подбираем по высоте.
			const want = Math.max(1, Math.min(maxRows, 10));
			let n = want;
			let fitSize = Math.floor((usableH - (n - 1) * GRID_GAP) / n);
			if (fitSize < iconFloor) {
				// Даже при минимальном размере все ряды не влезают — сокращаем ряды.
				n = Math.max(1, Math.floor((usableH + GRID_GAP) / (iconFloor + GRID_GAP)));
				fitSize = Math.floor((usableH - (n - 1) * GRID_GAP) / n);
			}
			size = clamp(fitSize, iconFloor, maxSize);
			// Колонки заполняют ширину поля при выбранном размере; ячейки растягиваются (1fr).
			const capacity = Math.max(1, colsAt(size)) * n;
			cap = capacity;
			count = shown.length;
			while (count > 0 && cellsFor(count) > capacity) count--;
			cols = Math.max(1, Math.ceil(cellsFor(count) / n));
		} else {
			let fitAll = false;
			for (let s = maxSize; s >= minIcon; s--) {
				if (colsAt(s) * rowsAt(s) >= cellsFor(shown.length)) {
					size = s;
					cap = colsAt(s) * rowsAt(s);
					count = shown.length;
					cols = Math.max(1, colsAt(s));
					fitAll = true;
					break;
				}
			}
			if (!fitAll) {
				size = minIcon;
				const capacity = colsAt(minIcon) * rowsAt(minIcon);
				cap = capacity;
				count = shown.length;
				while (count > 0 && cellsFor(count) > capacity) count--;
				cols = Math.max(1, colsAt(minIcon));
			}
		}

		// Счётчик «+N» — отдельная ячейка в конце списка. Он не должен вытеснять
		// иконку, которая и так влезает: если скрыт ровно один элемент и его
		// можно показать в той же ячейке — показываем его самого.
		if (count < shown.length && count + 1 === total) count = shown.length;
		// Хотя бы одна иконка важнее одного счётчика.
		if (count === 0 && shown.length > 0) count = 1;
		let overflow = Math.max(0, total - count);
		// Для счётчика нет отдельной ячейки — заполняем её следующим элементом,
		// а не оставляем пустое место перед скрытым +N.
		if (overflow > 0 && count + 1 > cap) {
			count = Math.min(shown.length, cap);
			overflow = 0;
		}
		const cells = count + (overflow > 0 ? 1 : 0);
		cols = Math.max(1, Math.min(cols, cells));
		return {
			items: shown.slice(0, count),
			overflow,
			size,
			cols,
			rows: Math.ceil(cells / cols),
			height: stretch ? usableH : undefined,
		};
	});

	// ---- Galería de capturas ------------------------------------------------
	const shots = $derived(isGallery ? items.filter((item) => item.src) : []);
	const galleryReady = $derived(
		isGallery && shots.length > 0 && width >= 48 && height >= 14,
	);
	let index = $state(0);
	/** Sentido de la última transición: 1 = siguiente, -1 = anterior. */
	let dir = $state<1 | -1>(1);
	let failedFull = $state<Record<string, boolean>>({});

	$effect(() => {
		if (index >= shots.length) index = 0;
	});

	function go(step: 1 | -1) {
		if (shots.length < 2) return;
		dir = step;
		index = (index + step + shots.length) % shots.length;
	}

	$effect(() => {
		if (!galleryReady || shots.length < 2 || holdMs === 0) return;
		const current = index;
		const timer = setTimeout(
			() => {
				dir = 1;
				index = (current + 1) % shots.length;
			},
			holdMs + (reducedMotion ? 0 : SLIDE_MS),
		);
		return () => clearTimeout(timer);
	});

	/** Los botones no deben arrastrar ni abrir el mosaico que los contiene. */
	function swallow(event: Event) {
		event.stopPropagation();
	}

	function press(event: Event, step: 1 | -1) {
		event.stopPropagation();
		event.preventDefault();
		go(step);
	}

	function pressKey(event: KeyboardEvent, step: 1 | -1) {
		if (event.key !== "Enter" && event.key !== " ") return;
		press(event, step);
	}

	// Se precarga la siguiente captura para que entre sin parpadeo.
	$effect(() => {
		if (!galleryReady || shots.length < 2) return;
		const next = shots[(index + 1) % shots.length];
		const src = sourceOf(next);
		if (src) {
			const preload = new Image();
			preload.src = src;
		}
	});

	function sourceOf(item: TileItem | undefined): string | null {
		if (!item) return null;
		return item.full && !failedFull[item.key] ? item.full : item.src;
	}

	const barHeight = 2;
	const glassGallery = $derived(galleryCaptionWidth !== undefined);
	const controlWidth = $derived(Math.max(0, width - (galleryCaptionWidth ?? 0)));
	const navPad = $derived(glassGallery ? Math.min(6, controlWidth / 8) : 6);
	const navSize = $derived(glassGallery ? clamp((controlWidth - 2 * navPad) / 2, 0, 22) : 22);
	// Los botones quedan fuera de las franjas desvanecidas (junto al texto y, en el
	// menú compacto, junto al chevron), donde la máscara los haría invisibles.
	const navOffsetLeft = $derived(
		navPad + (!glassGallery && side === "right" ? fade : 0) + (fadeBoth && side === "left" ? 18 : 0),
	);
	const navOffsetRight = $derived(
		navPad + (!glassGallery && side === "left" ? fade : 0) + (fadeBoth && side === "right" ? 18 : 0),
	);
	const slideDuration = $derived(reducedMotion ? 0 : SLIDE_MS);
</script>

{#if galleryReady}
	<span
		class="mq gallery"
		class:fade-from-left={!plain && side === "right"}
		class:fade-from-right={!plain && side === "left"}
		class:both={fadeBoth}
		style:--fade="{fade}px"
		style:--bar-h="{barHeight}px"
		style:--nav-size="{navSize}px"
		aria-hidden="true"
	>
		<span
			class="gal-frame"
			class:edge-to-edge={plain && inset === 0}
			style:left={plain || side === "left" ? `${inset}px` : "0"}
			style:right={plain || side === "right" ? `${inset}px` : "0"}
		>
			{#key index}
			{@const shot = shots[index]}
				<span
					class="gal-slide"
					in:fly={{
						x: dir * width,
						duration: slideDuration,
						easing: cubicInOut,
						opacity: 1,
					}}
					out:fly={{
						x: -dir * width,
						duration: slideDuration,
						easing: cubicInOut,
						opacity: 1,
					}}
				>
					{#if shot}
						<img class="gal-img" src={shot.src} alt="" draggable="false" />
						{#if shot.full && !failedFull[shot.key]}
							<img
								class="gal-img gal-full"
								src={shot.full}
								alt=""
								draggable="false"
								decoding="async"
								onerror={() => {
									failedFull[shot.key] = true;
								}}
							/>
						{/if}
					{/if}
				</span>
			{/key}
			{#if shots.length > 1 && (!glassGallery || galleryCaptionWidth)}
				<span
					class="gal-controls"
					class:bounded={glassGallery}
					style:left={glassGallery ? `${side === "right" ? galleryCaptionWidth : 0}px` : undefined}
					style:right={glassGallery ? `${side === "left" ? galleryCaptionWidth : 0}px` : undefined}
				>
				{#if !glassGallery}<span class="gal-shade"></span>{/if}
				<span
					class="gal-nav gal-prev"
					style:left="{navOffsetLeft}px"
					role="button"
					tabindex="-1"
					aria-label={t("common.previous")}
					onpointerdown={swallow}
					onclick={(event) => press(event, -1)}
					onkeydown={(event) => pressKey(event, -1)}
				>
					<svg viewBox="0 0 24 24" aria-hidden="true"
						><path d="M15 5l-7 7 7 7" /></svg
					>
				</span>
				<span
					class="gal-nav gal-next"
					style:right="{navOffsetRight}px"
					role="button"
					tabindex="-1"
					aria-label={t("common.next")}
					onpointerdown={swallow}
					onclick={(event) => press(event, 1)}
					onkeydown={(event) => pressKey(event, 1)}
				>
					<svg viewBox="0 0 24 24" aria-hidden="true"
						><path d="M9 5l7 7-7 7" /></svg
					>
				</span>
				{#if holdMs > 0}
					<span class="gal-track">
						{#key index}
							<span
								class="gal-bar"
								style:animation-duration="{holdMs}ms"
								style:animation-delay="{reducedMotion
									? 0
									: SLIDE_MS}ms"
							></span>
						{/key}
					</span>
				{/if}
				</span>
			{/if}
		</span>
	</span>
{:else if staticPlan && (appearance.sliderMotion === "off" || !plan)}
	<span
		class="mq static-icons"
		class:fade-from-left={!plain && side === "right"}
		class:fade-from-right={!plain && side === "left"}
		class:both={fadeBoth}
		style:--fade="{fade}px"
		style:--item-gap="{GAP}px"
		aria-hidden="true"
	>
		<span
			class="mq-static-wrap"
			style:width="{layoutW}px"
			style:height="{layoutH}px"
			style:justify-content={side === "right" ? "flex-end" : side === "center" ? "center" : "flex-start"}
			style:align-items={verticalAlign === "start" ? "flex-start" : verticalAlign === "end" ? "flex-end" : "center"}
			style:padding-left="{padLeft}px"
			style:padding-right="{padRight}px"
			style:transform={scale !== 1 ? `scale(${scale})` : undefined}
		>
			<span
				class="mq-static"
				style:--s="{staticPlan.size}px"
				style:--cols={staticPlan.cols}
				style:--gap="{GRID_GAP}px"
				style:height={staticPlan.height !== undefined ? `${staticPlan.height}px` : undefined}
				style:align-content={staticPlan.height !== undefined && staticPlan.rows > 1 ? "space-between" : "center"}
			>
				{#each staticPlan.items as item (item.key)}
					<span class="mq-static-item">
						{#if item.src}
							<img class="mq-img" src={item.src} alt="" draggable="false" loading="lazy" decoding="async" />
						{:else}
							<span class="mq-img mq-placeholder">{initial(item)}</span>
						{/if}
					</span>
				{/each}
				{#if staticPlan.overflow > 0}
					<span class="mq-overflow">+{staticPlan.overflow}</span>
				{/if}
			</span>
		</span>
	</span>
{:else if plan}
	<span
		class="mq"
		class:fade-from-left={!plain && side === "right"}
		class:fade-from-right={!plain && side === "left"}
		class:both={fadeBoth}
		class:motion-hover={appearance.sliderMotion === "hover"}
		class:motion-off={appearance.sliderMotion === "off"}
		class:soft-inset={plan.scrolls && (motionInset.left > 0 || motionInset.right > 0)}
		style:--inset-left="{motionInset.left}px"
		style:--inset-right="{motionInset.right}px"
		style:--inset-fade-left="{motionInset.left > 0 ? motionInset.left + 4 : 0}px"
		style:--inset-fade-right="{motionInset.right > 0 ? motionInset.right + 4 : 0}px"
		style:--fade="{fade}px"
		style:--item-gap="{GAP}px"
		style:--s="{plan.size}px"
		style:--lf="{plan.labelFont}px"
		aria-hidden="true"
	>
			<span
				class="mq-rows"
				style:gap="{ROW_GAP}px"
				style:width="{layoutW}px"
				style:height="{layoutH}px"
				style:justify-content={verticalAlign === "start" ? "flex-start" : verticalAlign === "end" ? "flex-end" : "center"}
				style:transform={scale !== 1 ? `scale(${scale})` : undefined}
			>
				{#each plan.rows as row (row.index)}
					<span
						class="mq-row"
						class:still={!plan.scrolls}
						style:justify-content={!plan.scrolls
							? side === "right"
								? "flex-end"
								: side === "center" ? "center" : "flex-start"
							: undefined}
						style:padding-left={!plan.scrolls
							? `${motionInset.left / scale + (!plain && side === "right" ? edgePad : 0)}px`
							: undefined}
						style:padding-right={!plan.scrolls
							? `${motionInset.right / scale + (!plain && side === "left" ? edgePad : 0)}px`
							: undefined}
					>
						<span
							class="mq-track"
							class:reverse={row.reverse}
							class:still={!plan.scrolls}
							style:animation-duration="{row.duration / speedFactor}s"
							style:animation-delay="{row.delay / speedFactor}s"
						>
							{#each plan.copies as copy (copy)}
								<span class="mq-seq">
									{#each row.items as item, n (n)}
										<span
											class="mq-item"
											class:chip={!hideText && item.label !== null}
										>
											{#if item.src}
												<img
													class="mq-img"
													src={item.src}
													alt=""
													draggable="false"
													loading="lazy"
													decoding="async"
												/>
											{:else}
												<span
													class="mq-img mq-placeholder"
													style:background="hsl({hue(
														item,
													)} 38% 42%)"
													>{initial(item)}</span
												>
											{/if}
											{#if !hideText && item.label !== null}
												<span class="mq-label"
													>{item.label}</span
												>
											{/if}
										</span>
									{/each}
									{#if row.spacer > 0}
										<span
											class="mq-spacer"
											style:width="{row.spacer}px"
										></span>
									{/if}
								</span>
							{/each}
						</span>
					</span>
				{/each}
			</span>
	</span>
{/if}

<style>
	.mq {
		display: block;
		position: absolute;
		inset: 0;
		overflow: hidden;
		pointer-events: none;
		user-select: none;
	}

	.mq.fade-from-left {
		-webkit-mask-image: linear-gradient(
			to right,
			transparent 0,
			#000 var(--fade)
		);
		mask-image: linear-gradient(to right, transparent 0, #000 var(--fade));
	}

	.mq.fade-from-right {
		-webkit-mask-image: linear-gradient(
			to left,
			transparent 0,
			#000 var(--fade)
		);
		mask-image: linear-gradient(to left, transparent 0, #000 var(--fade));
	}

	.mq.both.fade-from-left {
		-webkit-mask-image: linear-gradient(
			to right,
			transparent 0,
			#000 var(--fade),
			#000 calc(100% - 18px),
			transparent 100%
		);
		mask-image: linear-gradient(
			to right,
			transparent 0,
			#000 var(--fade),
			#000 calc(100% - 18px),
			transparent 100%
		);
	}

	.mq.both.fade-from-right {
		-webkit-mask-image: linear-gradient(
			to left,
			transparent 0,
			#000 var(--fade),
			#000 calc(100% - 18px),
			transparent 100%
		);
		mask-image: linear-gradient(
			to left,
			transparent 0,
			#000 var(--fade),
			#000 calc(100% - 18px),
			transparent 100%
		);
	}

	.mq-static-wrap {
		position: absolute;
		top: 0;
		left: 0;
		box-sizing: border-box;
		display: flex;
		align-items: center;
		transform-origin: 0 0;
	}
	/* Колонки делят всю доступную ширину поля (1fr), а не фиксированы размером иконки:
	   поэтому иконки растягиваются на доступное место, а сама иконка остаётся --s. */
	.mq-static {
		flex: 0 0 auto;
		width: max-content;
		min-width: 0;
		display: grid;
		grid-template-columns: repeat(var(--cols), var(--s));
		grid-auto-rows: var(--s);
		gap: var(--gap);
		justify-items: center;
		align-items: center;
		align-content: center;
	}
	.mq-static-item {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 0;
		width: var(--s);
		height: var(--s);
	}
	.mq-static-item .mq-img {
		width: var(--s);
		height: var(--s);
	}
	.mq-overflow {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--s);
		height: var(--s);
		padding: 0 5px;
		box-sizing: border-box;
		border-radius: 24%;
		background: color-mix(in srgb, var(--text-primary) 10%, transparent);
		color: var(--text-primary);
		font-size: clamp(9px, calc(var(--s) * 0.32), 15px);
		font-weight: 700;
		white-space: nowrap;
	}

	/* One stationary mask: a short fade avoids a flickering hard cut through moving icons. */
	.mq.soft-inset {
		--left-fade: 0px;
		--right-fade: 0px;
		mask-image: linear-gradient(
			to right,
			transparent var(--inset-left),
			#000 max(var(--left-fade), var(--inset-fade-left)),
			#000 calc(100% - max(var(--right-fade), var(--inset-fade-right))),
			transparent calc(100% - var(--inset-right))
		);
	}

	.mq.soft-inset.fade-from-left {
		--left-fade: var(--fade);
	}

	.mq.soft-inset.fade-from-right {
		--right-fade: var(--fade);
	}

	.mq-rows {
		display: flex;
		flex-direction: column;
		justify-content: center;
		height: 100%;
		transform-origin: 0 0;
	}

	.mq-row {
		display: flex;
		flex-shrink: 0;
		height: var(--s);
		overflow: hidden;
	}

	.mq-row.still {
		overflow: visible;
	}

	.mq-track {
		display: flex;
		flex-shrink: 0;
		width: max-content;
		height: 100%;
		animation: mq-slide linear infinite;
		will-change: transform;
	}

	.mq-track.still {
		animation: none;
		will-change: auto;
	}

	/* Movimiento de las cintas según el ajuste: solo al pasar el cursor o desactivado. */
	.mq.motion-off .mq-track {
		animation-play-state: paused;
	}

	:global(.tile:not(:hover):not(:focus-visible)) .mq.motion-hover .mq-track,
	:global(.nav-item:not(:hover):not(:focus-visible)) .mq.motion-hover .mq-track {
		animation-play-state: paused;
	}

	.mq-track.reverse {
		animation-direction: reverse;
	}

	@keyframes mq-slide {
		from {
			transform: translate3d(0, 0, 0);
		}
		to {
			transform: translate3d(-50%, 0, 0);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.mq-track {
			animation: none;
		}
	}

	.mq-seq {
		display: flex;
		flex-shrink: 0;
		align-items: center;
	}

	.mq-item {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		height: var(--s);
		margin-right: var(--item-gap, 8px);
	}

	.mq-item.chip {
		gap: 6px;
		padding-right: 10px;
		border-radius: calc(var(--s) * 0.26);
		background: color-mix(in srgb, var(--text-primary) 7%, transparent);
	}

	.mq-img {
		display: block;
		flex-shrink: 0;
		width: var(--s);
		height: var(--s);
		border-radius: 24%;
		object-fit: cover;
		background: color-mix(in srgb, var(--text-primary) 8%, transparent);
	}

	.mq-placeholder {
		display: flex;
		align-items: center;
		justify-content: center;
		color: #fff;
		font-size: calc(var(--s) * 0.46);
		font-weight: 700;
	}

	.mq-label {
		max-width: calc(22 * var(--lf) * 0.6);
		overflow: hidden;
		color: var(--text-secondary);
		font-size: var(--lf);
		font-weight: 600;
		line-height: 1;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* ---- Galería de capturas ---- */
	.gal-frame {
		position: absolute;
		top: 0;
		bottom: 0;
		overflow: hidden;
		border-radius: var(--border-radius-sm, 8px);
		background: color-mix(in srgb, var(--text-primary) 8%, transparent);
	}
	.gal-frame.edge-to-edge {
		/* The tile itself clips the image to its outer corners. */
		border-radius: 0;
	}

	.gal-controls:not(.bounded) {
		display: contents;
	}
	.gal-controls.bounded {
		position: absolute;
		top: 0;
		bottom: 0;
	}

	.gal-slide {
		position: absolute;
		inset: 0;
		display: block;
	}

	.gal-img {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.gal-full {
		position: absolute;
		inset: 0;
	}

	/* Sombra suave bajo la barra para que se lea sobre cualquier captura. */
	.gal-shade {
		position: absolute;
		left: 0;
		right: 0;
		bottom: 0;
		height: calc(var(--bar-h) * 6 + 8px);
		background: linear-gradient(
			to top,
			color-mix(in srgb, var(--bg-main, #000) 70%, transparent),
			transparent
		);
		pointer-events: none;
	}

	/* La "sombra" de la barra es el tiempo total que se mantiene la captura. */
	.gal-track {
		position: absolute;
		z-index: 3;
		left: 50%;
		bottom: 7px;
		width: min(60px, 42%);
		height: 6px;
		transform: translateX(-50%);
		overflow: hidden;
		border-radius: 999px;
		background: rgba(0, 0, 0, 0.9);
		box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.22), 0 1px 4px rgba(0, 0, 0, 0.5);
	}

	/* La barra se llena de izquierda a derecha, sea cual sea el lado del mosaico. */
	.gal-bar {
		display: block;
		position: absolute;
		left: 0;
		top: 2px;
		width: 100%;
		height: 2px;
		border-radius: inherit;
		background: #fff;
		transform-origin: left center;
		animation-name: gal-elapsed;
		animation-timing-function: linear;
		animation-fill-mode: both;
	}

	@keyframes gal-elapsed {
		from {
			transform: scaleX(0);
		}
		to {
			transform: scaleX(1);
		}
	}

	.gal-nav {
		position: absolute;
		top: 50%;
		z-index: 2;
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--nav-size);
		height: var(--nav-size);
		transform: translateY(-50%);
		border-radius: 50%;
		background: color-mix(in srgb, var(--bg-main, #000) 55%, transparent);
		color: var(--text-primary);
		/* Скрыты, пока курсор не на плитке: без наведения не видны и не кликаются. */
		opacity: 0;
		pointer-events: none;
		cursor: pointer;
		transition:
			opacity 0.1s ease,
			background 0.12s ease;
	}

	:global(.tile:hover) .gal-nav {
		opacity: 0.7;
		pointer-events: auto;
	}

	/* Fit the glass controls to the rendered tile height, including during resize. */
	.bounded .gal-nav {
		z-index: 4;
		width: auto;
		height: min(var(--nav-size), max(0px, calc(100% - 4px)));
		aspect-ratio: 1;
	}

	.gal-nav:hover {
		opacity: 1;
		background: color-mix(in srgb, var(--bg-main, #000) 80%, transparent);
	}

	.gal-nav svg {
		width: 14px;
		height: 14px;
		max-width: 100%;
		max-height: 100%;
		fill: none;
		stroke: currentColor;
		stroke-width: 2.4;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.mq-spacer {
		flex-shrink: 0;
		height: 1px;
	}
</style>
