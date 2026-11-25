/** Layout shared by the game header and the sidebar. Stored locally because it is purely UI state. */
import type { SidebarDock } from "./sidebarLayoutState.svelte";

/** Игровая панель закрепляется только сверху или снизу; боковые варианты удалены. */
export type GamePanelDock =
        | "top-start"
        | "top-end"
        | "bottom-start"
        | "bottom-end"
;

const KEY = "cubic.panelLayout.v1";

/** Совместимость со старыми сохранёнными значениями верхнего/нижнего края. */
const LEGACY: Record<string, GamePanelDock> = {
        top: "top-start",
        bottom: "bottom-start",
};
const DOCKS: GamePanelDock[] = [
        "top-start",
        "top-end",
        "bottom-start",
        "bottom-end",
];

function load() {
        const value: { gameDock: GamePanelDock; locked: boolean } = {
                gameDock: "top-start",
                locked: false,
        };
        try {
                const saved = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<{
                        gameDock: unknown;
                        locked: unknown;
                }> | null;
                const rawDock = typeof saved?.gameDock === "string" ? saved.gameDock : "";
                if (DOCKS.includes(rawDock as GamePanelDock)) value.gameDock = rawDock as GamePanelDock;
                else if (rawDock in LEGACY) value.gameDock = LEGACY[rawDock];
                if (typeof saved?.locked === "boolean") value.locked = saved.locked;
        } catch { /* use defaults */ }
        return value;
}
export const panelLayout = $state(load());
function persist() {
        try { localStorage.setItem(KEY, JSON.stringify(panelLayout)); } catch { /* session-only */ }
}

/** Edge of the window the game header is docked to. */
export function gamePanelEdge(dock: GamePanelDock): "top" | "bottom" {
        return dock.startsWith("bottom") ? "bottom" : "top";
}

/** Зеркальное выравнивание для верхней/нижней половины «end». */
export function gamePanelMirrored(dock: GamePanelDock): boolean {
        return dock.endsWith("end");
}

export function setGamePanelDock(dock: GamePanelDock) {
        if (!DOCKS.includes(dock)) return;
        panelLayout.gameDock = dock;
        persist();
}

export function setPanelOrientationLocked(locked: boolean) {
        panelLayout.locked = locked;
        persist();
}

export type { SidebarDock };
