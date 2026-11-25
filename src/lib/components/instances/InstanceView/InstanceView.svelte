<script lang="ts">
        import { InstState, type InstanceDto } from "$lib/types/types";
        import { checkVersionIntegrity } from "$lib/api/cubicApi";
        import { playInstance } from "$lib/api/instancePlay";
        import { t } from "$lib/i18n";
        import { launcherStore, showError } from "$lib/state/state.svelte";
        import { lazyComponent } from "$lib/utils/lazyComponent.svelte";
        import { animDuration } from "$lib/utils/animations";
        import { cubicOut } from "svelte/easing";
        import { isVersionDownloading } from "$lib/state/downloadState.svelte";
        import { clearGameReady, isGameReady, isLaunching } from "$lib/state/gameReadyState.svelte";
        import {
                downloads,
                getOverallPct,
                hasDownloadFailed,
        } from "$lib/state/downloadQueueState.svelte";
        import InstanceHeader from "./InstanceHeader.svelte";
        import InstanceModpack from "./InstanceModpack.svelte";
        import ResourcesIcon from "$lib/icons/ResourcesIcon.svelte";
        import ImageIcon from "$lib/icons/ImageIcon.svelte";
        import ShadersIcon from "$lib/icons/ShadersIcon.svelte";
        import ChevronRightIcon from "$lib/icons/ChevronRightIcon.svelte";
        import WorldIcon from "$lib/icons/WorldIcon.svelte";
        import Icon from "$lib/icons/Icon.svelte";
        import ListSkeleton from "$lib/components/ui/ListSkeleton.svelte";
        import PanelSkeleton from "$lib/components/ui/PanelSkeleton.svelte";
        import MarketCatalogSkeleton from "$lib/components/market/MarketCatalogSkeleton.svelte";
        import TilesIcon from "$lib/icons/TilesIcon.svelte";
        import TileSlider from "./TileSlider.svelte";
        import TileSettingsPanel from "./TileSettingsPanel.svelte";
        import ContextMenu, { type ContextMenuItem } from "$lib/components/layout/ContextMenu.svelte";
        import { hasOpenModal } from "$lib/components/layout/ModalBase.svelte";
        import { openInstanceDir } from "$lib/api/cubicApi";
        import ChevronGlyph from "$lib/components/ui/ChevronGlyph.svelte";
        import { saveSettings, killInst } from "$lib/api/launcherService";
        import { panelLayout, setPanelOrientationLocked, setGamePanelDock, gamePanelEdge, gamePanelMirrored, type GamePanelDock } from "$lib/state/panelLayoutState.svelte";
        import { appearance, setSliderMotion, setQuickActionsDisabled } from "$lib/state/appearanceState.svelte";
        import {
                resolveTextPos,
                resolveTileSide,
                sideForRect,
                resolveGlassBlur,
                resolveTileStyle,
        } from "$lib/state/tileStyle";
        import { tileBoardGeometry } from "$lib/utils/tileBoardGeometry";
        import { resolveTileSliderEdgeInset } from "$lib/state/tileDefaults";
        import { fly } from "svelte/transition";
        import { onDestroy, onMount } from "svelte";
        import {
                GRID_COLS,
                GRID_ROWS,
                MIN_TILE_H,
                MIN_TILE_W,
                TILE_IDS,
                clampRect,
                commitTileLayout,
                instanceNav,
                setTileMode,
                setTileRect,
                moveTile,
                setTileHidden,
                setTileNoBlur,
                tileLayout,
                type TileId,
                type TileRect,
        } from "$lib/state/instanceNavState.svelte";

        let { selectedInstance, onopenlibrary }: { selectedInstance: InstanceDto; onopenlibrary: () => void } = $props();
        let tabContentEl: HTMLDivElement | undefined = $state();
        const showingDetails = $derived(instanceNav.section === "detalles");
        let tileContextOpen = $state(false);
        let tileContextX = $state(0);
        let tileContextY = $state(0);
        let tileContextItems = $state<ContextMenuItem[]>([]);

        let bannerState = $derived.by(() => {
                if (selectedInstance.status === InstState.Started) return "Started";
                if (selectedInstance.status === InstState.Off) return "Idle";
                if (selectedInstance.status === InstState.Error) return "Error";
                if (selectedInstance.status === InstState.Starting) return "Starting";
                return "Idle";
        });
        // Индикатор запуска: виден, пока экземпляр запускается, а окно игры ещё не появилось.
        const launchPendingRaw = $derived(
                bannerState === "Starting" ||
                        (bannerState === "Started" &&
                                isLaunching(selectedInstance.uuid) &&
                                !isGameReady(selectedInstance.uuid)),
        );
        /** Запасной предел: если сигнал готовности так и не пришёл, индикатор не крутится вечно. */
        const LAUNCH_INDICATOR_MAX_MS = 60_000;
        let launchTimedOut = $state(false);
        $effect(() => {
                if (!launchPendingRaw) {
                        launchTimedOut = false;
                        return;
                }
                const timer = window.setTimeout(() => (launchTimedOut = true), LAUNCH_INDICATOR_MAX_MS);
                return () => window.clearTimeout(timer);
        });
        const launchPending = $derived(launchPendingRaw && !launchTimedOut);
        $effect(() => {
                // Сбрасываем готовность, как только игра не запущена (для следующего запуска).
                if (bannerState !== "Started") clearGameReady(selectedInstance.uuid);
        });

        const loaderLower = $derived(selectedInstance.loader.toLowerCase());
        // Disable Market (mods browser) for Vanilla and OptiFine; allow for others.
        const supportsMods = $derived(
                loaderLower !== "vanilla" && loaderLower !== "optifine",
        );
        // Shaders/resources are allowed for all non-Vanilla loaders, including OptiFine.
        const supportsShaders = $derived(loaderLower !== "vanilla");
        const modpackDownloadId = $derived(`modpack-${selectedInstance.uuid}`);
        let checkedInstallation = $state<{ version: string; complete: boolean } | null>(null);
        let enqueueingVersion = $state<string | null>(null);
        let integrityRevision = $state(0);
        const needsDownload = $derived(
                checkedInstallation?.version === selectedInstance.version
                        ? !checkedInstallation.complete
                        : hasDownloadFailed(selectedInstance.version),
        );
        const downloadKind = $derived(
                isVersionDownloading(modpackDownloadId)
                        ? "mods"
                        : isVersionDownloading(selectedInstance.version) ||
                          enqueueingVersion === selectedInstance.version
                                ? "version"
                                : null,
        );
        function refreshIntegrity() {
                integrityRevision++;
        }

        $effect(() => {
                const { uuid, version } = selectedInstance;
                const revision = integrityRevision;
                if (downloadKind) return;
                let current = true;
                void checkVersionIntegrity(version).then((result) => {
                        if (current && selectedInstance.uuid === uuid &&
                                selectedInstance.version === version && integrityRevision === revision &&
                                !downloadKind && result) {
                                checkedInstallation = { version, complete: result.complete };
                        }
                });
                return () => { current = false; };
        });
        const downloadItem = $derived(downloads.get(
                downloadKind === "mods" ? modpackDownloadId : selectedInstance.version,
        ));
        const downloadStarted = $derived(!!downloadItem?.started);
        const downloadProgress = $derived(downloadItem ? getOverallPct(downloadItem) : 0);

        $effect(() => {
                if (instanceNav.section === "market" && !supportsMods) {
                        instanceNav.section = "detalles";
                }
                if (instanceNav.section === "shaderpacks" && !supportsShaders) {
                        instanceNav.section = "detalles";
                }
        });

        function getMarketContentType(section: string) {
                return section === "market"
                        ? "mods"
                        : section === "resources"
                                ? "resourcepacks"
                                : section === "shaderpacks"
                                        ? "shaderpacks"
                                        : null;
        }

        function handlePanelError(error: unknown) {
                instanceNav.section = "detalles";
                showError(t("errors.title"), String(error));
        }

        const marketPanel = lazyComponent(
                () => import("../Market/Market.svelte"),
                () => getMarketContentType(instanceNav.section) !== null,
                handlePanelError,
        );
        const Market = $derived(marketPanel.component);

        const worldsPanel = lazyComponent(
                () => import("../WorldsTab.svelte"),
                () => instanceNav.section === "worlds",
                handlePanelError,
        );
        const WorldsTab = $derived(worldsPanel.component);

        const serversPanel = lazyComponent(
                () => import("../ServersTab.svelte"),
                () => instanceNav.section === "servers",
                handlePanelError,
        );
        const ServersTab = $derived(serversPanel.component);

        const screenshotsPanel = lazyComponent(
                () => import("../ScreenshotsTab.svelte"),
                () => instanceNav.section === "screenshots",
                handlePanelError,
        );
        const ScreenshotsTab = $derived(screenshotsPanel.component);

        // La sección abierta vive en un estado compartido; al salir de la vista se reinicia.
        onDestroy(() => {
                instanceNav.section = "detalles";
        });

        // ---- Modo de mosaicos -------------------------------------------------------
        const RESIZE_DIRS = ["n", "s", "e", "w", "ne", "nw", "se", "sw"] as const;
        const DRAG_THRESHOLD_PX = 4;

        const availableTiles = $derived(
                TILE_IDS.filter((id) =>
                        id === "market"
                                ? supportsMods
                                : id === "shaderpacks"
                                        ? supportsShaders
                                        : true,
                ),
        );
        const visibleTiles = $derived(
                availableTiles
                        .filter((id) => !tileLayout.hidden.includes(id))
                        .sort((a, b) => tileLayout.zOrder.indexOf(a) - tileLayout.zOrder.indexOf(b)),
        );
        const hiddenTiles = $derived(
                availableTiles.filter((id) => tileLayout.hidden.includes(id)),
        );

        function tileLabel(id: TileId): string {
                switch (id) {
                        case "market":
                                return t("instanceView.tabs.market");
                        case "resources":
                                return t("instanceView.tabs.resources");
                        case "shaderpacks":
                                return t("instanceView.tabs.shaderpacks");
                        case "worlds":
                                return t("worlds.title");
                        case "servers":
                                return t("servers.title");
                        case "screenshots":
                                return t("instanceView.tabs.screenshots");
                }
        }

        const tileStyle = $derived(resolveTileStyle(launcherStore.settings));
        const tileSide = $derived(resolveTileSide(launcherStore.settings));
        const glassBlurPx = $derived(
                resolveGlassBlur(launcherStore.settings) * 0.15,
        );
        const textPosSetting = $derived(resolveTextPos(launcherStore.settings));
        /** «Стильный» и «Стекло»: поле слайдеров на всю плитку, подпись поверх него. */
        const plainStyle = $derived(tileStyle !== "standard");
        /** Отступ поля слайдеров от краёв плитки в «plain»-стилях. */
        const PLAIN_PAD = 6;

        /** Mosaico en la mitad derecha: texto alineado a la derecha y el icono tras el texto. */
        function isRightSide(rect: TileRect): boolean {
                return sideForRect(tileSide, rect, GRID_COLS) === "right";
        }

        /**
         * Положение подписи в стиле «Стильный»; автоопределение — нижний угол по центру плитки,
         * как в «Стандартном». Сторона из настроек здесь не учитывается: в «Стильном» она скрыта,
         * и сохранённое значение от «Стандартного» иначе сломало бы определение.
         */
        function captionPos(rect: TileRect): string {
                if (textPosSetting !== "auto") return textPosSetting;
                return sideForRect("auto", rect, GRID_COLS) === "right"
                        ? "bottom-right"
                        : "bottom-left";
        }

        /**
         * Escala del campo de cintas. Los ajustes guardan 80 % como el aspecto original (1.0);
         * valores menores encogen el campo y sus iconos, manteniéndolo pegado al borde exterior de la plantilla. Las capturas tienen
         * su propio sistema y no se tocan.
         */
        const sliderScale = $derived(
                Math.min(
                        1.25,
                        Math.max(0.3 / 0.8, launcherStore.settings.tile_slider_size / 80),
                ),
        );

        /** Граница затухания краёв полосы, % (30–100; 50 = как раньше). */
        const sliderScaleY = $derived(
                launcherStore.settings.tile_slider_layout === "stretch"
                        ? 1
                        : Math.min(1.25, Math.max(0.3 / 0.8, (launcherStore.settings.tile_slider_size_y ?? 80) / 80)),
        );

        const sliderBorder = $derived(
                Math.min(100, Math.max(30, launcherStore.settings.tile_slider_border ?? 50)),
        );

        /** Extensión vertical (0–1): el campo crece hacia arriba y abajo sin cambiar el tamaño de los iconos. */
        const sliderLayout = $derived(launcherStore.settings.tile_slider_layout ?? "size");
        const sliderExtend = $derived(sliderLayout === "stretch" ? 1 : 0);

        let boardEl: HTMLDivElement | undefined = $state();
        let viewportWidth = $state(0);
        let viewportHeight = $state(0);
        const board = $derived(tileBoardGeometry(viewportWidth, viewportHeight));
        let navWidth = $state(0);
        let screenshotCaptionWidth = $state(0);

        /**
         * Niveles fijos de contenido (por ALTURA del mosaico en px): el tamaño del mosaico es libre,
         * pero texto, icono y número de cintas saltan por escalones, sin escalado continuo.
         * El nivel "medio" equivale al aspecto por defecto.
         */
        interface TileTier {
                maxH: number;
                font: number;
                icon: number;
                weight: number;
                pad: number;
                gap: number;
                /** Margen vertical de la zona de cintas. */
                vpad: number;
                /** Máximo de cintas (filas) deslizantes; el contenido decide cuántas se usan de verdad. */
                rows: number;
                /** Rango de tamaño de los iconos de la cinta: se estiran entre ambos según el contenido. */
                minIcon: number;
                maxIcon: number;
                boldIcon: boolean;
        }

        const TIERS: TileTier[] = [
                // pequeño
                { maxH: 56, font: 11.5, icon: 15, weight: 600, pad: 10, gap: 7, vpad: 10, rows: 1, minIcon: 18, maxIcon: 27, boldIcon: false },
                // medio (por defecto)
                { maxH: 100, font: 13.5, icon: 19, weight: 600, pad: 14, gap: 10, vpad: 13, rows: 2, minIcon: 22, maxIcon: 46, boldIcon: false },
                // grande
                { maxH: 160, font: 15.5, icon: 22, weight: 600, pad: 16, gap: 11, vpad: 15, rows: 4, minIcon: 27, maxIcon: 75, boldIcon: false },
                // enorme: texto e icono en negrita
                { maxH: 240, font: 17.5, icon: 25, weight: 700, pad: 18, gap: 12, vpad: 17, rows: 5, minIcon: 31, maxIcon: 100, boldIcon: true },
                // gigante
                { maxH: Infinity, font: 20, icon: 28, weight: 800, pad: 20, gap: 13, vpad: 19, rows: 8, minIcon: 36, maxIcon: 132, boldIcon: true },
        ];

        function tierFor(heightPx: number): TileTier {
                return TIERS.find((tier) => heightPx < tier.maxH) ?? TIERS[TIERS.length - 1];
        }

        /** Las cintas solo se activan si, tras el texto más largo, queda este ancho mínimo. */
        const MIN_SLIDER_W = 96;
        const FADE_GAP = 14;

        const maxLabelChars = $derived(
                Math.max(1, ...visibleTiles.map((id) => tileLabel(id).length)),
        );

        function sliderWidth(tileW: number, tier: TileTier): number {
                const charW = tier.weight >= 800 ? 0.68 : tier.weight >= 700 ? 0.65 : 0.62;
                const textZone =
                        tier.pad +
                        tier.icon +
                        tier.gap +
                        tier.font * charW * maxLabelChars +
                        FADE_GAP;
                const base = Math.floor(tileW - textZone);
                if (base < MIN_SLIDER_W) return 0;
                // Граница: при 50% — как раньше; выше — поле шире (заходит дальше к тексту), ниже — уже.
                const grown = Math.floor((base * sliderBorder) / 50);
                return Math.max(MIN_SLIDER_W, Math.min(grown, Math.floor(tileW - tier.pad)));
        }

        /** Cinta de una sola fila en el menú compacto, entre la etiqueta y el chevron. */
        const NAV_SLIDER_H = 30;
        const navSliderWidth = $derived.by(() => {
                const textEnd = 14 + 20 + 10 + 13.12 * 0.62 * maxLabelChars + 12;
                const width = Math.floor(navWidth - textEnd - 40);
                return width >= MIN_SLIDER_W ? width : 0;
        });

        const RESIZE_CURSORS: Record<string, string> = {
                n: "ns-resize",
                s: "ns-resize",
                e: "ew-resize",
                w: "ew-resize",
                ne: "nesw-resize",
                sw: "nesw-resize",
                nw: "nwse-resize",
                se: "nwse-resize",
        };

        /** Cursores del sistema (visibles siempre) mientras se arrastra o redimensiona. */
        function dragCursor(mode: string): string {
                return mode === "move" ? "move" : (RESIZE_CURSORS[mode] ?? "move");
        }
        let drag = $state<{
                id: TileId;
                mode: string;
                startX: number;
                startY: number;
                origin: TileRect;
                moved: boolean;
        } | null>(null);
        let suppressClick = false;

        function beginDrag(event: PointerEvent, id: TileId, mode: string) {
                if (!editing || event.button !== 0) return;
                suppressClick = false;
                // Захваченная плитка поднимается наверх: иначе её край или тело может оказаться
                // под соседней плиткой, и ручки изменения размера перестанут ловить указатель.
                if (tileLayout.zOrder[tileLayout.zOrder.length - 1] !== id) moveTile(id, "front");
                drag = {
                        id,
                        mode,
                        startX: event.clientX,
                        startY: event.clientY,
                        origin: { ...tileLayout.rects[id] },
                        moved: false,
                };
        }

        function onBoardPointerMove(event: PointerEvent) {
                if (!drag || !boardEl) return;
                const dx = event.clientX - drag.startX;
                const dy = event.clientY - drag.startY;
                if (!drag.moved) {
                        if (Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) return;
                        drag.moved = true;
                        // Captura solo al arrancar el arrastre, para no romper el clic simple.
                        boardEl.setPointerCapture(event.pointerId);
                }
                const box = boardEl.getBoundingClientRect();
                if (!box.width || !box.height) return;
                const dCols = Math.round(dx / (box.width / GRID_COLS));
                const dRows = Math.round(dy / (box.height / GRID_ROWS));
                const o = drag.origin;
                let { x, y, w, h } = o;
                if (drag.mode === "move") {
                        x = o.x + dCols;
                        y = o.y + dRows;
                } else {
                        if (drag.mode.includes("e")) {
                                w = Math.min(o.w + dCols, GRID_COLS - o.x);
                        }
                        if (drag.mode.includes("s")) {
                                h = Math.min(o.h + dRows, GRID_ROWS - o.y);
                        }
                        if (drag.mode.includes("w")) {
                                x = Math.max(0, Math.min(o.x + dCols, o.x + o.w - MIN_TILE_W));
                                w = o.x + o.w - x;
                        }
                        if (drag.mode.includes("n")) {
                                y = Math.max(0, Math.min(o.y + dRows, o.y + o.h - MIN_TILE_H));
                                h = o.y + o.h - y;
                        }
                }
                setTileRect(drag.id, clampRect({ x, y, w, h }));
        }

        function endDrag(event: PointerEvent) {
                if (!drag) return;
                const moved = drag.moved;
                drag = null;
                if (boardEl?.hasPointerCapture(event.pointerId)) {
                        boardEl.releasePointerCapture(event.pointerId);
                }
                // Положение сохраняется только кнопкой «Сохранить» в режиме редактирования.
                if (moved) suppressClick = true;
        }

        function runEditProtectedAction(action: () => void) {
                if (editing) requestEditExit(action);
                else action();
        }

        function openTileContextMenu(event: MouseEvent, id: TileId) {
                event.preventDefault();
                event.stopPropagation();
                tileContextX = event.clientX;
                tileContextY = event.clientY;
                const folderByTile: Record<TileId, string> = {
                        market: "mods",
                        resources: "resourcepacks",
                        shaderpacks: "shaderpacks",
                        worlds: "saves",
                        servers: "",
                        screenshots: "screenshots",
                };
                const items: ContextMenuItem[] = [
                        {
                                label: "Открыть папку",
                                iconName: "instance:folder",
                                action: () => runEditProtectedAction(() => void openInstanceDir(selectedInstance.uuid, folderByTile[id] || undefined)),
                        },
                        {
                                label: id === "market"
                                        ? "Открыть установленные моды"
                                        : id === "resources"
                                                ? "Открыть установленные ресурспаки"
                                                : id === "shaderpacks"
                                                        ? "Открыть установленные шейдеры"
                                                        : `Открыть ${tileLabel(id).toLowerCase()}`,
                                iconName: "ui:check-circle",
                                action: () => runEditProtectedAction(() => {
                                        instanceNav.openInstalled = id === "market" || id === "resources" || id === "shaderpacks";
                                        instanceNav.section = id;
                                }),
                        },
                ];
                if (editing) {
                        items.push({ separator: true, label: "" });
                        const top = tileLayout.zOrder[tileLayout.zOrder.length - 1];
                        const bottom = tileLayout.zOrder[0];
                        items.push(
                                { label: "На передний план", iconName: "ui:layer-front", disabled: id === top, action: () => layerMove(id, "front") },
                                { label: "Выше на один слой", iconName: "ui:layer-up", disabled: id === top, action: () => layerMove(id, 1) },
                                { label: "Ниже на один слой", iconName: "ui:layer-down", disabled: id === bottom, action: () => layerMove(id, -1) },
                                { label: "На задний план", iconName: "ui:layer-back", disabled: id === bottom, action: () => layerMove(id, "back") },
                                { label: "Скрыть плитку", iconName: "ui:eye", action: () => hideTile(id) },
                        );
                        if (tileStyle === "glass") items.push({ label: tileLayout.noBlur.includes(id) ? "Включить прозрачность" : "Отключить прозрачность", iconName: "ui:eye", action: () => setTileNoBlur(id, !tileLayout.noBlur.includes(id)) });
                }
                tileContextItems = items;
                tileContextOpen = true;
        }

        function onTileClick(event: MouseEvent, id: TileId) {
                if (editing) return;
                if (suppressClick) {
                        suppressClick = false;
                        return;
                }
                if ((event.target as HTMLElement).closest(".tile-handle")) return;
                instanceNav.section = id;
        }

        // ---- Режим редактирования плиток ------------------------------------------
        /** Параметры плиток, которые меняются в панели и откатываются при выходе без сохранения. */
        const TILE_SETTING_KEYS = [
                "tile_slider_size",
                "tile_slider_size_y",
                "tile_slider_edge_inset",
                "tile_slider_speed",
                "tile_slider_layout",
                "tile_slider_rows",
                "tile_slider_border",
                "tile_slider_grid_gap",
                "tile_slider_gap",
                "tile_slider_hide_text",
                "screenshot_interval",
                "tile_side",
                "tile_centered_blur",
                "tile_style",
                "tile_text_pos",
        ] as const;

        let editing = $state(false);
        let settingsOpen = $state(false);
        let hiddenOpen = $state(false);
        type InfoWindowMode = "auto" | "hidden" | "pinned";
        let infoWindowMode = $state<InfoWindowMode>("auto");
        let infoWindowExpanded = $state(false);
        let infoWindowModeLoaded = $state(false);
        onMount(() => {
                try {
                        const saved = localStorage.getItem("moonlight.infoWindowMode");
                        if (saved === "auto" || saved === "hidden" || saved === "pinned")
                                infoWindowMode = saved;
                } catch { /* настройки останутся на текущую сессию */ }
                infoWindowModeLoaded = true;
        });
        $effect(() => {
                if (!infoWindowModeLoaded) return;
                try { localStorage.setItem("moonlight.infoWindowMode", infoWindowMode); }
                catch { /* настройки останутся на текущую сессию */ }
        });
        let exitConfirmOpen = $state(false);
        let pendingEditAction: (() => void) | null = null;
        let barEl: HTMLDivElement | undefined = $state();
        let snapshot: {
                rects: Record<TileId, TileRect>;
                settings: Record<string, unknown>;
                motion: typeof appearance.sliderMotion;
                quickActionsDisabled: boolean;
                panelsLocked: boolean;
                zOrder: TileId[];
                hidden: TileId[];
        } | null = null;

        function startEdit() {
                const rects = {} as Record<TileId, TileRect>;
                for (const id of TILE_IDS) rects[id] = { ...tileLayout.rects[id] };
                const settings: Record<string, unknown> = {};
                const current = launcherStore.settings as unknown as Record<string, unknown>;
                for (const key of TILE_SETTING_KEYS) settings[key] = current[key];
                snapshot = {
                        rects,
                        settings,
                        motion: appearance.sliderMotion,
                        quickActionsDisabled: appearance.quickActionsDisabled,
                        panelsLocked: panelLayout.locked,
                        zOrder: [...tileLayout.zOrder],
                        hidden: [...tileLayout.hidden],
                };
                hiddenOpen = false;
                editUuid = selectedInstance.uuid;
                editing = true;
        }

        /** Выход без сохранения: плитки и настройки возвращаются к состоянию до редактирования. */
        /** Сравнивает текущее состояние с исходным снимком, включая изменения, которые пользователь затем отменил. */
        function hasEditChanges(): boolean {
                if (!snapshot) return false;
                const currentSettings = launcherStore.settings as unknown as Record<string, unknown>;
                for (const key of TILE_SETTING_KEYS) {
                        if (!Object.is(currentSettings[key], snapshot.settings[key])) return true;
                }
                if (appearance.sliderMotion !== snapshot.motion) return true;
                if (appearance.quickActionsDisabled !== snapshot.quickActionsDisabled) return true;
                if (panelLayout.locked !== snapshot.panelsLocked) return true;
                if (JSON.stringify(tileLayout.zOrder) !== JSON.stringify(snapshot.zOrder)) return true;
                if (JSON.stringify(tileLayout.hidden) !== JSON.stringify(snapshot.hidden)) return true;
                for (const id of TILE_IDS) {
                        if (JSON.stringify(tileLayout.rects[id]) !== JSON.stringify(snapshot.rects[id])) return true;
                }
                return false;
        }

        function requestEditExit(action?: unknown) {
                pendingEditAction = typeof action === "function" ? (action as () => void) : null;
                if (hasEditChanges()) exitConfirmOpen = true;
                else {
                        discardEdit();
                        const next = pendingEditAction;
                        pendingEditAction = null;
                        next?.();
                }
        }

        function discardEdit() {
                if (snapshot) {
                        for (const id of TILE_IDS) tileLayout.rects[id] = { ...snapshot.rects[id] };
                        const target = launcherStore.settings as unknown as Record<string, unknown>;
                        for (const key of TILE_SETTING_KEYS) target[key] = snapshot.settings[key];
                        setSliderMotion(snapshot.motion);
                        setQuickActionsDisabled(snapshot.quickActionsDisabled);
                        setPanelOrientationLocked(snapshot.panelsLocked);
                        tileLayout.zOrder = [...snapshot.zOrder];
                        tileLayout.hidden = [...snapshot.hidden];
                }
                snapshot = null;
                editing = false;
                exitConfirmOpen = false;
                settingsOpen = false;
                hiddenOpen = false;
                drag = null;
                const next = pendingEditAction;
                pendingEditAction = null;
                next?.();
        }

        async function confirmSave() {
                commitTileLayout();
                try {
                        await saveSettings();
                        snapshot = null;
                        editing = false;
                        exitConfirmOpen = false;
                        settingsOpen = false;
                        hiddenOpen = false;
                        commitTileLayout();
                        const next = pendingEditAction;
                        pendingEditAction = null;
                        next?.();
                } catch (error) {
                        showError(t("errors.title"), String(error));
                }
        }

        function onWindowPointerDown(event: PointerEvent) {
                const target = event.target as Node;
                if (
                        settingsOpen &&
                        !barEl?.contains(target) &&
                        !(target instanceof Element && target.closest(".tile-settings-pop"))
                ) {
                        settingsOpen = false;
                }
                if (exitConfirmOpen && !barEl?.contains(target)) {
                        exitConfirmOpen = false;
                        pendingEditAction = null;
                }
                if (
                        hiddenOpen &&
                        !(target instanceof Element && target.closest(".tile-hidden-wrap"))
                ) {
                        hiddenOpen = false;
                }
        }

        /** Меню скрытых закрывается само, когда скрытых элементов не осталось. */
        $effect(() => {
                if (hiddenOpen && hiddenTiles.length === 0) hiddenOpen = false;
        });

        function onWindowKeyDown(event: KeyboardEvent) {
                if (event.key !== "Escape" || event.defaultPrevented || event.repeat || hasOpenModal() || tileContextOpen) return;
                if (exitConfirmOpen) {
                        event.preventDefault();
                        exitConfirmOpen = false;
                        pendingEditAction = null;
                } else if (settingsOpen || hiddenOpen) {
                        event.preventDefault();
                        settingsOpen = false;
                        hiddenOpen = false;
                } else if (editing) {
                        event.preventDefault();
                        requestEditExit();
                } else if (instanceNav.section !== "detalles") {
                        // Глобальные обработчики вложенных списков должны получить Esc первыми.
                        const section = instanceNav.section;
                        const uuid = selectedInstance.uuid;
                        window.setTimeout(() => {
                                if (!event.defaultPrevented && !hasOpenModal() && tabContentEl?.isConnected && selectedInstance.uuid === uuid && instanceNav.section === section) {
                                        instanceNav.section = "detalles";
                                }
                        });
                }
        }

        /** Кнопки порядка слоёв на каждой плитке: сама плитка становится активной. */
        function layerMove(id: TileId, direction: -1 | 1 | "front" | "back") {
                moveTile(id, direction);
        }

        function hideTile(id: TileId) {
                setTileHidden(id, true);
        }

        // Уход из «Деталей» или смена инстанса во время редактирования = выход без сохранения.
        let editUuid = "";
        $effect(() => {
                instanceNav.editActionGuard = editing
                        ? (action) => { requestEditExit(action); return true; }
                        : null;
                return () => {
                        if (instanceNav.editActionGuard) instanceNav.editActionGuard = null;
                };
        });
        let lastGuardedSection = instanceNav.section;
        $effect(() => {
                const section = instanceNav.section;
                if (editing && section !== lastGuardedSection) {
                        lastGuardedSection = section;
                        instanceNav.section = "detalles";
                        requestEditExit(() => { instanceNav.section = section; });
                } else {
                        lastGuardedSection = section;
                }
        });
        $effect(() => {
                if (!tileLayout.enabled && editing) discardEdit();
        });
        onDestroy(() => {
                if (editing) discardEdit();
        });

        // ---- Перемещение игровой панели --------------------------------------
        // Игровое меню можно перетаскивать только в четыре верхние/нижние зоны.
        const GAME_ZONES: GamePanelDock[] = [
                "top-start",
                "top-end",
                "bottom-start",
                "bottom-end",
        ];
        let gamePanelDrag = $state<{ dock: GamePanelDock } | null>(null);
        function startGamePanelDrag(event: PointerEvent) {
                if (panelLayout.locked || event.button !== 0 || gamePanelDrag) return;
                const target = event.target;
                if (
                        target instanceof Element &&
                        target.closest(
                                'button, a, input, select, textarea, [contenteditable="true"], [data-no-drag], [data-no-panel-drag], [role="button"], [role="menu"], [role="menuitem"]',
                        )
                ) return;
                const startX = event.clientX;
                const startY = event.clientY;
                let moved = false;
                let nextDock: GamePanelDock = panelLayout.gameDock;
                const getDock = (x: number, y: number): GamePanelDock => {
                        const rect = rootEl?.getBoundingClientRect();
                        if (!rect || !rect.width || !rect.height) return panelLayout.gameDock;
                        // Игровая панель размещается только сверху или снизу окна.
                        const dists: [string, number][] = [
                                ["top", (y - rect.top) / rect.height],
                                ["bottom", (rect.bottom - y) / rect.height],
                        ];
                        dists.sort((a, b) => a[1] - b[1]);
                        const edge = dists[0][0];
                        const align: "start" | "end" =
                                x - rect.left < rect.right - x ? "start" : "end";
                        return `${edge}-${align}` as GamePanelDock;
                };
                const cleanup = () => {
                        window.removeEventListener("pointermove", move);
                        window.removeEventListener("pointerup", up);
                        window.removeEventListener("pointercancel", cancel);
                        window.removeEventListener("keydown", key);
                        gamePanelDrag = null;
                        document.documentElement.classList.remove("panel-dragging");
                };
                const move = (e: PointerEvent) => {
                        if (!moved && Math.hypot(e.clientX - startX, e.clientY - startY) < 8) return;
                        moved = true;
                        e.preventDefault();
                        nextDock = getDock(e.clientX, e.clientY);
                        gamePanelDrag = { dock: nextDock };
                        document.documentElement.classList.add("panel-dragging");
                };
                const up = () => {
                        cleanup();
                        if (moved) setGamePanelDock(nextDock);
                };
                const cancel = () => cleanup();
                const key = (e: KeyboardEvent) => { if (e.key === "Escape") cleanup(); };
                window.addEventListener("pointermove", move);
                window.addEventListener("pointerup", up);
                window.addEventListener("pointercancel", cancel);
                window.addEventListener("keydown", key);
        }
        let rootEl: HTMLDivElement | undefined = $state();

        function cycleInfoWindowMode() {
                const modes: InfoWindowMode[] = ["auto", "hidden", "pinned"];
                infoWindowMode = modes[(modes.indexOf(infoWindowMode) + 1) % modes.length];
                infoWindowExpanded = false;
        }

        async function handlePlay() {
                if (downloadKind || bannerState === "Starting") return;
                const { version } = selectedInstance;
                if (needsDownload && bannerState !== "Started") enqueueingVersion = version;
                try {
                        await playInstance(selectedInstance, needsDownload);
                } finally {
                        if (enqueueingVersion === version) enqueueingVersion = null;
                }
        }
