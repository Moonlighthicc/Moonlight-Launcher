/**
 * Disposición de la barra lateral:
 *  - `dock`: borde de la ventana al que está acoplada (izquierda / derecha / arriba / abajo).
 *  - `align`: mitad del borde en la que se soltó. "start" mantiene el contenido en su lado
 *    habitual; "end" lo refleja por completo (orden invertido).
 *  - Orden de las instancias: las ancladas van primero (en el orden que el usuario les dio) y
 *    el resto se ordena por el criterio elegido o, tras arrastrarlas, de forma manual.
 * Todo se guarda en localStorage para sobrevivir al cierre del lanzador.
 */
import type { InstanceDto } from "$lib/types/types";

export type SidebarAlign = "start" | "end";

export type SidebarDock = "left" | "right" | "top" | "bottom";
export type SortKey = "lastPlayed" | "name" | "playtime" | "created";
export type SortMode = SortKey | "manual";
export type SortDir = "asc" | "desc";

export const SORT_KEYS: SortKey[] = [
        "lastPlayed",
        "name",
        "playtime",
        "created",
];

/** Dirección inicial al elegir un criterio (lo más reciente / más largo primero, nombres A → Z). */
export const DEFAULT_DIR: Record<SortKey, SortDir> = {
        lastPlayed: "desc",
        name: "asc",
        playtime: "desc",
        created: "desc",
};

const STORAGE_KEY = "cubic.sidebarLayout.v1";
const SEEN_KEY = "cubic.instanceFirstSeen.v1";
const DOCKS: SidebarDock[] = ["left", "right", "top", "bottom"];

interface Saved {
        dock: SidebarDock;
        align: SidebarAlign;
        sortMode: SortMode;
        sortDir: SortDir;
        pinOrder: string[];
        manualOrder: string[];
}

function isStringArray(value: unknown): value is string[] {
        return Array.isArray(value) && value.every((v) => typeof v === "string");
}

function load(): Saved {
        const saved: Saved = {
                dock: "left",
                align: "start",
                sortMode: "lastPlayed",
                sortDir: DEFAULT_DIR.lastPlayed,
                pinOrder: [],
                manualOrder: [],
        };
        try {
                const raw = localStorage.getItem(STORAGE_KEY);
                if (!raw) return saved;
                const data = JSON.parse(raw) as Partial<Record<keyof Saved, unknown>>;
                if (DOCKS.includes(data.dock as SidebarDock))
                        saved.dock = data.dock as SidebarDock;
                if (data.align === "start" || data.align === "end")
                        saved.align = data.align;
                if (
                        data.sortMode === "manual" ||
                        SORT_KEYS.includes(data.sortMode as SortKey)
                )
                        saved.sortMode = data.sortMode as SortMode;
                if (data.sortDir === "asc" || data.sortDir === "desc")
                        saved.sortDir = data.sortDir;
                if (isStringArray(data.pinOrder)) saved.pinOrder = data.pinOrder;
                if (isStringArray(data.manualOrder))
                        saved.manualOrder = data.manualOrder;
        } catch {
                /* localStorage no disponible o JSON dañado: valores por defecto */
        }
        return saved;
}

export const sidebarLayout = $state<Saved>(load());

function save() {
        try {
                localStorage.setItem(STORAGE_KEY, JSON.stringify(sidebarLayout));
        } catch {
                /* sin almacenamiento: solo dura esta sesión */
        }
}

/** Cambia solo el borde; conserva el alineamiento actual (usado por Ajustes). */
export function setSidebarDock(dock: SidebarDock) {
        if (sidebarLayout.dock === dock) return;
        sidebarLayout.dock = dock;
        save();
}

/** Cambia borde y alineación a la vez (al soltar la barra en una mitad del borde). */
export function setSidebarPlacement(dock: SidebarDock, align: SidebarAlign) {
        if (sidebarLayout.dock === dock && sidebarLayout.align === align) return;
        sidebarLayout.dock = dock;
        sidebarLayout.align = align;
        save();
}

export function setSidebarAlign(align: SidebarAlign) {
        if (sidebarLayout.align === align) return;
        sidebarLayout.align = align;
        save();
}

/**
 * Положение боковой панели одной из шести зон (та же модель, что у игровой панели):
 * левый и правый край — одна зона каждый с одинаковым порядком элементов, верх и низ — по две
 * половины («start» — обычная раскладка, «end» — зеркальная).
 */
export type SidebarCell = "left" | "right" | "top-start" | "top-end" | "bottom-start" | "bottom-end";

export function cellOf(dock: SidebarDock, align: SidebarAlign): SidebarCell {
        if (dock === "left" || dock === "right") return dock;
        return `${dock}-${align}` as SidebarCell;
}

export function placementOf(cell: SidebarCell): { dock: SidebarDock; align: SidebarAlign } {
        if (cell === "left") return { dock: "left", align: "start" };
        if (cell === "right") return { dock: "right", align: "start" };
        const [dock, align] = cell.split("-") as [SidebarDock, SidebarAlign];
        return { dock, align };
}

