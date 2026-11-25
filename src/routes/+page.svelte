<script lang="ts">
        import { onMount, onDestroy } from "svelte";
        import { lazyComponent } from "$lib/utils/lazyComponent.svelte";
        import "../styles/App.css";
        import { launcherStore } from "$lib/state/state.svelte";
        import {
                getVersions,
                syncSettings,
                initEventListeners,
                destroyEventListeners,
        } from "$lib/api/launcherService";
        import type { InstanceDto } from "$lib/types/types";
        import Sidebar from "$lib/components/layout/Sidebar/Sidebar.svelte";
        import SidebarCompact from "$lib/components/layout/Sidebar/SidebarCompact.svelte";
        import SidebarContextMenu from "$lib/components/layout/Sidebar/SidebarContextMenu.svelte";
        import DeleteInstanceModal from "$lib/components/layout/Sidebar/DeleteInstanceModal.svelte";
        import InstanceLibrary from "$lib/components/instances/InstanceLibrary.svelte";
        import { deleteInst } from "$lib/api/launcherService";
        import { pinInstance } from "$lib/api/cubicApi";
        import { instanceNav } from "$lib/state/instanceNavState.svelte";
        import InstanceView from "$lib/components/instances/InstanceView/InstanceView.svelte";
        import Drawer from "$lib/components/layout/Drawer.svelte";
        import NotificationContainer from "$lib/components/ui/NotificationContainer.svelte";
        import JreInstallPrompt from "$lib/components/ui/JreInstallPrompt.svelte";
        import InstanceViewSkeleton from "$lib/components/instances/InstanceView/InstanceViewSkeleton.svelte";
        import ProfileViewSkeleton from "$lib/components/profiles/ProfileViewSkeleton.svelte";
        import SettingsSkeleton from "$lib/components/settings/SettingsSkeleton.svelte";
        import InstanceDrawerSkeleton from "$lib/components/instances/InstanceDrawer/InstanceDrawerSkeleton.svelte";
        import AccountSetup from "$lib/components/layout/auth/AccountSetup.svelte";
        import { getAccountProvider } from "$lib/utils/accountProviders";
        import { updaterState } from "$lib/state/updaterState.svelte";
        import Icon from "$lib/icons/Icon.svelte";
        import { t } from "$lib/i18n";
        import {
                applyTheme,
                importMoonTheme,
                THEME_EXTENSION,
        } from "$lib/api/themeManager";
        import { saveSettings } from "$lib/api/launcherService";
        import { showSuccess, showError } from "$lib/state/state.svelte";
        import { animDuration, windowSlide } from "$lib/utils/animations";
        import { limitSidebarSize, sidebarGeometry, sidebarSizes, type SidebarAxis } from "$lib/utils/sidebarResize";
        import {
                applyInterfaceScale,
                applyInterfaceDensity,
                applyInterfaceFont,
        } from "$lib/api/interfaceAppearance";
        import { interfacePreferences } from "$lib/utils/interfacePreferences";
        import { injectProjectFonts } from "$lib/fonts/projectFonts";
        import {
                isHorizontalDock,
                appendToPinOrder,
                removeFromPinOrder,
                orderInstances,
                setSidebarPlacement,
                cellOf,
                placementOf,
                type SidebarCell,
                sidebarLayout,
                type SidebarAlign,
                type SidebarDock,
        } from "$lib/state/sidebarLayoutState.svelte";
        import { panelLayout } from "$lib/state/panelLayoutState.svelte";
        import { appearance, applyAccentColor } from "$lib/state/appearanceState.svelte";

        $effect(() => applyAccentColor(appearance.accentColor));

        const logParams = $derived.by(() => {
                if (typeof window === "undefined") return null;
                const params = new URLSearchParams(window.location.search);
                const logId = params.get("log");
                if (!logId) return null;
                return {
                        instanceId: decodeURIComponent(logId),
                        instanceName: decodeURIComponent(params.get("name") || "Logs"),
                };
        });

        let selectedInstance = $state<InstanceDto | null>(null);
        const lastInstanceStorageKey = "moonlauncher:lastSelectedInstance";
        let pendingModpackName = $state<string | null>(null);
        let instanceMenu: ReturnType<typeof SidebarContextMenu> | undefined = $state();
        let deleteTarget: InstanceDto | null = $state(null);
        let deleteOpen = $state(false);

        function showLibrary() {
                const open = () => {
                        selectedInstance = null;
                        showProfileView = false;
                };
                if (!instanceNav.editActionGuard?.(open)) open();
        }

        function openInstance(instance: InstanceDto) {
                instanceNav.section = "detalles";
                selectedInstance = instance;
        }

        function editInstance(instance: InstanceDto) {
                editingInstance = instance;
                instanceEditorOpen = true;
        }

        function requestDelete(instance: InstanceDto) {
                deleteTarget = instance;
                deleteOpen = true;
        }

        async function confirmDelete() {
                if (deleteTarget && await deleteInst(deleteTarget.uuid)) deleteOpen = false;
        }

        async function setMenuPin(instance: InstanceDto, pinned: boolean) {
                await pinInstance(instance.uuid, pinned);
                if (pinned) {
                        appendToPinOrder(instance.uuid, orderInstances(launcherStore.loadedInstances)
                                .filter((item) => item.pinned && item.uuid !== instance.uuid)
                                .map((item) => item.uuid));
                } else removeFromPinOrder(instance.uuid);
        }
        let sidebarSize = $state<Record<SidebarAxis, number>>({
                vertical: sidebarSizes.vertical.normal,
                horizontal: sidebarSizes.horizontal.normal,
        });
        let sidebarSizesReady = $state(false);
        let resizeStart = $state<{ pointerId: number; coordinate: number; size: number } | null>(null);
        let quickMenuOpen = $state(false);
        let instanceEditorOpen = $state(false);
        let versionDownloaderOpen = $state(false);
        let openCreateModal = $state(false);
        let showProfileView = $state(false);
        let droppedMrpackPath = $state<string | null>(null);
        let droppedInstanceZipPath = $state<string | null>(null);
        let isDragOver = $state(false);
        let dragPaths = $state<string[]>([]);
        let editingInstance = $state<InstanceDto | null>(null);
        let showTutorial = $state(false);
        const settingsPanel = lazyComponent(
                () => import("$lib/components/settings/Settings.svelte"),
                () => quickMenuOpen,
                (error) => {
                        quickMenuOpen = false;
                        showError(t("errors.title"), String(error));
                },
        );
        const SettingsComponent = $derived(settingsPanel.component);

        const versionPanel = lazyComponent(
                () =>
                        import("$lib/components/layout/VersionDownloader/VersionDownloader.svelte"),
                () => versionDownloaderOpen,
                (error) => {
                        versionDownloaderOpen = false;
                        showError(t("errors.title"), String(error));
                },
        );
        const VersionDownloaderComponent = $derived(versionPanel.component);

        const createPanel = lazyComponent(
                () =>
                        import("$lib/components/instances/CreateInstanceModal/CreateInstanceModal.svelte"),
                () => openCreateModal,
                (error) => {
                        openCreateModal = false;
                        showError(t("errors.title"), String(error));
                },
        );
        const CreateInstanceModal = $derived(createPanel.component);

        const editorPanel = lazyComponent(
                () =>
                        import("$lib/components/instances/InstanceDrawer/InstanceDrawer.svelte"),
                () => instanceEditorOpen,
                (error) => {
                        instanceEditorOpen = false;
                        showError(t("errors.title"), String(error));
                },
        );
        const InstanceDrawer = $derived(editorPanel.component);

        const profilePanel = lazyComponent(
                () => import("$lib/components/profiles/ProfileView.svelte"),
                () => showProfileView,
                (error) => {
                        showProfileView = false;
                        showError(t("errors.title"), String(error));
                },
        );
        const ProfileView = $derived(profilePanel.component);

        const tutorialPanel = lazyComponent(
                () => import("$lib/components/layout/welcome/welcome.svelte"),
                () => showTutorial,
                (error) => {
                        showTutorial = false;
                        showError(t("errors.title"), String(error));
                },
        );
        const Tutorial = $derived(tutorialPanel.component);

        let logError = $state("");
        const logPanel = lazyComponent(
                () => import("$lib/components/log/LogWindow.svelte"),
                () => logParams !== null,
                (error) => {
                        logError = String(error);
                },
        );
        const LogWindow = $derived(logPanel.component);

        const updatePanel = lazyComponent(
                () => import("$lib/components/ui/UpdateModal.svelte"),
                () => !logParams && updaterState.open,
                (error) => {
                        updaterState.open = false;
                        showError(t("errors.title"), String(error));
                },
        );
        const UpdateModal = $derived(updatePanel.component);

        const sidebarTransitionDuration = $derived(animDuration(0.35, 0.05));
        const interfaceScale = $derived(
                interfacePreferences(launcherStore.settings.interface_preferences)
                        .scale,
        );
        const interfaceDensity = $derived(
                interfacePreferences(launcherStore.settings.interface_preferences)
                        .density,
        );
        const interfaceFont = $derived(
                interfacePreferences(launcherStore.settings.interface_preferences).font,
        );

        $effect(() => {
                void applyInterfaceScale(interfaceScale).catch((error) => {
                        showError(t("settings.interface.scaleError"), String(error));
                });
        });

        $effect(() => {
                applyInterfaceDensity(interfaceDensity);
        });

        $effect(() => {
                applyInterfaceFont(interfaceFont);
        });
        onDestroy(() => {
                if (typeof document !== "undefined") applyInterfaceDensity("theme");
        });

        $effect(() => {
                if (typeof document === "undefined") return;
                const html = document.documentElement;
                const s = launcherStore.settings;
                html.toggleAttribute("data-reduce-motion", s.reduce_animations);
                html.toggleAttribute("data-no-blur", s.disable_blur_effects);
                html.toggleAttribute(
                        "data-no-infinite-animations",
                        s.disable_infinite_animations,
                );
                html.toggleAttribute(
                        "data-no-background-image",
                        s.disable_background_image,
                );
        });

        let unlistenDragDrop: (() => void) | undefined;
        let settingsReady = $state(false);
        let startupReady = $state(false);
        let destroyed = false;

        // Las actualizaciones automáticas están desconectadas del launcher
        // (ver updaterServices.startAutoUpdates); el código se conserva.

        onMount(async () => {
                injectProjectFonts();
                initEventListeners(logParams ? "logs" : "main");
                if (logParams) {
                        await syncSettings();
                        return;
                }

                try {
                        const saved = JSON.parse(localStorage.getItem("sidebarSizes") ?? "null");
                        const legacyCompact = localStorage.getItem("sidebarMode") === "compact";
                        for (const axis of ["vertical", "horizontal"] as const) {
                                sidebarSize[axis] = Number.isFinite(saved?.[axis])
                                        ? limitSidebarSize(axis, saved[axis])
                                        : sidebarSizes[axis][legacyCompact ? "compact" : "normal"];
                        }
                } catch {
                        /* Storage may be unavailable; startup must still continue. */
                }
                sidebarSizesReady = true;

                // Updates depend on settings, not on a successful instance/version scan.
                const settings = syncSettings()
                        .then(() => {
                                if (destroyed) return;
                                showTutorial =
                                        launcherStore.settings.show_tutorial ||
                                        !launcherStore.settings.license_accepted;
                                settingsReady = true;
                        })
                        .catch((error) => {
                                console.warn("Could not load startup settings", error);
                                if (!destroyed) settingsReady = true;
                        });
                // Installed versions are only needed by the downloader and instance
                // editor. Loading them here delays reopening after a game exits.
                const [, instancesResult] = await Promise.allSettled([settings, getVersions()]);
                if (destroyed) return;

                if (instancesResult.status === "fulfilled" && launcherStore.settings.restore_last_instance && !selectedInstance) {
                        try {
                                const uuid = localStorage.getItem(lastInstanceStorageKey);
                                const instance = launcherStore.loadedInstances.find((item) => item.uuid === uuid);
                                if (instance) openInstance(instance);
                        } catch {
                                /* Opening the launcher does not depend on local storage. */
                        }
                }
                startupReady = true;

                setupDragDrop();
        });

        onDestroy(() => {
                destroyed = true;
                destroyEventListeners();
                unlistenDragDrop?.();
        });

        $effect(() => {
                if (logParams || !startupReady) return;
                const uuid = launcherStore.settings.restore_last_instance ? selectedInstance?.uuid : null;
                try {
                        if (uuid) localStorage.setItem(lastInstanceStorageKey, uuid);
                        else localStorage.removeItem(lastInstanceStorageKey);
                } catch {
                        /* The current selection still works without local storage. */
                }
        });

        $effect(() => {
                const theme = launcherStore.settings.theme;
                void applyTheme(theme || "dark");
        });

        $effect(() => {
                if (!sidebarSizesReady || resizeStart) return;
                try {
                        localStorage.setItem("sidebarSizes", JSON.stringify(sidebarSize));
                } catch {
                        /* Resizing still works when storage is unavailable. */
                }
        });

        async function setupDragDrop() {
                try {
                        const { getCurrentWebview } =
                                await import("@tauri-apps/api/webview");
                        if (destroyed) return;
                        const webview = getCurrentWebview();
                        unlistenDragDrop = await webview.onDragDropEvent((event) => {
                                if (destroyed) return;
                                if (event.payload.type === "enter") {
                                        const payload = event.payload as { paths: string[] };
                                        dragPaths = payload.paths ?? [];
                                        isDragOver =
                                                dragPaths.length > 0 &&
                                                dragPaths.some(
                                                        (p) =>
                                                                /\.(mrpack|zip)$/i.test(p) ||
                                                                p.toLowerCase().endsWith(`.${THEME_EXTENSION}`),
                                                );
                                } else if (event.payload.type === "leave") {
                                        isDragOver = false;
                                        dragPaths = [];
                                } else if (event.payload.type === "drop") {
                                        isDragOver = false;
                                        const paths =
                                                (event.payload as { paths: string[] }).paths ?? [];
                                        const zipFile = paths.find((p: string) =>
                                                /\.zip$/i.test(p),
                                        );
                                        const mrpackFile = paths.find((p: string) =>
                                                /\.mrpack$/i.test(p),
                                        );
                                        const themeFile = paths.find((p) => p.toLowerCase().endsWith(`.${THEME_EXTENSION}`));
                                        if (themeFile) {
                                                void handleThemeDrop(themeFile);
                                        } else if (mrpackFile) {
                                                droppedMrpackPath = mrpackFile;
                                                droppedInstanceZipPath = null;
                                                openCreateModal = true;
                                        } else if (zipFile) {
                                                droppedMrpackPath = null;
                                                droppedInstanceZipPath = zipFile;
                                                openCreateModal = true;
                                        }
                                        dragPaths = [];
                                }
                        });
                        if (destroyed) unlistenDragDrop();
                } catch (e) {
                        console.warn("Drag-drop not available:", e);
                }
        }

        async function handleThemeDrop(path: string) {
                try {
                        const entry = await importMoonTheme(path);
                        const id = `user:${entry.id}`;
                        const replacing = launcherStore.settings.theme === id;
                        launcherStore.settings.theme = id;
                        await Promise.all([saveSettings(), applyTheme(id, { force: replacing })]);
                        showSuccess(
                                t("themes.importSuccess"),
                                t("themes.importSuccessMessage"),
                        );
                } catch (e) {
                        showError(t("themes.importError"), String(e));
                }
        }

        // ---- Mover la barra lateral de borde (arrastrando un hueco de la lista o el rótulo) ----
        const dock = $derived(sidebarLayout.dock);
        const horizontalDock = $derived(isHorizontalDock(dock));

        // En ventanas estrechas la barra lateral desplegada no cabe (queda como un
        // amasijo de textos cortados), así que se muestra siempre la versión compacta.
        let narrowWindow = $state(false);
        $effect(() => {
                const query = window.matchMedia("(max-width: 650px)");
                narrowWindow = query.matches;
                const onChange = (event: MediaQueryListEvent) => {
                        narrowWindow = event.matches;
                };
                query.addEventListener("change", onChange);
                return () => query.removeEventListener("change", onChange);
        });
        const sidebarAxis = $derived(horizontalDock ? "horizontal" : "vertical");
        const geometry = $derived(sidebarGeometry(sidebarAxis,
                narrowWindow && sidebarSize[sidebarAxis] > sidebarSizes[sidebarAxis].threshold
                        ? sidebarSizes[sidebarAxis].compact
                        : sidebarSize[sidebarAxis], narrowWindow));
        const resizeDirection = $derived(dock === "right" || dock === "bottom" ? -1 : 1);
        let appEl: HTMLDivElement | undefined = $state();
        // Шесть зон, как у игровой панели: края left/right и половины верха/низа.
        const SIDEBAR_CELLS: SidebarCell[] = [
                "left",
                "right",
                "top-start",
                "top-end",
                "bottom-start",
                "bottom-end",
        ];
        let panelDrag = $state<SidebarCell | null>(null);
        const currentCell = $derived(cellOf(dock, sidebarLayout.align));

        /** Зона под указателем: ближайший край, а на верхнем/нижнем краю — левая или правая половина. */
        function dockAt(x: number, y: number): SidebarCell {
                const rect = appEl?.getBoundingClientRect();
                if (!rect) return currentCell;
                const distances: [SidebarDock, number][] = [
                        ["left", (x - rect.left) / rect.width],
                        ["right", (rect.right - x) / rect.width],
                        ["top", (y - rect.top) / rect.height],
                        ["bottom", (rect.bottom - y) / rect.height],
                ];
                distances.sort((a, b) => a[1] - b[1]);
                const edge = distances[0][0];
                if (edge === "left" || edge === "right") return edge;
                return `${edge}-${x - rect.left < rect.right - x ? "start" : "end"}` as SidebarCell;
        }

        function startPanelDrag(event: PointerEvent) {
                if (event.button !== 0 || panelDrag || panelLayout.locked) return;
                const startX = event.clientX;
                const startY = event.clientY;
                let moved = false;

                const stop = () => {
                        window.removeEventListener("pointermove", onMove);
                        window.removeEventListener("pointerup", onUp);
                        window.removeEventListener("pointercancel", onCancel);
                        window.removeEventListener("keydown", onKey);
                        document.documentElement.classList.remove("panel-dragging");
                        panelDrag = null;
                };
                const onMove = (e: PointerEvent) => {
                        if (!moved) {
                                if (Math.hypot(e.clientX - startX, e.clientY - startY) < 8)
                                        return;
                                moved = true;
                                document.documentElement.classList.add("panel-dragging");
                        }
                        panelDrag = dockAt(e.clientX, e.clientY);
                };
                const onUp = () => {
                        const target = panelDrag;
                        stop();
                        if (moved && target) {
                                const placement = placementOf(target);
                                setSidebarPlacement(placement.dock, placement.align);
                        }
                };
                const onCancel = () => stop();
                const onKey = (e: KeyboardEvent) => {
                        if (e.key === "Escape") stop();
                };
                window.addEventListener("pointermove", onMove);
                window.addEventListener("pointerup", onUp);
                window.addEventListener("pointercancel", onCancel);
                window.addEventListener("keydown", onKey);
        }

        function startSidebarResize(event: PointerEvent) {
                if (!event.isPrimary || event.button !== 0 || resizeStart) return;
                const handle = event.currentTarget as HTMLElement;
                handle.focus({ preventScroll: true });
                handle.setPointerCapture(event.pointerId);
                resizeStart = { pointerId: event.pointerId, coordinate: horizontalDock ? event.clientY : event.clientX, size: geometry.size };
        }

        function moveSidebarResize(event: PointerEvent) {
                if (event.pointerId !== resizeStart?.pointerId) return;
                const coordinate = horizontalDock ? event.clientY : event.clientX;
                sidebarSize[sidebarAxis] = limitSidebarSize(sidebarAxis,
                        resizeStart.size + resizeDirection * (coordinate - resizeStart.coordinate), narrowWindow);
        }

        function endSidebarResize(event: PointerEvent) {
                if (event.pointerId !== resizeStart?.pointerId) return;
                resizeStart = null;
        }

        function resizeSidebarWithKeys(event: KeyboardEvent) {
                const limits = sidebarSizes[sidebarAxis];
                const grow = horizontalDock ? "ArrowDown" : "ArrowRight";
                const shrink = horizontalDock ? "ArrowUp" : "ArrowLeft";
                let size: number;
                if (event.key === "Home") size = limits.min;
                else if (event.key === "End") size = narrowWindow ? limits.compactMax : limits.max;
                else if (event.key === grow || event.key === shrink)
                        size = (narrowWindow ? geometry.size : sidebarSize[sidebarAxis]) + (event.key === grow ? 1 : -1) * resizeDirection * 8;
                else return;
                event.preventDefault();
                sidebarSize[sidebarAxis] = limitSidebarSize(sidebarAxis, size, narrowWindow);
        }

        function onTutorialClose() {
                launcherStore.settings.show_tutorial = false;
                saveSettings();
        }

        function selectPendingModpack() {
                if (!pendingModpackName) return;
                const instance = launcherStore.loadedInstances.find(
                        (instance) => instance.name === pendingModpackName,
                );
                if (instance) {
                        selectedInstance = instance;
                        showProfileView = false;
                        pendingModpackName = null;
                }
        }

        $effect(() => {
                selectPendingModpack();
        });

        $effect(() => {
                const instances = launcherStore.loadedInstances;
                const sel = selectedInstance;
                if (sel) {
                        const updated = instances.find((i) => i.uuid === sel.uuid);
                        if (
                                updated &&
                                (updated.status !== sel.status ||
                                        updated.name !== sel.name ||
                                        updated.loader !== sel.loader ||
                                        updated.version !== sel.version ||
                                        updated.last_played !== sel.last_played ||
                                        updated.playtime_seconds !== sel.playtime_seconds ||
                                        updated.icon !== sel.icon ||
                                        updated.cover_image !== sel.cover_image)
                        ) {
                                selectedInstance = updated;
                        } else if (!updated) {
                                selectedInstance = null;
                        }
                }
        });

        $effect(() => {
                const instances = launcherStore.loadedInstances;
                const ei = editingInstance;
                if (ei) {
                        const updated = instances.find((i) => i.uuid === ei.uuid);
                        if (updated && updated.overrides !== ei.overrides) {
                                editingInstance = updated;
                        } else if (!updated) {
                                editingInstance = null;
                        }
                }
        });

        function preventNativeContextMenu(event: MouseEvent) {
                if (!event.defaultPrevented) event.preventDefault();
        }

        function usePointerFocus() {
                document.documentElement.setAttribute("data-pointer-focus", "");
        }

        function useKeyboardFocus(event: KeyboardEvent) {
                if (event.key === "Tab") {
                        document.documentElement.removeAttribute("data-pointer-focus");
                }
        }