</script>

{#snippet tileIcon(id: TileId, size: number)}
        <span class="tile-icon" style:width="{size}px" style:height="{size}px">
                {#if id === "market"}
                        <Icon name="instance:market" fallbackName="instance:grid" {size} />
                {:else if id === "resources"}
                        <ResourcesIcon {size} />
                {:else if id === "shaderpacks"}
                        <ShadersIcon {size} />
                {:else if id === "worlds"}
                        <WorldIcon {size} />
                {:else if id === "servers"}
                        <Icon
                                name="instance:multiplayer"
                                fallbackName="instance:servers"
                                {size}
                        />
                {:else}
                        <ImageIcon {size} />
                {/if}
        </span>
{/snippet}

{#snippet navSlider(id: TileId)}
        {#if navSliderWidth > 0}
                {@const navW =
                        id === "screenshots"
                                ? navSliderWidth
                                : Math.min(
                                                Math.round(navSliderWidth * sliderScale),
                                                navSliderWidth + FADE_GAP,
                                        )}
                {@const navH = id === "screenshots" ? NAV_SLIDER_H : NAV_SLIDER_H * sliderScaleY}
                <span
                        class="nav-slider"
                        style:width="{navW}px"
                        style:height="{navH}px"
                        style:top="{5 + (NAV_SLIDER_H - navH) / 2}px"
                        style:right="{40 + (navW < navSliderWidth ? (navSliderWidth - navW) / 2 : 0)}px"
                        aria-hidden="true"
                >
                        <TileSlider
                                uuid={selectedInstance.uuid}
                                name={selectedInstance.name}
                                {id}
                                width={navW}
                                height={navH}
                                maxRows={1}
                                maxIcon={NAV_SLIDER_H}
                                minIcon={18}
                                side="right"
                                fadePx={28}
                                fadeBoth
                                scale={id === "screenshots" ? 1 : sliderScale}
                        />
                </span>
        {/if}
{/snippet}

<svelte:window
        onfocus={refreshIntegrity}
        onpointerdown={onWindowPointerDown}
        onkeydown={onWindowKeyDown}
/>

<div class="instance-view game-edge-{gamePanelEdge(panelLayout.gameDock)}" bind:this={rootEl}>
        <div
                class="game-panel-wrap"
                class:dragging={!!gamePanelDrag}
                role="group"
                aria-label={t("settings.launcher.appearance.gameDockTitle")}
                onpointerdown={startGamePanelDrag}
        >
        <InstanceHeader
                instance={selectedInstance}
                {bannerState}
                {launchPending}
                {downloadKind}
                {needsDownload}
                {downloadStarted}
                {downloadProgress}
                bind:activeSection={instanceNav.section}
                onPlay={handlePlay}
                {onopenlibrary}
                onStop={() => killInst(selectedInstance.uuid)}
                dock={gamePanelEdge(panelLayout.gameDock)}
                mirrored={gamePanelMirrored(panelLayout.gameDock)}
        >
                {#snippet modpack()}
                        {#key selectedInstance.uuid}
                                <InstanceModpack
                                        id={selectedInstance.uuid}
                                        busy={bannerState === "Started" ||
                                                bannerState === "Starting" ||
                                                !!downloadKind}
                                />
                        {/key}
                {/snippet}
        </InstanceHeader>
        </div>
        {#if gamePanelDrag}
                {#each GAME_ZONES as zone (zone)}
                        <div
                                class="game-dock-preview {zone}"
                                class:hover={gamePanelDrag.dock === zone}
                                class:current={zone === panelLayout.gameDock}
                                class:compact={instanceNav.section !== "detalles"}
                                aria-hidden="true"
                        ><span class="dock-bar"></span></div>
                {/each}
        {/if}

        <div
                class="tab-content"
                style:--section-duration={`${animDuration(220)}ms`}
                bind:this={tabContentEl}
        >
                        <div class="details-section" class:inactive={!showingDetails} inert={!showingDetails}>
                                {#if tileLayout.enabled}
                                        <div class="tile-viewport" bind:clientWidth={viewportWidth} bind:clientHeight={viewportHeight}>
                                        <div
                                                class="tile-board"
                                                role="presentation"
                                                class:dragging={drag?.moved}
                                                class:editing
                                                bind:this={boardEl}
                                                style:width="{board?.width ?? 0}px"
                                                style:height="{board?.height ?? 0}px"
                                                style:transform={board && board.scale < 1 ? `scale(${board.scale})` : undefined}
                                                style:visibility={board ? undefined : "hidden"}
                                                style:cursor={drag?.moved ? dragCursor(drag.mode) : undefined}
                                                onpointermove={onBoardPointerMove}
                                                onpointerup={endDrag}
                                                onpointercancel={endDrag}
                                        >
                                                {#each visibleTiles as id (id)}
                                                        {@const rect = tileLayout.rects[id]}
                                                        {@const tw = (rect.w / GRID_COLS) * (board?.width ?? 0)}
                                                        {@const th = (rect.h / GRID_ROWS) * (board?.height ?? 0)}
                                                        {@const tier = tierFor(th)}
                                                        {@const right = isRightSide(rect)}
                                                        {@const fullTileGallery = tileStyle === "glass" && id === "screenshots"}
                                                        {@const vpad = fullTileGallery ? 0 : plainStyle ? PLAIN_PAD : tier.vpad}
                                                        {@const sw = plainStyle
                                                                ? tw - 2 >= 48
                                                                        ? Math.floor(tw - 2)
                                                                        : 0
                                                                : sliderWidth(tw, tier)}
                                                        {@const scaled = id !== "screenshots"}
                                                        {@const fullH = th - 2 - 2 * vpad}
                                                        {@const slW = scaled
                                                                ? Math.min(
                                                                                Math.round(sw * sliderScale),
                                                                                plainStyle ? sw : sw + FADE_GAP,
                                                                        )
                                                                : sw}
                                                        {@const edge = 0}
                                                        {@const baseH = Math.min(
                                                                Math.round(fullH * sliderScaleY),
                                                                th - 2,
                                                        )}
                                                        {@const slH = scaled
                                                                ? Math.round(
                                                                                baseH +
                                                                                        Math.max(0, th - 2 - baseH) *
                                                                                                sliderExtend,
                                                                        )
                                                                : fullH}
                                                        {@const rowsBoost = scaled
                                                                ? Math.max(1, slH / Math.max(1, baseH))
                                                                : 1}
                                                        {@const cpos = captionPos(rect)}
                                                        {@const capRev = tileStyle === "glass"
                                                                ? right
                                                                : cpos.endsWith("right")}
                                                        {@const sliderSide = tileStyle === "stylish"
                                                                ? cpos.endsWith("center")
                                                                        ? "center"
                                                                        : cpos.endsWith("left")
                                                                                ? "right"
                                                                                : cpos.endsWith("right")
                                                                                        ? "left"
                                                                                        : right ? "left" : "right"
                                                                : right ? "left" : "right"}
                                                        {@const sliderVerticalAlign = tileStyle === "stylish"
                                                                ? cpos.startsWith("top")
                                                                        ? "end"
                                                                        : cpos.startsWith("bottom")
                                                                                ? "start"
                                                                                : "center"
                                                                : "center"}
                                                        <button
                                                                type="button"
                                                                class="tile"
                                                                class:right={!plainStyle && right}
                                                                class:plain={plainStyle}
                                                                class:editing
                                                                class:bold-icon={tier.boldIcon}
                                                                class:active={drag?.id === id && drag.moved}
                                                                style:z-index={tileLayout.zOrder.indexOf(id) + 1}
                                                                style:left="{(rect.x / GRID_COLS) * 100}%"
                                                                style:top="{(rect.y / GRID_ROWS) * 100}%"
                                                                style:width="{(rect.w / GRID_COLS) * 100}%"
                                                                style:height="{(rect.h / GRID_ROWS) * 100}%"
                                                                style:font-size="{tier.font}px"
                                                                style:font-weight={tier.weight}
                                                                style:gap="{tier.gap}px"
                                                                style:padding-left={plainStyle
                                                                        ? "0px"
                                                                        : `${right && sw ? sw : tier.pad}px`}
                                                                style:padding-right={plainStyle
                                                                        ? "0px"
                                                                        : `${!right && sw ? sw : tier.pad}px`}
                                                                onpointerdown={(event) => beginDrag(event, id, "move")}
                                                                onclick={(event) => onTileClick(event, id)}
                                                                oncontextmenu={(event) => openTileContextMenu(event, id)}
                                                        >
                                                                {#if !plainStyle}
                                                                        {@render tileIcon(id, tier.icon)}
                                                                        <span class="tile-label">{tileLabel(id)}</span>
                                                                {/if}
                                                        {#if sw > 0 && slW > 0 && slH > 0}
                                                                        <span class="tile-clip" aria-hidden="true">
                                                                                <span
                                                                                        class="tile-slider"
                                                                                        style:width={fullTileGallery ? "100%" : `${slW}px`}
                                                                                        style:top={fullTileGallery ? "0px" : plainStyle && sliderVerticalAlign === "start"
                                                                                                ? `${vpad}px`
                                                                                                : plainStyle && sliderVerticalAlign === "end"
                                                                                                        ? `${vpad + fullH - slH}px`
                                                                                                        : `${vpad + (fullH - slH) / 2}px`}
                                                                                        style:bottom={fullTileGallery ? "0px" : plainStyle && sliderVerticalAlign === "start"
                                                                                                ? `${vpad + fullH - slH}px`
                                                                                                : plainStyle && sliderVerticalAlign === "end"
                                                                                                        ? `${vpad}px`
                                                                                                        : `${vpad + (fullH - slH) / 2}px`}
                                                                                        style:left={plainStyle
                                                                                                ? sliderSide === "right" ? `${sw - slW}px` : "0px"
                                                                                                : right
                                                                                                        ? `${edge}px`
                                                                                                        : undefined}
                                                                                        style:right={plainStyle
                                                                                                ? undefined
                                                                                                : right
                                                                                                        ? undefined
                                                                                                        : `${edge}px`}
                                                                                >
                                                                                        <TileSlider
                                                                                                uuid={selectedInstance.uuid}
                                                                                                name={selectedInstance.name}
                                                                                                {id}
                                                                                                width={slW}
                                                                                                height={slH}
                                                                                                maxRows={sliderLayout === "custom"
                                                                                                        ? Math.min(10, Math.max(1, launcherStore.settings.tile_slider_rows ?? 2))
                                                                                                        : Math.max(
                                                                                                                tier.rows,
                                                                                                                Math.round(tier.rows * rowsBoost),
                                                                                                        )}
                                                                                                forceRows={sliderLayout === "custom"}
                                                                                                maxIcon={tier.maxIcon}
                                                                                                minIcon={tier.minIcon}
                                                                                                inset={vpad}
                                                                                                galleryCaptionWidth={fullTileGallery ? screenshotCaptionWidth : undefined}
                                                                                                scale={scaled ? sliderScale : 1}
                                                                                                stretch={sliderLayout === "stretch"}
                                                                                                edgeInset={resolveTileSliderEdgeInset(launcherStore.settings.tile_slider_edge_inset)}
                                                                                                plain={plainStyle}
                                                                                                side={sliderSide}
                                                                                                verticalAlign={sliderVerticalAlign}
                                                                                        />
                                                                                </span>
                                                                        </span>
                                                                {/if}
                                                                {#if plainStyle}
                                                                        <span class="tile-caption-clip" aria-hidden="true">
                                                                                <span
                                                                                        class="tile-caption"
                                                                                        class:glass={tileStyle === "glass"}
                                                                                        class:gallery-caption={fullTileGallery}
                                                                                        class:no-blur={tileLayout.noBlur.includes(id) || launcherStore.settings.disable_blur_effects}
                                                                                        class:solid={plainStyle}
                                                                                        class:stylish={tileStyle === "stylish"}
                                                                                        class:rev={capRev}
                                                                                        data-pos={tileStyle === "glass" ? (right ? "center-right" : "center-left") : cpos}
                                                                                        style:--cap-pad="{tier.pad}px"
                                                                                        style:--glass-width="{sliderBorder}%"
                                                                                        style:--cap-fade="{Math.round(tier.pad * 1.8)}px"
                                                                                        style:--caption-blur="{glassBlurPx}px"
                                                                                        style:gap="{tier.gap}px"
                                                                                        {@attach (node) => {
                                                                                                if (!fullTileGallery) return;
                                                                                                const measure = () => { screenshotCaptionWidth = node.clientWidth; };
                                                                                                measure();
                                                                                                const observer = new ResizeObserver(measure);
                                                                                                observer.observe(node);
                                                                                                return () => observer.disconnect();
                                                                                        }}
                                                                                >
                                                                                        {@render tileIcon(id, tier.icon)}
                                                                                        <span class="tile-label">{tileLabel(id)}</span>
                                                                                </span>
                                                                        </span>
                                                                {/if}
                                                                {#if editing}
                                                                        {#each RESIZE_DIRS as dir (dir)}
                                                                                <span
                                                                                        class="tile-handle handle-{dir}"
                                                                                        aria-hidden="true"
                                                                                        onpointerdown={(event) => {
                                                                                                event.stopPropagation();
                                                                                                beginDrag(event, id, dir);
                                                                                        }}
                                                                                ></span>
                                                                        {/each}
                                                                {/if}
                                                        </button>
                                                {/each}
                                        </div>
                                        </div>
                                        <div class="tile-bar" bind:this={barEl}>
                                                {#if !editing}
                                                        <button
                                                                type="button"
                                                                class="tile-btn"
                                                                title={t("instanceView.tilesExit")}
                                                                aria-label={t("instanceView.tilesExit")}
                                                                onclick={() => setTileMode(false)}
                                                        >
                                                                <ChevronGlyph size={16} rotate={90} />
                                                        </button>
                                                        <button
                                                                type="button"
                                                                class="tile-btn"
                                                                title={t("instanceView.tilesEdit")}
                                                                aria-label={t("instanceView.tilesEdit")}
                                                                onclick={startEdit}
                                                        >
                                                                <Icon name="nav:pencil" size={16} />
                                                        </button>
                                                {:else}
                                                        <button
                                                                type="button"
                                                                class="tile-btn"
                                                                title={t("instanceView.tilesEditExit")}
                                                                aria-label={t("instanceView.tilesEditExit")}
                                                                onclick={() => requestEditExit()}
                                                        >
                                                                <Icon name="ui:logout" size={16} />
                                                        </button>
                                                        {#if exitConfirmOpen}
                                                                <div class="tile-confirm exit-confirm" role="dialog" aria-label={t("instanceView.tilesSaveConfirm")}>
                                                                        <span class="tile-confirm-text">{t("instanceView.tilesSaveConfirm")}</span>
                                                                        <div class="tile-confirm-actions">
                                                                                <button type="button" class="tile-confirm-action primary" onclick={() => { exitConfirmOpen = false; confirmSave(); }}>
                                                                                        <Icon name="ui:check" size={14} />
                                                                                        <span>{t("instanceView.tilesSaveYes")}</span>
                                                                                </button>
                                                                                <button type="button" class="tile-confirm-action danger" onclick={() => { exitConfirmOpen = false; discardEdit(); }}>
                                                                                        <Icon name="ui:close" size={14} />
                                                                                        <span>{t("instanceView.tilesEditExit")}</span>
                                                                                </button>
                                                                        </div>
                                                                        <button type="button" class="tile-confirm-cancel" onclick={() => { exitConfirmOpen = false; pendingEditAction = null; }}>{t("common.cancel")}</button>
                                                                </div>
                                                        {/if}
                                                        
                                                        <button
                                                                type="button"
                                                                class="tile-btn"
                                                                class:open={settingsOpen}
                                                                title={t("instanceView.tilesSettings")}
                                                                aria-label={t("instanceView.tilesSettings")}
                                                                aria-expanded={settingsOpen}
                                                                onclick={() => { settingsOpen = !settingsOpen; if (settingsOpen) hiddenOpen = false; }}
                                                        >
                                                                <Icon name="nav:sliders" size={16} />
                                                        </button>
                                                        {#if hiddenTiles.length > 0}
                                                                <span class="tile-hidden-wrap">
                                                                        <button type="button" class="tile-btn" class:open={hiddenOpen} title={t("instanceView.tileHidden")} aria-label={t("instanceView.tileHidden")} aria-expanded={hiddenOpen} onclick={() => { hiddenOpen = !hiddenOpen; if (hiddenOpen) settingsOpen = false; }}>
                                                                                <Icon name="ui:eye" size={16} />
                                                                        </button>
                                                                        {#if editing && hiddenOpen}
                                                                                <div class="tile-hidden-pop" transition:fly={{ y: 8, duration: 140 }}>
                                                                                        <strong>{t("instanceView.tileHidden")}</strong>
                                                                                        {#each hiddenTiles as id (id)}
                                                                                                <button type="button" class="tile-hidden-item" onclick={() => { setTileHidden(id, false); }}>
                                                                                                        <span>{tileLabel(id)}</span><Icon name="ui:eye" size={14} />
                                                                                                </button>
                                                                                        {/each}
                                                                                </div>
                                                                        {/if}
                                                                </span>
                                                        {/if}

                                                {/if}
                                        </div>
                                        {#if editing && settingsOpen}
                                                <div
                                                        class="tile-settings-pop"
                                                        transition:fly={{ y: 10, duration: 160 }}
                                                >
                                                        <TileSettingsPanel />
                                                </div>
                                        {/if}
                                {:else}
                                <div
                                        class="nav-card"
                                        class:info-hidden={infoWindowMode === "hidden"}
                                        class:info-pinned={infoWindowMode === "pinned"}
                                        class:info-expanded={infoWindowExpanded}
                                >
                                        <div
                                                class="nav-card-header"
                                                role="group"
                                                aria-label={t("instanceView.tabs.details")}
                                        >
                                                {#if infoWindowMode === "hidden"}
                                                        <button
                                                                type="button"
                                                                class="nav-card-header-toggle"
                                                                aria-label={t("instanceView.infoWindowToggle")}
                                                                aria-expanded={infoWindowExpanded}
                                                                onclick={() => (infoWindowExpanded = !infoWindowExpanded)}
                                                        ></button>
                                                {/if}
                                                <button
                                                        type="button"
                                                        class="nav-card-title"
                                                        aria-expanded={infoWindowMode === "pinned" || (infoWindowMode === "hidden" && infoWindowExpanded)}
                                                        title={t("instanceView.infoWindowToggle")}
                                                        onclick={() => {
                                                                if (infoWindowMode === "hidden") infoWindowExpanded = !infoWindowExpanded;
                                                        }}
                                                >{t("instanceView.tabs.details")}</button>
                                                <button
                                                        type="button"
                                                        class="info-mode-toggle"
                                                        title={t(`instanceView.infoWindowModes.${infoWindowMode}`)}
                                                        aria-label={t("instanceView.infoWindowModeNext")}
                                                        onclick={cycleInfoWindowMode}
                                                >
                                                        {#if infoWindowMode === "pinned"}
                                                                <Icon name="ui:pin" size={14} />
                                                        {:else if infoWindowMode === "hidden"}
                                                                <Icon name="ui:eye" size={14} />
                                                        {:else}
                                                                <Icon name="ui:chevron-down" size={14} />
                                                        {/if}
                                                </button>
                                                <button
                                                        type="button"
                                                        class="tile-toggle"
                                                        title={t("instanceView.tilesEnable")}
                                                        aria-label={t("instanceView.tilesEnable")}
                                                        onclick={() => setTileMode(true)}
                                                >
                                                        <TilesIcon size={14} />
                                                </button>
                                        </div>
                                        <div class="nav-items" bind:clientWidth={navWidth}>
                                                {#if supportsMods}
                                                        <button
                                                                type="button"
                                                                class="nav-item priority"
                                                                onclick={() => (instanceNav.section = "market")}
                                                        >
                                                                <span class="nav-icon"
                                                                        ><Icon
                                                                                name="instance:market"
                                                                                fallbackName="instance:grid"
                                                                                size={18}
                                                                        /></span
                                                                >
                                                                <span class="nav-label"
                                                                        >{t("instanceView.tabs.market")}</span
                                                                >
                                                                {@render navSlider("market")}
                                                                <span class="nav-chevron"
                                                                        ><ChevronRightIcon size={14} /></span
                                                                >
                                                        </button>
                                                {/if}
                                                <button
                                                        type="button"
                                                        class="nav-item priority"
                                                        onclick={() => (instanceNav.section = "resources")}
                                                >
                                                        <span class="nav-icon"
                                                                ><ResourcesIcon size={18} /></span
                                                        >
                                                        <span class="nav-label"
                                                                >{t("instanceView.tabs.resources")}</span
                                                        >
                                                        {@render navSlider("resources")}
                                                        <span class="nav-chevron"
                                                                ><ChevronRightIcon size={14} /></span
                                                        >
                                                </button>
                                                {#if supportsShaders}
                                                        <button
                                                                type="button"
                                                                class="nav-item secondary"
                                                                onclick={() => (instanceNav.section = "shaderpacks")}
                                                        >
                                                                <span class="nav-icon"
                                                                        ><ShadersIcon size={18} /></span
                                                                >
                                                                <span class="nav-label"
                                                                        >{t("instanceView.tabs.shaderpacks")}</span
                                                                >
                                                                {@render navSlider("shaderpacks")}
                                                                <span class="nav-chevron"
                                                                        ><ChevronRightIcon size={14} /></span
                                                                >
                                                        </button>
                                                {/if}
                                                <div class="nav-divider" role="separator"></div>
                                                <button
                                                        type="button"
                                                        class="nav-item"
                                                        onclick={() => (instanceNav.section = "worlds")}
                                                >
                                                        <span class="nav-icon"><WorldIcon size={18} /></span
                                                        >
                                                        <span class="nav-label">{t("worlds.title")}</span>
                                                        {@render navSlider("worlds")}
                                                        <span class="nav-chevron"
                                                                ><ChevronRightIcon size={14} /></span
                                                        >
                                                </button>
                                                <button
                                                        type="button"
                                                        class="nav-item"
                                                        onclick={() => (instanceNav.section = "servers")}
                                                >
                                                        <span class="nav-icon"
                                                                ><Icon
                                                                        name="instance:multiplayer"
                                                                        fallbackName="instance:servers"
                                                                        size={18}
                                                                /></span
                                                        >
                                                        <span class="nav-label">{t("servers.title")}</span>
                                                        {@render navSlider("servers")}
                                                        <span class="nav-chevron"
                                                                ><ChevronRightIcon size={14} /></span
                                                        >
                                                </button>
                                                <button
                                                        type="button"
                                                        class="nav-item"
                                                        onclick={() => (instanceNav.section = "screenshots")}
                                                >
                                                        <span class="nav-icon"><ImageIcon size={18} /></span
                                                        >
                                                        <span class="nav-label"
                                                                >{t("instanceView.tabs.screenshots")}</span
                                                        >
                                                        {@render navSlider("screenshots")}
                                                        <span class="nav-chevron"
                                                                ><ChevronRightIcon size={14} /></span
                                                        >
                                                </button>
                                        </div>
                                </div>
                                {/if}
                        </div>
                        {#each showingDetails ? [] : [instanceNav.section] as section (section)}
                        {@const marketContentType = getMarketContentType(section)}
                        <div
                                class="subview-section"
                                class:gallery={section === "screenshots"}
                                transition:fly={{ y: 8, duration: animDuration(220), easing: cubicOut }}
                        >
                                <div class="subview-body">
                                {#if marketContentType}
                                        {#key `${selectedInstance.uuid}:${marketContentType}`}
                                                {#if Market}
                                                        <Market
                                                                instance={selectedInstance}
                                                                contentType={marketContentType}
                                                        />
                                                {:else}
                                                        <div class="fallback-panel">
                                                                <MarketCatalogSkeleton />
                                                        </div>
                                                {/if}
                                        {/key}
                                {:else if section === "worlds"}
                                        {#key selectedInstance.uuid}
                                                {#if WorldsTab}<WorldsTab
                                                                instance={selectedInstance}
                                                        />{:else}
                                                        <div class="fallback-panel">
                                                                <ListSkeleton
                                                                        rows={6}
                                                                        label={t("worlds.loading")}
                                                                        actionCount={8}
                                                                />
                                                        </div>
                                                {/if}
                                        {/key}
                                {:else if section === "servers"}
                                        {#key selectedInstance.uuid}
                                                {#if ServersTab}<ServersTab
                                                                instance={selectedInstance}
                                                        />{:else}
                                                        <div class="fallback-panel">
                                                                <ListSkeleton
                                                                        rows={6}
                                                                        label={t("servers.loading")}
                                                                        actionCount={5}
                                                                        wideFirstAction
                                                                />
                                                        </div>
                                                {/if}
                                        {/key}
                                {:else if section === "screenshots"}
                                        {#if ScreenshotsTab}
                                                <ScreenshotsTab instance={selectedInstance} />
                                        {:else}
                                                <div class="fallback-panel">
                                                        <PanelSkeleton
                                                                variant="tiles"
                                                                rows={9}
                                                                label={t("common.loading")}
                                                        />
                                                </div>
                                        {/if}
                                {/if}
                                </div>
                        </div>
                        {/each}
        </div>
</div>

{#if tileContextOpen}
        <ContextMenu bind:open={tileContextOpen} x={tileContextX} y={tileContextY} items={tileContextItems} />
{/if}

<style>
        .instance-view {
                position: relative;
                display: flex;
                flex-direction: column;
                flex: 1;
                min-height: 0;
        }

        .game-panel-wrap { display: contents; }
        :global(.game-panel-wrap.dragging .instance-header) { opacity: 0.72; }
        :global(.game-panel-wrap .instance-header) { cursor: default; }
        :global(.game-panel-wrap.dragging .instance-header) { cursor: default; }
        /* Превью — тот же вид, что выбор в настройках: рамка и полоска внутри. */
        .game-dock-preview {
                position: absolute;
                z-index: 30;
                box-sizing: border-box;
                pointer-events: none;
                border: 1px solid var(--border-color);
                border-radius: 4px;
                background: var(--bg-card);
                opacity: 0.35;
                transition: all 0.16s ease;
                animation: game-dock-preview-in 0.14s ease-out;
        }
        .game-dock-preview .dock-bar {
                position: absolute;
                inset: 6px;
                border-radius: 3px;
                background: var(--border-color);
                transition: background 0.15s;
        }
        .game-dock-preview.current:not(.hover) { opacity: 0.75; border-color: var(--accent); }
        .game-dock-preview.current .dock-bar,
        .game-dock-preview.hover .dock-bar { background: var(--accent); }
        .game-dock-preview.hover {
                opacity: 1;
                border-color: var(--accent);
                background: color-mix(in srgb, var(--accent) 14%, transparent);
        }
        @keyframes game-dock-preview-in { from { opacity: 0; } to { opacity: 1; } }
        .game-dock-preview.top-start { top: 0; left: 0; right: 50%; height: 180px; }
        .game-dock-preview.top-end { top: 0; left: 50%; right: 0; height: 180px; }
        .game-dock-preview.bottom-start { bottom: 0; left: 0; right: 50%; height: 180px; }
        .game-dock-preview.bottom-end { bottom: 0; left: 50%; right: 0; height: 180px; }
        .game-dock-preview.top-start.compact, .game-dock-preview.top-end.compact { height: 56px; }
        .game-dock-preview.bottom-start.compact, .game-dock-preview.bottom-end.compact { height: 56px; }
        /* Боковые края — по ширине панели (320px), но между верхней и нижней зонами:
           верх и низ упираются в них, а не налезают друг на друга. */
        .instance-view.game-edge-bottom { flex-direction: column-reverse; }
        .instance-view.game-edge-left { flex-direction: row; }
        .instance-view.game-edge-right { flex-direction: row-reverse; }

        .tab-content {
                flex: 1;
                min-height: 0;
                overflow: clip;
                position: relative;
        }

        .details-section,
        .subview-section {
                position: absolute;
                inset: 0;
        }

        /* Keep the board measurable while a subview is open. */
        .details-section {
                transition: opacity var(--section-duration) ease, visibility var(--section-duration);
        }

        .details-section.inactive {
                opacity: 0;
                visibility: hidden;
        }

        .subview-section {
                z-index: 1;
                padding: 24px;
                overflow-y: auto;
                scrollbar-gutter: stable;
        }

        .subview-section.gallery {
                display: flex;
                flex-direction: column;
                overflow: hidden;
                scrollbar-gutter: auto;
        }
        .gallery .subview-body {
                display: flex;
                flex: 1;
                min-height: 0;
        }


        .fallback-panel {
                flex: 1;
                min-width: 0;
                min-height: 0;
                overflow: hidden;
        }

        .nav-card {
                position: absolute;
                bottom: 24px;
                left: 24px;
                width: 340px;
                max-width: calc(100% - 48px);
                border-radius: var(--border-radius);
                background: var(--bg-card);
                border: 1px solid var(--border);
                overflow: hidden;
                transition: box-shadow 0.6s cubic-bezier(0.25, 0.1, 0.25, 1);
        }

        .nav-card:hover {
                box-shadow: var(--shadow-lg);
        }

        .nav-card.info-hidden .nav-items {
                max-height: 0;
        }
        .nav-card.info-hidden:hover .nav-items,
        .nav-card.info-hidden:focus-within .nav-items {
                max-height: 0;
        }
        .nav-card.info-hidden.info-expanded .nav-items {
                max-height: 360px;
        }
        .nav-card.info-pinned .nav-items {
                max-height: 360px;
        }
        .nav-card.info-pinned:hover .nav-items,
        .nav-card.info-pinned:focus-within .nav-items {
                max-height: 360px;
        }

        .nav-card-header {
                position: relative;
                display: flex;
                align-items: center;
                gap: 8px;
                padding: 9px 14px;
                color: var(--text-tertiary);
                font-family: inherit;
                font-size: 0.72rem;
                font-weight: 600;
                letter-spacing: 0.3px;
                text-transform: uppercase;
                user-select: none;
        }

        .nav-card-header-toggle {
                position: absolute;
                inset: 0;
                z-index: 0;
                width: 100%;
                padding: 0;
                border: 0;
                background: transparent;
                cursor: pointer;
        }
        .nav-card-title,
        .info-mode-toggle,
        .tile-toggle {
                position: relative;
                z-index: 1;
        }
        .nav-card-title {
                flex: 1;
                padding: 0;
                border: 0;
                background: transparent;
                color: inherit;
                font: inherit;
                text-align: left;
                cursor: pointer;
        }
        .nav-card-title:focus-visible,
        .info-mode-toggle:focus-visible {
                outline: 2px solid var(--accent);
                outline-offset: 3px;
                border-radius: 3px;
        }
        .info-mode-toggle {
                display: flex;
                align-items: center;
                justify-content: center;
                width: 24px;
                height: 24px;
                padding: 0;
                border: 0;
                border-radius: var(--border-radius-sm);
                background: transparent;
                color: var(--text-tertiary);
                cursor: pointer;
        }
        .info-mode-toggle:hover {
                background: var(--surface-hover);
                color: var(--text-primary);
        }

        .nav-items {
                overflow: hidden;
                max-height: 82px;
                transition: max-height 0.6s cubic-bezier(0.25, 0.1, 0.25, 1);
        }

        .nav-card:hover .nav-items,
        .nav-card:focus-within .nav-items {
                max-height: 360px;
        }

        .nav-item {
                position: relative;
                display: flex;
                align-items: center;
                gap: 10px;
                min-height: 40px;
                padding: 10px 14px;
                background: transparent;
                border: none;
                color: var(--text-secondary);
                font-family: inherit;
                font-size: 0.82rem;
                font-weight: 600;
                cursor: pointer;
                transition:
                        background 0.12s ease,
                        color 0.12s ease;
                text-align: left;
                width: 100%;
        }

        .nav-divider {
                height: 1px;
                margin: 6px 14px;
                background: var(--border);
        }

        .nav-item:focus-visible {
                outline: 2px solid var(--accent);
                outline-offset: -2px;
        }

        .nav-item:hover {
                background: var(--surface-hover);
                color: var(--text-primary);
        }

        .nav-icon {
                display: flex;
                align-items: center;
                justify-content: center;
                width: 20px;
                height: 20px;
                flex-shrink: 0;
                opacity: 0.7;
        }

        .nav-item:hover .nav-icon {
                opacity: 1;
        }

        .nav-label {
                flex: 1;
                min-width: 0;
                overflow-wrap: anywhere;
        }

        .nav-chevron {
                display: flex;
                align-items: center;
                justify-content: center;
                width: 16px;
                height: 20px;
                flex-shrink: 0;
                opacity: 0.4;
                transition:
                        transform 0.15s ease,
                        opacity 0.15s ease;
        }

        .nav-item:hover .nav-chevron {
                opacity: 0.8;
                transform: translateX(2px);
        }

        .nav-slider {
                position: absolute;
                top: 5px;
                right: 40px;
                height: 30px;
                pointer-events: none;
        }

        .tile-clip {
                position: absolute;
                inset: 0;
                overflow: hidden;
                border-radius: inherit;
                pointer-events: none;
        }

        .tile-slider {
                position: absolute;
                display: block;
        }

        /* Icono más grueso en los niveles grandes (iconos de máscara: se engrosa el contorno). */
        .tile.bold-icon .tile-icon {
                filter: drop-shadow(0 0 0.5px currentColor)
                        drop-shadow(0 0 0.5px currentColor);
        }

        .tile-toggle {
                display: flex;
                align-items: center;
                justify-content: center;
                width: 24px;
                height: 24px;
                margin: -4px -4px -4px 0;
                padding: 0;
                background: transparent;
                border: none;
                border-radius: var(--border-radius-sm);
                color: var(--text-tertiary);
                cursor: pointer;
                transition:
                        background 0.12s ease,
                        color 0.12s ease;
        }

        .tile-toggle:hover,
        .tile-toggle:focus-visible {
                color: var(--text-primary);
                background: color-mix(in srgb, var(--text-primary) 8%, transparent);
        }

        /* ---- Modo de mosaicos ---- */
        .tile-viewport {
                position: absolute;
                z-index: 1;
                top: 24px;
                left: 24px;
                right: 24px;
                bottom: 76px;
                overflow: clip;
                overflow-clip-margin: 6px;
        }

        .tile-board {
                position: absolute;
                top: 0;
                left: 0;
                transform-origin: top left;
        }

        /* Cuadrícula 60×40, visible solo mientras se mueve o redimensiona un mosaico. */
        .tile-board::before {
                content: "";
                position: absolute;
                inset: 0;
                pointer-events: none;
                opacity: 0;
                transition: opacity 0.18s ease;
                border-radius: var(--border-radius);
                outline: 1px dashed color-mix(in srgb, var(--accent) 28%, transparent);
                background-image:
                        linear-gradient(
                                to right,
                                color-mix(in srgb, var(--accent) 12%, transparent) 1px,
                                transparent 1px
                        ),
                        linear-gradient(
                                to bottom,
                                color-mix(in srgb, var(--accent) 12%, transparent) 1px,
                                transparent 1px
                        );
                background-size:
                        calc(100% / 60) 100%,
                        100% calc(100% / 40);
        }

        .tile-board.dragging::before {
                opacity: 1;
        }

        .tile {
                position: absolute;
                display: flex;
                align-items: center;
                justify-content: flex-start;
                gap: 10px;
                padding: 0 14px;
                box-sizing: border-box;
                background: var(--bg-card);
                border: 1px solid var(--border);
                border-radius: var(--border-radius);
                color: var(--text-secondary);
                font-family: inherit;
                font-size: 0.82rem;
                font-weight: 600;
                text-align: left;
                cursor: pointer;
                user-select: none;
                touch-action: none;
                transition:
                        left 0.08s ease,
                        top 0.08s ease,
                        width 0.08s ease,
                        height 0.08s ease,
                        background 0.12s ease,
                        color 0.12s ease,
                        box-shadow 0.2s ease;
        }

        .tile.right {
                flex-direction: row-reverse;
                text-align: right;
        }

        /* В режиме редактирования плитки можно двигать и менять размер. */
        .tile.editing {
                cursor: move;
                border-style: dashed;
                border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
        }

        .tile.plain {
                overflow: visible;
        }

        /* ---- «Стильный» и «Стекло»: подпись поверх растянутых слайдеров ---- */
        .tile-caption-clip {
                position: absolute;
                inset: 0;
                z-index: auto;
                overflow: hidden;
                border-radius: inherit;
                pointer-events: none;
        }

        .tile-caption {
                z-index: 2;
                position: absolute;
                display: flex;
                align-items: center;
                max-width: 100%;
                box-sizing: border-box;
                padding: var(--cap-pad);
                color: inherit;
                isolation: isolate;
        }

        /* Подложка цвета плитки: за пределы подписи затухает во все стороны. */
        .tile-caption::before {
                box-shadow: 0 1px 5px color-mix(in srgb, var(--text-primary) 12%, transparent);
                content: "";
                position: absolute;
                inset: calc(var(--cap-fade) * -1);
                z-index: -1;
                background: color-mix(in srgb, var(--bg-card) 94%, transparent);
                -webkit-mask-image:
                        linear-gradient(
                                to right,
                                transparent 0,
                                #000 var(--cap-fade),
                                #000 calc(100% - var(--cap-fade)),
                                transparent 100%
                        ),
                        linear-gradient(
                                to bottom,
                                transparent 0,
                                #000 var(--cap-fade),
                                #000 calc(100% - var(--cap-fade)),
                                transparent 100%
                        );
                -webkit-mask-composite: source-in;
                mask-image:
                        linear-gradient(
                                to right,
                                transparent 0,
                                #000 var(--cap-fade),
                                #000 calc(100% - var(--cap-fade)),
                                transparent 100%
                        ),
                        linear-gradient(
                                to bottom,
                                transparent 0,
                                #000 var(--cap-fade),
                                #000 calc(100% - var(--cap-fade)),
                                transparent 100%
                        );
                mask-composite: intersect;
        }

        /* В стилях «Стильный» и «Стекло» подложка подписи — цельная
           скруглённая плашка, без мягкого фейда. Стандартный стиль не меняется. */
        :global([data-theme="dark"]) .tile-caption::before,
        :global([data-theme="midnight"]) .tile-caption::before,
        :global(.theme-dark) .tile-caption::before {
                box-shadow: 0 0 9px color-mix(in srgb, var(--accent) 55%, transparent);
        }
        .tile-caption.solid::before {
                inset: 0;
                border-radius: var(--border-radius);
                background: var(--bg-card);
                -webkit-mask-image: none;
                mask-image: none;
                -webkit-mask-composite: source-over;
                mask-composite: add;
        }

        /* Более чистая стеклянная подложка для «Стильного» — контрастная и в тёмной теме. */
        .tile-caption.solid.stylish::before {
                inset: 0;
                border: 1px solid color-mix(in srgb, var(--accent) 22%, var(--border-color));
                /* У «Стильного» скругляется дальний от подписи угол панели;
                   два угла у стороны привязки остаются прямыми. */
                border-radius: 0;
                /* Используем базовую поверхность текущей темы: светлая — светлая,
                   тёмная — тёмная, стандартная — её собственный цвет. */
                background: var(--bg-card);
                box-shadow: 0 6px 18px color-mix(in srgb, #000 16%, transparent);
                -webkit-mask-image: none;
                mask-image: none;
                -webkit-mask-composite: source-over;
                mask-composite: add;
                backdrop-filter: none;
        }

        /* Асимметричная рамка «Стильного»: скруглён дальний от подписи угол.
           Для центрального положения симметрично скругляются два дальних угла. */
        .tile-caption.stylish[data-pos^="top"]::before { border-top-color: transparent; }
        .tile-caption.stylish[data-pos^="bottom"]::before { border-bottom-color: transparent; }
        .tile-caption.stylish[data-pos$="left"]::before { border-left-color: transparent; }
        .tile-caption.stylish[data-pos$="right"]::before { border-right-color: transparent; }

        .tile-caption.solid.stylish[data-pos="top-left"]::before {
                border-radius: 0 0 12px 0;
        }
        .tile-caption.solid.stylish[data-pos="top-center"]::before {
                border-radius: 0 0 12px 12px;
        }
        .tile-caption.solid.stylish[data-pos="top-right"]::before {
                border-radius: 0 0 0 12px;
        }
        .tile-caption.solid.stylish[data-pos="bottom-left"]::before {
                border-radius: 0 12px 0 0;
        }
        .tile-caption.solid.stylish[data-pos="bottom-center"]::before {
                border-radius: 12px 12px 0 0;
        }
        .tile-caption.solid.stylish[data-pos="bottom-right"]::before {
                border-radius: 12px 0 0 0;
        }

        /* «Стекло»: обычное расположение подписи, но на полупрозрачной панели
           поверх слайдеров. Ширина панели регулируется настройкой «Размер поля». */
        .tile-caption.glass {
                top: 0;
                bottom: 0;
                width: max-content;
                min-width: var(--glass-width, 50%);
                max-width: 100%;
                justify-content: flex-start;
                align-items: center;
                isolation: auto;
        }
        .tile-caption.glass.gallery-caption {
                /* Keep room for both arrows, even when the glass width is set to 100%. */
                --gallery-caption-max: max(50%, calc(100% - 56px));
                min-width: min(var(--glass-width, 50%), var(--gallery-caption-max));
                max-width: var(--gallery-caption-max);
        }
        .tile-caption.glass::before {
                inset: 0;
                border: 0;
                border-radius: 0;
                /* Keep moving artwork visible without letting its colors dominate the caption. */
                background: color-mix(in srgb, var(--bg-card) 82%, transparent);
                box-shadow: inset 0 1px rgb(255 255 255 / 15%);
                -webkit-backdrop-filter: blur(var(--caption-blur, 12px)) saturate(55%);
                backdrop-filter: blur(var(--caption-blur, 12px)) saturate(55%);
                -webkit-mask-image: none;
                mask-image: none;
                -webkit-mask-composite: source-over;
                mask-composite: add;
        }
        .tile-caption.glass[data-pos$="right"] {
                left: auto;
                right: 0;
        }
        .tile-caption.glass[data-pos$="left"] {
                left: 0;
                right: auto;
        }
        .tile-caption.glass .tile-label { text-align: inherit; }

        .tile-caption.rev {
                flex-direction: row-reverse;
                text-align: right;
        }

        .tile-caption .tile-label {
                flex: 0 1 auto;
        }

        .tile-caption[data-pos^="top"] {
                top: 0;
        }
        .tile-caption[data-pos^="bottom"] {
                bottom: 0;
        }
        .tile-caption[data-pos$="left"] {
                left: 0;
        }
        .tile-caption[data-pos$="right"] {
                right: 0;
        }
        .tile-caption[data-pos$="center"]:not(.glass) {
                left: 50%;
                transform: translateX(-50%);
        }
        .tile-caption.glass.no-blur::before {
                -webkit-backdrop-filter: none;
                backdrop-filter: none;
                background-color: var(--bg-card);
        }

        .tile:hover {
                color: var(--text-primary);
                box-shadow: var(--shadow-lg);
        }

        .tile.active {
                z-index: 2;
                color: var(--text-primary);
                border-color: var(--accent);
        }

        .tile:focus-visible {
                outline: 2px solid var(--accent);
                outline-offset: -2px;
        }

        .tile-icon {
                display: flex;
                align-items: center;
                justify-content: center;
                flex-shrink: 0;
                opacity: 0.7;
        }

        .tile:hover .tile-icon {
                opacity: 1;
        }

        .tile-label {
                flex: 1;
                min-width: 0;
                max-height: 100%;
                overflow: hidden;
                overflow-wrap: anywhere;
                line-height: 1.2;
        }

        .tile-handle {
                position: absolute;
                z-index: 1;
        }

        .handle-n,
        .handle-s {
                left: 10px;
                right: 10px;
                height: 8px;
                cursor: ns-resize;
        }
        .handle-e,
        .handle-w {
                top: 10px;
                bottom: 10px;
                width: 8px;
                cursor: ew-resize;
        }
        .handle-n {
                top: -4px;
        }
        .handle-s {
                bottom: -4px;
        }
        .handle-e {
                right: -4px;
        }
        .handle-w {
                left: -4px;
        }
        .handle-ne,
        .handle-nw,
        .handle-se,
        .handle-sw {
                width: 14px;
                height: 14px;
        }
        .handle-ne {
                top: -5px;
                right: -5px;
                cursor: nesw-resize;
        }
        .handle-sw {
                bottom: -5px;
                left: -5px;
                cursor: nesw-resize;
        }
        .handle-nw {
                top: -5px;
                left: -5px;
                cursor: nwse-resize;
        }
        .handle-se {
                bottom: -5px;
                right: -5px;
                cursor: nwse-resize;
        }

        /* ---- Панель управления режимом плиток ---- */
        .tile-bar {
                position: absolute;
                left: 24px;
                bottom: 24px;
                z-index: 5;
                display: flex;
                align-items: center;
                gap: 8px;
        }

        .tile-btn {
                display: flex;
                align-items: center;
                justify-content: center;
                width: 36px;
                height: 36px;
                padding: 0;
                background: var(--bg-card);
                border: 1px solid var(--border);
                border-radius: var(--border-radius);
                color: var(--text-secondary);
                font-family: inherit;
                cursor: pointer;
                transition:
                        background 0.12s ease,
                        color 0.12s ease,
                        border-color 0.12s ease,
                        box-shadow 0.2s ease;
        }

        .tile-btn:hover,
        .tile-btn.open {
                color: var(--text-primary);
                box-shadow: var(--shadow-lg);
        }

        .tile-btn.open {
                border-color: var(--accent);
        }

        .tile-btn:focus-visible {
                outline: 2px solid var(--accent);
                outline-offset: -2px;
        }

        /* Подтверждение над кнопкой «Сохранить» — как список смены аккаунта. */
        .tile-confirm {
                position: absolute;
                left: 0;
                bottom: calc(100% + 8px);
                z-index: 6;
                display: flex;
                align-items: center;
                gap: 6px;
                padding: 6px 6px 6px 12px;
                white-space: nowrap;
                background-color: var(--bg-sidebar);
                background-clip: padding-box;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                box-shadow: var(--shadow-sm);
        }

        .tile-confirm.exit-confirm {
                flex-direction: column;
                align-items: stretch;
                gap: 10px;
                width: 240px;
                padding: 14px;
                white-space: normal;
                border-radius: var(--border-radius-md);
                box-shadow: var(--shadow-lg, 0 12px 32px rgba(0, 0, 0, 0.45));
                isolation: isolate;
                animation: tile-confirm-in 160ms ease-out;
        }
        .tile-confirm-actions { display: flex; flex-direction: column; gap: 6px; }
        .tile-confirm-action {
                display: flex;
                align-items: center;
                gap: 8px;
                width: 100%;
                padding: 8px 10px;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                background: var(--bg-item);
                color: var(--text-primary);
                font: inherit;
                font-size: 0.8rem;
                font-weight: 600;
                text-align: left;
                cursor: pointer;
                transition: background 0.15s, border-color 0.15s, color 0.15s;
        }
        .tile-confirm-action.primary { background: var(--accent); border-color: var(--accent); color: var(--accent-text, #fff); }
        .tile-confirm-action.primary:hover { filter: brightness(1.08); }
        .tile-confirm-action.danger:hover {
                border-color: var(--color-error);
                color: var(--color-error);
                background: rgba(var(--color-error-rgb), 0.12);
        }
        @keyframes tile-confirm-in {
                from { opacity: 0; transform: translateY(6px); }
                to { opacity: 1; transform: none; }
        }
        @media (prefers-reduced-motion: reduce) {
                .tile-confirm.exit-confirm { animation: none; }
        }
        .tile-confirm-cancel { flex-basis: 100%; border: 0; padding: 4px; background: transparent; color: var(--accent); font: inherit; font-weight: 600; cursor: pointer; text-align: center; }
        .tile-confirm-cancel:hover { color: var(--text-primary); }
        .tile-confirm-text {
                margin-right: 4px;
                font-size: 0.8rem;
                font-weight: 600;
                color: var(--text-primary);
        }

        .tile-confirm-btn {
                display: flex;
                align-items: center;
                justify-content: center;
                width: 28px;
                height: 28px;
                padding: 0;
                background: transparent;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                color: var(--text-secondary);
                cursor: pointer;
                transition:
                        background 0.12s ease,
                        color 0.12s ease,
                        border-color 0.12s ease;
        }

        .tile-confirm-btn.yes:hover {
                background: rgba(var(--color-success-rgb), 0.14);
                border-color: var(--color-success);
                color: var(--color-success);
        }

        .tile-confirm-btn.no:hover {
                background: rgba(var(--color-error-rgb), 0.12);
                border-color: var(--color-error);
                color: var(--color-error);
        }

        .tile-settings-pop {
                position: absolute;
                left: 24px;
                bottom: 72px;
                z-index: 1100; /* панели редактирования поверх плиток и управляющих слоёв */
                isolation: isolate; /* собственный слой: содержимое плиток не может просвечивать */
                backdrop-filter: none;
                width: 340px;
                max-width: calc(100% - 48px);
                max-height: calc(100% - 104px);
                overflow-y: auto;
                padding: 14px;
                box-sizing: border-box;
                background: var(--bg-sidebar);
                border: 1px solid var(--border);
                border-radius: var(--border-radius);
                box-shadow: var(--shadow-lg);
        }

        @media (max-width: 700px) {
                .tile-viewport {
                        top: 16px;
                        left: 16px;
                        right: 16px;
                        bottom: 68px;
                }
                .tile-bar {
                        left: 16px;
                        bottom: 16px;
                }
                .tile-settings-pop {
                        left: 16px;
                        bottom: 64px;
                }
                .subview-section {
                        padding: 16px;
                }
                .nav-card {
                        bottom: 16px;
                        left: 16px;
                        max-width: calc(100% - 32px);
                }
        }

        @media (max-width: 400px) {
                .tile-viewport {
                        top: 12px;
                        left: 12px;
                        right: 12px;
                        bottom: 64px;
                }
                .tile-bar {
                        left: 12px;
                        bottom: 12px;
                }
                .tile-settings-pop {
                        left: 12px;
                        bottom: 60px;
                }
                .subview-section {
                        padding: 12px;
                }
                .nav-card {
                        bottom: 12px;
                        left: 12px;
                        max-width: calc(100% - 24px);
                }
        }

        /* Слой кнопок порядка: лежит ровно поверх плитки, сам кликов не ловит,
           кроме своих кнопок. Колонка прижата к правому краю плитки. */
        .tile-layer-ctrl {
                position: absolute;
                z-index: 40;
                pointer-events: none;
                box-sizing: border-box;
        }
        .tlc-col {
                position: absolute;
                top: 4px;
                left: 50%;
                transform: translateX(-50%);
                display: flex;
                flex-direction: row;
                gap: 3px;
                padding: 3px;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                background: var(--bg-card);
                box-shadow: var(--shadow-sm, var(--shadow-md));
                pointer-events: auto;
        }
        .tlc-extra {
                position: absolute;
                right: 4px;
                bottom: 4px;
                display: flex;
                flex-direction: row;
                gap: 3px;
                padding: 3px;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                background: var(--bg-card);
                box-shadow: var(--shadow-sm, var(--shadow-md));
                pointer-events: auto;
        }
        .tlc-btn.open {
                color: var(--accent);
                border-color: var(--accent);
        }
        .tlc-btn {
                width: 22px;
                height: 22px;
                padding: 0;
                display: flex;
                align-items: center;
                justify-content: center;
                border: 1px solid var(--border-color);
                border-radius: 4px;
                background: transparent;
                color: var(--text-primary);
                font-size: 12px;
                line-height: 1;
                cursor: pointer;
        }
        .tlc-btn:hover:not(:disabled) {
                border-color: var(--accent);
                color: var(--accent);
        }
        .tlc-btn:disabled {
                opacity: 0.35;
                cursor: default;
        }
                .tile-hidden-wrap {
                position: relative;
                display: inline-flex;
        }
        .tile-hidden-pop {
                position: absolute;
                left: 50%;
                bottom: calc(100% + 12px);
                margin-left: -95px; /* половина min-width, без transform: его занимает fly-переход */
                z-index: 1200;
                display: flex;
                flex-direction: column;
                gap: 5px;
                min-width: 190px;
                padding: 10px;
                border: 1px solid var(--border);
                border-radius: var(--border-radius);
                background: var(--bg-card);
                box-shadow: var(--shadow-lg);
                color: var(--text-primary);
                font-size: 0.78rem;
        }
        .tile-hidden-pop strong {
                padding: 2px 4px 5px;
                color: var(--text-muted);
                font-size: 0.68rem;
                text-transform: uppercase;
        }
        .tile-hidden-item {
                display: flex;
                align-items: center;
                justify-content: space-between;
                gap: 12px;
                padding: 7px 8px;
                border: 0;
                border-radius: var(--border-radius-sm);
                background: var(--bg-input);
                color: var(--text-primary);
                font: inherit;
                cursor: pointer;
        }
        .tile-hidden-item:hover { background: var(--surface-hover); }
</style>
