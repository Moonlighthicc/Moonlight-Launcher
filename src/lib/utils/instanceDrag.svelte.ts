/**
 * Arrastre de instancias dentro de la barra lateral (con punteros, no con el DnD HTML5,
 * que en el webview no es fiable). Mientras se arrastra, `display` devuelve la lista ya
 * reordenada para que el resto de filas se aparte en vivo.
 *
 * Reglas:
 *  - Las ancladas solo se mueven entre ellas y nunca bajan de las no ancladas.
 *  - Las no ancladas solo se mueven entre ellas.
 *  - Soltar sobre la zona de anclado (cabecera) ancla la instancia, o la desancla si ya lo estaba.
 */
import type { InstanceDto } from "$lib/types/types";

const START_DISTANCE = 6;
const EDGE_ZONE = 44;
const EDGE_SPEED = 14;

export interface InstanceDragOptions {
	/** Lista completa en el orden actual (ancladas primero). */
	list: () => InstanceDto[];
	/** Posición (índice en la lista mostrada) que corresponde al puntero. */
	indexAt: (x: number, y: number) => number;
	/** ¿El puntero está sobre la zona de anclado? */
	inPinZone: (x: number, y: number) => boolean;
	/** Desplaza la lista si el puntero está junto a su borde; devuelve true si se movió. */
	autoScroll?: (x: number, y: number) => boolean;
	onreorder: (instance: InstanceDto, ordered: InstanceDto[]) => void;
	onpinchange: (instance: InstanceDto, pinned: boolean) => void;
}

export const DRAG_EDGE = { zone: EDGE_ZONE, speed: EDGE_SPEED };

export class InstanceDrag {
	uuid = $state<string | null>(null);
	active = $state(false);
	x = $state(0);
	y = $state(0);
	target = $state(0);
	pinZone = $state(false);

	#startX = 0;
	#startY = 0;
	#frame: number | undefined;
	#options: InstanceDragOptions;

	constructor(options: InstanceDragOptions) {
		this.#options = options;
	}

	get dragged(): InstanceDto | null {
		if (!this.uuid) return null;
		return this.#options.list().find((i) => i.uuid === this.uuid) ?? null;
	}

	/** Lista con la instancia arrastrada ya en su posición provisional. */
	get display(): InstanceDto[] {
		const base = this.#options.list();
		if (!this.active || this.pinZone) return base;
		const from = base.findIndex((i) => i.uuid === this.uuid);
		if (from < 0) return base;
		const item = base[from];
		const pinnedCount = base.filter((i) => i.pinned).length;
		const min = item.pinned ? 0 : pinnedCount;
		const max = item.pinned ? pinnedCount - 1 : base.length - 1;
		const to = Math.min(max, Math.max(min, this.target));
		const rest = base.filter((_, i) => i !== from);
		rest.splice(to, 0, item);
		return rest;
	}

	/** Se llama desde `onpointerdown` de una fila. */
	start = (event: PointerEvent, instance: InstanceDto) => {
		if (event.button !== 0 || this.uuid) return;
		const target = event.target as HTMLElement | null;
		if (target?.closest("[data-no-drag]")) return;
		this.uuid = instance.uuid;
		this.active = false;
		this.#startX = event.clientX;
		this.#startY = event.clientY;
		this.x = event.clientX;
		this.y = event.clientY;
		window.addEventListener("dragstart", this.#noNativeDrag);
		window.addEventListener("pointermove", this.#move);
		window.addEventListener("pointerup", this.#up);
		window.addEventListener("pointercancel", this.#cancel);
		window.addEventListener("keydown", this.#key);
	};

	// Un arrastre nativo (p. ej. de la <img> del avatar) cancelaría los eventos de puntero
	// y mostraría el cursor rojo de "no permitido".
	#noNativeDrag = (event: DragEvent) => event.preventDefault();

	#move = (event: PointerEvent) => {
		this.x = event.clientX;
		this.y = event.clientY;
		if (!this.active) {
			if (
				Math.hypot(event.clientX - this.#startX, event.clientY - this.#startY) <
				START_DISTANCE
			)
				return;
			this.active = true;
			document.documentElement.classList.add("instance-dragging");
			this.#loop();
		}
		this.#update();
	};

	#update() {
		this.pinZone = this.#options.inPinZone(this.x, this.y);
		if (!this.pinZone) this.target = this.#options.indexAt(this.x, this.y);
	}

	// Mantiene el desplazamiento automático aunque el puntero esté quieto junto al borde.
	#loop = () => {
		if (!this.active) return;
		if (this.#options.autoScroll?.(this.x, this.y)) this.#update();
		this.#frame = requestAnimationFrame(this.#loop);
	};

	#up = () => {
		const instance = this.dragged;
		const wasActive = this.active;
		const before = this.#options.list().map((i) => i.uuid);
		const after = this.display;
		const overPin = this.pinZone;
		this.#finish();
		if (!wasActive || !instance) return;
		// El clic que sigue a soltar no debe seleccionar la instancia.
		const swallow = (e: Event) => {
			e.stopPropagation();
			e.preventDefault();
		};
		window.addEventListener("click", swallow, { capture: true, once: true });
		setTimeout(() => window.removeEventListener("click", swallow, true), 0);
		if (overPin) {
			this.#options.onpinchange(instance, !instance.pinned);
		} else if (after.some((item, i) => item.uuid !== before[i])) {
			this.#options.onreorder(instance, after);
		}
	};

	#cancel = () => this.#finish();

	#key = (event: KeyboardEvent) => {
		if (event.key === "Escape") this.#finish();
	};

	#finish() {
		window.removeEventListener("dragstart", this.#noNativeDrag);
		window.removeEventListener("pointermove", this.#move);
		window.removeEventListener("pointerup", this.#up);
		window.removeEventListener("pointercancel", this.#cancel);
		window.removeEventListener("keydown", this.#key);
		if (this.#frame !== undefined) cancelAnimationFrame(this.#frame);
		this.#frame = undefined;
		document.documentElement.classList.remove("instance-dragging");
		this.active = false;
		this.pinZone = false;
		this.uuid = null;
	}

	destroy() {
		this.#finish();
	}
}