</script>
<svelte:window
        oncontextmenu={preventNativeContextMenu}
        onpointerdowncapture={usePointerFocus}
        onkeydowncapture={useKeyboardFocus}
/>

{#if logParams}
        {#if LogWindow}
                <LogWindow
                        instanceId={logParams.instanceId}
                        instanceName={logParams.instanceName}
                />
        {:else if logError}
                <p role="alert">{t("errors.title")}: {logError}</p>
        {:else}
                <p role="status">{t("common.loading")}</p>
        {/if}
{:else}
        <div
                class="app-container dock-{dock}"
                class:drag-over={isDragOver}
                bind:this={appEl}
                data-part="app"
                data-sidebar-dock={dock}
        >
                {#if isDragOver}
                        <div class="drag-overlay">
                                <div class="drag-overlay-content">
                                        <div class="drag-icon">
                                                <Icon name="instance:box" size={48} />
                                        </div>
                                        <h2>{t("main.dropTitle")}</h2>
                                        <p>{t("main.dropDesc")}</p>
                                </div>
                        </div>
                {/if}

                <div
                        id="instance-sidebar"
                        class="sidebar-container"
                        data-part="sidebar"
                        class:resizing={resizeStart !== null}
                        class:horizontal={horizontalDock}
                        style:--sidebar-size={`${geometry.size}px`}
                        style:--sidebar-card-scale={geometry.scale}
                        style:--sidebar-resize-duration={`${sidebarTransitionDuration}s`}
                >
                        {#if !geometry.compact}
                                <Sidebar
                                        bind:selectedInstance
                                        onopenquickmenu={() => (quickMenuOpen = true)}
                                        onopenprofileview={() => (showProfileView = !showProfileView)}
                                        profileOpen={showProfileView}
                                        onopenversiondownloader={() =>
                                                (versionDownloaderOpen = true)}
                                        onopencreateinstance={() => (openCreateModal = true)}
                                        onopeneditinstance={editInstance}
                                        ondeleteinstance={requestDelete}
                                        onopenlibrary={showLibrary}
                                        oninstancemenu={(event) => void instanceMenu?.openContextMenu(event)}
                                        onselectinstance={() => (showProfileView = false)}
                                        {dock}
                                        align={sidebarLayout.align}
                                        onpaneldrag={startPanelDrag}
                                />
                        {:else}
                                <SidebarCompact
                                        bind:selectedInstance
                                        onopenquickmenu={() => (quickMenuOpen = true)}
                                        onopenprofileview={() => (showProfileView = !showProfileView)}
                                        profileOpen={showProfileView}
                                        onopenversiondownloader={() =>
                                                (versionDownloaderOpen = true)}
                                        onopencreateinstance={() => (openCreateModal = true)}
                                        onopenlibrary={showLibrary}
                                        oninstancemenu={(event) => void instanceMenu?.openContextMenu(event)}
                                        onselectinstance={() => (showProfileView = false)}
                                        {dock}
                                        align={sidebarLayout.align}
                                        onpaneldrag={startPanelDrag}
                                />
                        {/if}
                        <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (Focusable ARIA window splitter) -->
                        <div class="sidebar-resize-handle {dock}"
                                role="separator" tabindex="0"
                                aria-label={t("sidebar.resize")}
                                aria-orientation={horizontalDock ? "horizontal" : "vertical"}
                                aria-valuemin={sidebarSizes[sidebarAxis].min}
                                aria-valuemax={sidebarSizes[sidebarAxis][narrowWindow ? "compactMax" : "max"]}
                                aria-valuenow={geometry.size}
                                onpointerdown={startSidebarResize}
                                onpointermove={moveSidebarResize}
                                onpointerup={endSidebarResize}
                                onpointercancel={endSidebarResize}
                                onlostpointercapture={endSidebarResize}
                                onkeydown={resizeSidebarWithKeys}
                        ></div>
                </div>

                {#if panelDrag}
                        {#each SIDEBAR_CELLS as cell (cell)}
                                <div
                                        class="dock-preview {cell}"
                                        class:hover={panelDrag === cell}
                                        class:current={cell === currentCell}
                                        class:compact={geometry.compact}
                                        aria-hidden="true"
                                ><span class="dock-bar"></span></div>
                        {/each}
                {/if}

                <main class="main-content" data-part="content">
                        <div
                                class="background-overlay"
                                data-part="background-overlay"
                        ></div>

                        <div class="instance-surface" inert={showProfileView}>
                        {#if selectedInstance}
                                <div class="view-pane" in:windowSlide={{ edge: "right", distance: 24, fade: true }} out:windowSlide={{ edge: "left", distance: 24, fade: true }}>
                                        <InstanceView {selectedInstance} onopenlibrary={showLibrary} />
                                </div>
                        {:else if !startupReady || launcherStore.instancesLoading}
                                <div class="view-pane" in:windowSlide={{ edge: "right", distance: 24, fade: true }} out:windowSlide={{ edge: "left", distance: 24, fade: true }}>
                                        <InstanceViewSkeleton />
                                </div>
                        {:else}
                                <div class="view-pane" in:windowSlide={{ edge: "left", distance: 24, fade: true }} out:windowSlide={{ edge: "right", distance: 24, fade: true }}>
                                        <InstanceLibrary
                                                onselect={openInstance}
                                                oncreate={() => (openCreateModal = true)}
                                                oninstancemenu={(event, anchor) => void instanceMenu?.openContextMenu(event, anchor)}
                                        />
                                </div>
                        {/if}
                        </div>

                        {#if showProfileView}
                                <div
                                        class="profile-surface"
                                        in:windowSlide
                                        out:windowSlide
                                >
                                        {#if ProfileView}
                                                <ProfileView open={showProfileView} onclose={() => (showProfileView = false)} />
                                        {:else}
                                                <ProfileViewSkeleton />
                                        {/if}
                                </div>
                        {/if}
                </main>
        </div>

        <SidebarContextMenu bind:this={instanceMenu} onedit={editInstance} ondelete={requestDelete} onpin={setMenuPin} />
        <DeleteInstanceModal bind:open={deleteOpen} instanceName={deleteTarget?.name ?? ""} onconfirm={confirmDelete} />

        <Drawer bind:open={quickMenuOpen} direction="right">
                {#if SettingsComponent}
                        <SettingsComponent onclose={() => (quickMenuOpen = false)} />
                {:else}
                        <SettingsSkeleton />
                {/if}
        </Drawer>

        {#if editingInstance}
                <Drawer bind:open={instanceEditorOpen} direction="right" onclosed={() => (editingInstance = null)}>
                        {#if InstanceDrawer}
                                <InstanceDrawer
                                        onclose={() => (instanceEditorOpen = false)}
                                        instance={editingInstance}
                                />
                        {:else}
                                <InstanceDrawerSkeleton />
                        {/if}
                </Drawer>
        {/if}

        {#if VersionDownloaderComponent}
                <VersionDownloaderComponent bind:open={versionDownloaderOpen} />
        {/if}

        {#if CreateInstanceModal}
                <CreateInstanceModal
                        bind:open={openCreateModal}
                        bind:mrpackPath={droppedMrpackPath}
                        bind:instanceZipPath={droppedInstanceZipPath}
                        oninstallstarted={(name) => (pendingModpackName = name)}
                        oninstallfailed={(name) => {
                                if (pendingModpackName === name) pendingModpackName = null;
                        }}
                />
        {/if}

        {#if Tutorial}
                <Tutorial
                        bind:open={showTutorial}
                        onclose={onTutorialClose}
                        onopensettings={() => (quickMenuOpen = true)}
                />
        {/if}

        <NotificationContainer />
        <JreInstallPrompt />
        {#if launcherStore.reauthPrompt && !logParams}
                <AccountSetup
                        provider={getAccountProvider(launcherStore.reauthPrompt)}
                        initialServerUrl={launcherStore.reauthPrompt.yggdrasil_server_url ??
                                undefined}
                        autoConnect={launcherStore.reauthPrompt.user_type === "Yggdrasil"}
                        reauthUsername={launcherStore.reauthPrompt.username}
                        onclose={() => (launcherStore.reauthPrompt = null)}
                />
        {/if}
        {#if UpdateModal}
                <UpdateModal />
        {/if}
{/if}

<style>
        /* Панель экрана занимает всю поверхность, поэтому при переходе
           входящий и уходящий экраны перекрываются и не сдвигают друг друга. */
        .instance-surface > .view-pane {
        	position: absolute;
        	inset: 0;
        	display: flex;
        	flex-direction: column;
        	min-width: 0;
        	min-height: 0;
        }

        .instance-surface {
                position: relative;
                display: flex;
                flex: 1;
                min-height: 0;
                min-width: 0;
        }

        .profile-surface {
                position: absolute;
                inset: 0;
                z-index: 40;
                background: var(--bg-main);
        }

        /* The instance tabs and profile own their scrolling, not the shell. */
        .main-content {
                overflow: clip;
        }

        .drag-overlay {
                position: fixed;
                inset: 0;
                z-index: 9999;
                background: var(--bg-overlay);
                display: flex;
                align-items: center;
                justify-content: center;
                backdrop-filter: blur(var(--backdrop-blur-overlay, 4px));
        }

        .drag-overlay-content {
                text-align: center;
                color: var(--media-overlay-text);
        }

        .drag-overlay-content .drag-icon {
                display: flex;
                justify-content: center;
                align-items: center;
                margin-bottom: 16px;
        }

        .drag-overlay-content h2 {
                font-size: 1.5rem;
                margin-bottom: 8px;
        }

        .drag-overlay-content p {
                font-size: 0.9rem;
                opacity: 0.8;
        }

        .sidebar-container {
                --sidebar-edge-inset: 5px;
                position: relative;
                flex-shrink: 0;
                overflow: visible;
                display: flex;
                width: var(--sidebar-size);
                background: var(--bg-sidebar);
                transition: width var(--sidebar-resize-duration) cubic-bezier(0.32, 0.72, 0, 1);
        }

        .sidebar-container:not(.horizontal) { --instance-icon-inset: 6px; }

        .sidebar-container.resizing { transition: none; }

        .sidebar-resize-handle {
                position: absolute;
                z-index: 51;
                touch-action: none;
        }
        .sidebar-resize-handle.left, .sidebar-resize-handle.right {
                inset-block: 0;
                width: 10px;
                cursor: ew-resize;
        }
        .sidebar-resize-handle.left { right: -5px; }
        .sidebar-resize-handle.right { left: -5px; }
        .sidebar-resize-handle.top, .sidebar-resize-handle.bottom {
                inset-inline: 0;
                height: 10px;
                cursor: ns-resize;
        }
        .sidebar-resize-handle.top { bottom: -5px; }
        .sidebar-resize-handle.bottom { top: -5px; }
        .sidebar-resize-handle::after {
                content: "";
                position: absolute;
                border-radius: 4px;
                background: var(--text-muted);
        }
        .sidebar-resize-handle.left::after, .sidebar-resize-handle.right::after {
                inset: calc(50% - 18px) 4px;
        }
        .sidebar-resize-handle.top::after, .sidebar-resize-handle.bottom::after {
                inset: 4px calc(50% - 18px);
        }
        .sidebar-resize-handle:is(:hover, :focus-visible)::after,
        .resizing .sidebar-resize-handle::after {
                background: var(--accent);
        }

        /* ---- Barra acoplada a otro borde ---- */
        .app-container.dock-right {
                flex-direction: row-reverse;
        }

        .app-container.dock-top {
                flex-direction: column;
        }

        .app-container.dock-bottom {
                flex-direction: column-reverse;
        }

        .app-container.dock-top .main-content,
        .app-container.dock-bottom .main-content {
                min-height: 0;
        }

        .sidebar-container.horizontal {
                width: 100%;
                height: var(--sidebar-size);
                transition-property: height;
                z-index: 50;
        }

        /* Превью зон — тот же вид, что у выбора в настройках (DockPicker):
           рамка, полоска внутри; наведённая зона и текущее положение — акцентом. */
        .dock-preview {
                position: fixed;
                z-index: 900;
                box-sizing: border-box;
                pointer-events: none;
                border: 1px solid var(--border-color);
                border-radius: var(--border-radius);
                background: var(--bg-card);
                opacity: 0.35;
                animation: dock-preview-in 0.14s ease-out;
                transition:
                        top 0.18s cubic-bezier(0.2, 0.7, 0.3, 1),
                        left 0.18s cubic-bezier(0.2, 0.7, 0.3, 1);
        }

        .dock-preview .dock-bar {
                position: absolute;
                inset: 6px;
                border-radius: var(--border-radius-sm);
                background: var(--border-color);
                transition: background 0.15s;
        }

        .dock-preview.current:not(.hover) {
                opacity: 0.75;
                border-color: var(--accent);
        }

        .dock-preview.current .dock-bar,
        .dock-preview.hover .dock-bar {
                background: var(--accent);
        }

        .dock-preview.hover {
                opacity: 1;
                border-color: var(--accent);
                background: color-mix(in srgb, var(--accent) 14%, transparent);
        }

        /* Боковые зоны упираются в верхнюю и нижнюю полосы, а не налезают на них. */
        .dock-preview.left,
        .dock-preview.right {
                top: var(--sidebar-bar-height, 176px);
                bottom: var(--sidebar-bar-height, 176px);
                width: var(--sidebar-width);
        }

        .dock-preview.left.compact,
        .dock-preview.right.compact {
                top: var(--sidebar-compact-bar-height, 66px);
                bottom: var(--sidebar-compact-bar-height, 66px);
                width: var(--sidebar-compact-width, 70px);
        }

        .dock-preview.left { left: 0; }
        .dock-preview.right { right: 0; }

        .dock-preview.top-start,
        .dock-preview.top-end,
        .dock-preview.bottom-start,
        .dock-preview.bottom-end {
                height: var(--sidebar-bar-height, 176px);
        }

        .dock-preview.top-start.compact,
        .dock-preview.top-end.compact,
        .dock-preview.bottom-start.compact,
        .dock-preview.bottom-end.compact {
                height: var(--sidebar-compact-bar-height, 66px);
        }

        .dock-preview.top-start { top: 0; left: 0; right: 50%; }
        .dock-preview.top-end { top: 0; left: 50%; right: 0; }
        .dock-preview.bottom-start { bottom: 0; left: 0; right: 50%; }
        .dock-preview.bottom-end { bottom: 0; left: 50%; right: 0; }

        @keyframes dock-preview-in {
                from {
                        opacity: 0;
                }
                to {
                        opacity: 1;
                }
        }

        :global(html.panel-dragging),
        :global(html.panel-dragging *) {
                cursor: grabbing !important;
                user-select: none !important;
        }

        .empty-state {
                position: relative;
                gap: var(--space-md);
        }

        .empty-logo {
                width: 68px;
                height: 68px;
                background: var(--cubic-logo);
                background-size: contain;
                background-repeat: no-repeat;
                background-position: center;
        }
</style>