/** Eje principal de la lista de instancias según el borde en que está la barra. */
export function isHorizontalDock(dock: SidebarDock): boolean {
        return dock === "top" || dock === "bottom";
}

/** Elige un criterio; si ya estaba activo, invierte la dirección. */
export function chooseSort(key: SortKey) {
        if (sidebarLayout.sortMode === key) {
                sidebarLayout.sortDir = sidebarLayout.sortDir === "asc" ? "desc" : "asc";
        } else {
                sidebarLayout.sortMode = key;
                sidebarLayout.sortDir = DEFAULT_DIR[key];
        }
        save();
}

// ---- Fecha de creación -------------------------------------------------------
// Las instancias anteriores a `created_at` llegan con 0: se usa la primera vez que el
// lanzador las vio, que es lo más cercano que se puede saber sin tocar los archivos.
let firstSeen: Record<string, number> = {};
try {
        const raw = localStorage.getItem(SEEN_KEY);
        if (raw) firstSeen = JSON.parse(raw) as Record<string, number>;
} catch {
        firstSeen = {};
}

/** Registra las instancias nuevas sin fecha; llamar fuera de contextos reactivos de lectura. */
export function rememberFirstSeen(instances: InstanceDto[]) {
        let changed = false;
        const now = Math.floor(Date.now() / 1000);
        for (const instance of instances) {
                if (!instance.created_at && firstSeen[instance.uuid] === undefined) {
                        firstSeen[instance.uuid] = now;
                        changed = true;
                }
        }
        if (!changed) return;
        try {
                localStorage.setItem(SEEN_KEY, JSON.stringify(firstSeen));
        } catch {
                /* ignorado */
        }
}

function createdOf(instance: InstanceDto): number {
        return instance.created_at || firstSeen[instance.uuid] || 0;
}

// ---- Orden --------------------------------------------------------------------
function byName(a: InstanceDto, b: InstanceDto): number {
        return a.name.localeCompare(b.name, undefined, {
                numeric: true,
                sensitivity: "base",
        });
}

function compareBy(key: SortKey, dir: SortDir) {
        const sign = dir === "asc" ? 1 : -1;
        return (a: InstanceDto, b: InstanceDto): number => {
                let diff = 0;
                switch (key) {
                        case "name":
                                diff = byName(a, b);
                                break;
                        case "lastPlayed":
                                diff = a.last_played - b.last_played;
                                break;
                        case "playtime":
                                diff = a.playtime_seconds - b.playtime_seconds;
                                break;
                        case "created":
                                diff = createdOf(a) - createdOf(b);
                                break;
                }
                // Los empates se desempatan siempre igual, sin depender de la dirección.
                return diff !== 0 ? diff * sign : byName(a, b);
        };
}

function byListPosition(order: string[], fallback: (a: InstanceDto, b: InstanceDto) => number) {
        const position = new Map(order.map((uuid, i) => [uuid, i]));
        return (a: InstanceDto, b: InstanceDto): number => {
                const pa = position.get(a.uuid);
                const pb = position.get(b.uuid);
                if (pa !== undefined && pb !== undefined) return pa - pb;
                // Las que no están en la lista guardada van después, en un orden estable.
                if (pa !== undefined) return -1;
                if (pb !== undefined) return 1;
                return fallback(a, b);
        };
}

export function splitInstances(list: InstanceDto[]): {
        pinned: InstanceDto[];
        rest: InstanceDto[];
} {
        const pinned = list
                .filter((i) => i.pinned)
                .sort(byListPosition(sidebarLayout.pinOrder, byName));
        const rest = list.filter((i) => !i.pinned);
        if (sidebarLayout.sortMode === "manual") {
                rest.sort(
                        byListPosition(
                                sidebarLayout.manualOrder,
                                compareBy("lastPlayed", "desc"),
                        ),
                );
        } else {
                rest.sort(compareBy(sidebarLayout.sortMode, sidebarLayout.sortDir));
        }
        return { pinned, rest };
}

/** Lista final: ancladas arriba (en su orden) y debajo el resto según el criterio activo. */
export function orderInstances(list: InstanceDto[]): InstanceDto[] {
        const { pinned, rest } = splitInstances(list);
        return [...pinned, ...rest];
}

/** Guarda el nuevo orden de las ancladas. */
export function commitPinOrder(uuids: string[]) {
        sidebarLayout.pinOrder = uuids;
        save();
}

/** Guarda un orden manual del resto de instancias y pasa a modo manual. */
export function commitManualOrder(uuids: string[]) {
        sidebarLayout.manualOrder = uuids;
        sidebarLayout.sortMode = "manual";
        save();
}

/** Coloca una instancia recién anclada al final de las ancladas (justo sobre las demás). */
export function appendToPinOrder(uuid: string, current: string[]) {
        sidebarLayout.pinOrder = [...current.filter((id) => id !== uuid), uuid];
        save();
}

export function removeFromPinOrder(uuid: string) {
        if (!sidebarLayout.pinOrder.includes(uuid)) return;
        sidebarLayout.pinOrder = sidebarLayout.pinOrder.filter((id) => id !== uuid);
        save();
}
