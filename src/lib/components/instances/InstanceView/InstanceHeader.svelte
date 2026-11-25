<script lang="ts">
	import { fly } from "svelte/transition";
        import { t } from "$lib/i18n";
        import { invoke } from "@tauri-apps/api/core";
        import { onMount, tick } from "svelte";
        import type { Snippet } from "svelte";
        import { subscribeLogPreview } from "$lib/api/logStream";
        import { onAppEvent } from "$lib/api/launcherService";
        import type { InstanceDto } from "$lib/types/types";
        import { getLoaderLogo, getDisplayIconSrc, getPreviewIconSrc } from "$lib/icons/logos";
        import { showErrorParsed } from "$lib/state/state.svelte";
        import Icon from "$lib/icons/Icon.svelte";
        import { animateWidth } from "$lib/utils/animateWidth";
        import { animDuration } from "$lib/utils/animations";
        import { formatPlaytime, hasPlaytime } from "$lib/utils/playtime";
        import { windowManagerPopover } from "$lib/utils/popoverPlacement";
        import { hasOpenModal } from "$lib/components/layout/ModalBase.svelte";

        let {
                instance,
                bannerState = "Idle",
                launchPending = false,
                downloadKind = null,
                needsDownload = false,
                downloadStarted = false,
                downloadProgress = 0,
                activeSection = $bindable("detalles"),
                onPlay = () => {},
                onStop = () => {},
                onopenlibrary,
                modpack = undefined,
                dock = "top",
                mirrored = false,
        }: {
                instance: InstanceDto;
                bannerState: string;
                /** Экземпляр запускается, окно игры ещё не появилось. */
                launchPending?: boolean;
                downloadKind?: "mods" | "version" | null;
                needsDownload?: boolean;
                downloadStarted?: boolean;
                downloadProgress?: number;
                activeSection: string;
                onPlay: () => void;
                onStop?: () => void;
                onopenlibrary: () => void;
                modpack?: Snippet;
                /** Edge of the window the header is docked to: flips the fade and the border. */
                dock?: "top" | "bottom";
                mirrored?: boolean;
        } = $props();

        const loaderIcon = $derived(getLoaderLogo(instance.loader));
        const isDefaultIcon = $derived(!instance.icon);
        const downloadLabel = $derived(
                t(downloadStarted ? "downloadProgress.statusGeneric" : "downloadProgress.statusConnecting"),
        );
        const buttonDownloading = $derived(
                !!downloadKind &&
                        bannerState !== "Started" &&
                        bannerState !== "Starting",
        );
        /** Пока игра запущена, главная кнопка работает как «Завершить» (как в старом проекте). */
        const isRunning = $derived(bannerState === "Started" && !buttonDownloading);
        const showDownloadAction = $derived(needsDownload && !buttonDownloading && !isRunning);
        const buttonLabel = $derived(
                buttonDownloading
                        ? downloadLabel
                        : isRunning
                                ? t("instanceView.close")
                                : t(needsDownload ? "instanceView.downloadBtn" : "instanceView.playBtn"),
        );
        let processIds = $state<string[]>([]);
        let launchButtonHeight = $state(36);
        let fullHeaderHeight = $state(0);
        const compact = $derived(activeSection !== "detalles");
        let processMenuOpen = $state(false);
        const processDropUp = $derived(dock === "bottom");
        let processAnchor: HTMLButtonElement | undefined = $state();
        let processAnchorSection: string | undefined;
        /**
         * Меню переносится в <body>: у .header-content есть transform, а при нём
         * position: fixed считается относительно шапки, и меню режется её overflow: hidden.
         */
        function portalToBody(node: HTMLElement) {
                document.body.appendChild(node);
                return { destroy() { node.remove(); } };
        }
        let processMenuPos = $state({ top: 0, bottom: 0, left: 0, maxHeight: 0 });

        function positionProcessMenu() {
                const control = processAnchor?.closest(".launch-control");
                if (activeSection !== processAnchorSection || !control?.isConnected || control.closest("[inert]")) { processMenuOpen = false; return; }
                processMenuPos = windowManagerPopover(control.getBoundingClientRect(), {
                        width: window.innerWidth, height: window.innerHeight,
                }, dock);
        }

        async function toggleProcessMenu(event: MouseEvent) {
                if (!processMenuOpen) {
                        processAnchor = event.currentTarget as HTMLButtonElement;
                        processAnchorSection = activeSection;
                }
                processMenuOpen = !processMenuOpen;
                if (processMenuOpen) {
                        await tick();
                        processMenuEl?.focus({ preventScroll: true });
                }
        }
        let processMenuEl: HTMLDivElement | undefined = $state();
        $effect(() => {
                if (!processMenuOpen) return;
                if (!processIds.length) { processMenuOpen = false; return; }
                positionProcessMenu();
                const observer = new ResizeObserver(positionProcessMenu);
                const control = processAnchor?.closest(".launch-control");
                if (control) observer.observe(control);
                return () => observer.disconnect();
        });
        function onProcessWindowPointerDown(e: PointerEvent) {
                const target = e.target as Node;
                if (processMenuEl?.contains(target)) return;
                if ((e.target as HTMLElement).closest?.(".process-toggle")) return;
                processMenuOpen = false;
        }
        function onProcessWindowKeydown(e: KeyboardEvent) {
                if (e.key !== "Escape" || e.defaultPrevented || !processMenuOpen || hasOpenModal()) return;
                e.preventDefault();
                if (!e.repeat) {
                        processMenuOpen = false;
                        processAnchor?.focus({ preventScroll: true });
                }
        }

        // Process exit is reported by the backend at the exact lifecycle transition.
        // Keep one initial snapshot for already-running instances; no interval polling.
        $effect(() => {
                const id = instance.uuid;
                let active = true;
                void invoke<string[]>("get_instance_processes", { uuid: id })
                        .then((ids) => {
                                if (active) processIds = ids;
                        })
                        .catch(() => {
                                // Preserve the last known state if the initial snapshot fails.
                        });
                const unsubscribe = onAppEvent("InstanceProcessesChanged", (payload) => {
                        if (!active || payload.id !== id) return;
                        processIds = Array.isArray(payload.process_ids)
                                ? payload.process_ids as string[]
                                : [];
                        if (processIds.length === 0) processMenuOpen = false;
                });
                return () => {
                        active = false;
                        unsubscribe();
                        processIds = [];
                };
        });
        async function closeProcess(id: string) {
                if (!window.confirm(t("instanceView.confirmCloseProcess"))) return;
                try {
                        await invoke("kill_instance", { uuid: instance.uuid, processId: id });
                        processIds = processIds.filter((processId) => processId !== id);
                        await tick();
                        (processIds.length ? processMenuEl : processAnchor)?.focus({ preventScroll: true });
                } catch (error) {
                        showErrorParsed(error);
                }
        }
        const resizeDuration = $derived(animDuration(220));

        const statusLabel = $derived(
                instance.status === "started"
                        ? t("instanceView.status.started")
                        : instance.status === "starting"
                                ? t("instanceView.status.starting")
                                : instance.status === "error"
                                        ? "Error"
                                        : t("instanceView.status.idle"),
        );
        const statusClass = $derived(
                instance.status === "started"
                        ? "status-started"
                        : instance.status === "starting"
                                ? "status-starting"
                                : instance.status === "error"
                                        ? "status-error"
                                        : "status-idle",
        );

        const playtimeLabel = $derived(
                formatPlaytime(instance.playtime_seconds, t),
        );

        let lastLog = $state("");
        let visible = $state(true);
        onMount(() => {
                const update = () => {
                        visible = !document.hidden;
                };
                update();
                document.addEventListener("visibilitychange", update);
                return () => document.removeEventListener("visibilitychange", update);
        });

        $effect(() => {
                void instance.uuid;
                lastLog = "";
        });

        $effect(() => {
                const id = instance.uuid;
                if (
                        !visible ||
                        activeSection !== "detalles" ||
                        (bannerState !== "Started" && bannerState !== "Starting")
                )
                        return;
                return subscribeLogPreview(id, (line) => {
                        lastLog = line;
                });
        });

        function openDir(subDir?: string) {
                invoke("open_instance_dir", {
                        id: instance.uuid,
                        subDir: subDir ?? null,
                });
        }

        function openLogs() {
                invoke("open_log_window", {
                        instanceId: instance.uuid,
                        instanceName: instance.name,
                });
        }
</script>

<svelte:window onpointerdown={onProcessWindowPointerDown} onkeydown={onProcessWindowKeydown} onresize={() => { if (processMenuOpen) positionProcessMenu(); }} />

{#snippet launchButton(compact: boolean)}
        <div class="launch-control">
                <button
                        type="button"
                        class="btn-primary launch-btn"
                        class:downloading={buttonDownloading}
                        bind:clientHeight={launchButtonHeight}
                        class:retry-download={showDownloadAction}
                        use:animateWidth={resizeDuration}
                        disabled={bannerState === "Starting" || buttonDownloading}
                        onclick={isRunning ? onStop : onPlay}
                        aria-label={`${buttonLabel}${buttonDownloading ? ` ${downloadProgress}%` : ""}${processIds.length ? `, ${t("instanceView.runningWindows", { count: processIds.length })}` : ""}`}
                >
                        <span class="launch-label">
                                {#if !buttonDownloading}
                                        <span class="launch-icon">
                                                <Icon name={showDownloadAction ? "ui:download" : isRunning ? "ui:close" : "ui:play"} size={16} />
                                        </span>
                                {/if}
                                <span>{buttonLabel}</span>
                                {#if buttonDownloading}<span class="download-percent">{downloadProgress}%</span>{/if}
                        </span>
                        {#if buttonDownloading}
                                <svg class="download-border" aria-hidden="true"><rect width="100%" height="100%" rx={Math.min(9, launchButtonHeight / 2)} pathLength="100" stroke-dasharray={`${downloadProgress} 100`} /></svg>
                        {/if}
                        {#if launchPending}
                                <span class="launch-progress" role="status" aria-label={t("instanceView.launching")} title={t("instanceView.launching")}></span>
                        {/if}
                </button>
                {#if processIds.length > 0}
                        <button type="button" class="process-toggle" class:up={processDropUp} aria-label={t("instanceView.runningWindows", { count: processIds.length })} title={t("instanceView.runningWindows", { count: processIds.length })} aria-haspopup="dialog" aria-expanded={processMenuOpen && compact === (activeSection !== "detalles")} onclick={toggleProcessMenu}>
                                <span class="process-count" aria-hidden="true">{processIds.length > 99 ? "99+" : processIds.length}</span>
                                <span class="process-chevron"><Icon name="ui:chevron-down" size={13} /></span>
                        </button>
                {/if}
        </div>
{/snippet}

{#if processMenuOpen && processIds.length > 0}
        <div
                use:portalToBody
                bind:this={processMenuEl}
                class="process-menu"
                class:process-menu--up={processDropUp}
                role="dialog"
                aria-label={t("library.windows")}
                tabindex="-1"
                style:top={processDropUp ? "auto" : `${processMenuPos.top}px`}
                style:bottom={processDropUp ? `${processMenuPos.bottom}px` : "auto"}
                style:left={`${processMenuPos.left}px`}
                style:max-height={`min(360px, 60vh, ${processMenuPos.maxHeight}px)`}
                transition:fly={{ y: processDropUp ? 6 : -6, duration: animDuration(160) }}
        >
                <div class="process-menu-title"><strong>{t("library.windows")}</strong><span>{processIds.length}</span></div>
                <p class="process-instance" title={instance.name}>{instance.name}</p>
                <ul class="process-list">
                        {#each processIds as processId, index (processId)}
                                {@const label = t("instanceView.minecraftWindow", { number: index + 1 })}
                                <li class="process-row">
                                        <span class="process-number" aria-hidden="true">{index + 1}</span>
                                        <span>{label}</span>
                                        <button type="button" class="process-close" aria-label={`${t("instanceView.closeMinecraftWindow")}: ${label}`} title={t("instanceView.closeMinecraftWindow")} onclick={() => closeProcess(processId)}><Icon name="ui:close" size={14} /></button>
                                </li>
                        {/each}
                </ul>
                <button type="button" class="process-launch-more" disabled={bannerState === "Starting" || !!downloadKind}
                        onclick={() => { processMenuOpen = false; processAnchor?.focus({ preventScroll: true }); onPlay(); }}>
                        <Icon name="ui:play" size={14} />
                        <span>{t("instanceView.launchAnother")}</span>
                </button>
        </div>
{/if}

<header
        class="instance-header"
        class:compact
        class:dock-bottom={dock === "bottom"}
        class:mirrored={mirrored}
        style:--header-motion={`${animDuration(220)}ms`}
>
        <div class="header-bg" aria-hidden="true">
                {#if instance.cover_image}
                        {#key instance.cover_image}
                                <img src={getPreviewIconSrc(instance.cover_image)} alt="" decoding="async" onerror={(event) => event.currentTarget.setAttribute("hidden", "")} />
                        {/key}
                {/if}
        </div>
        <div class="header-body" style:height={compact ? "60px" : fullHeaderHeight ? `${fullHeaderHeight}px` : undefined}>
                <div
                        class="header-content"
                        class:visible={!compact}
                        inert={compact}
                        bind:clientHeight={fullHeaderHeight}
                >
                        <button type="button" class="back-library" onclick={onopenlibrary}>
                                <Icon name="ui:chevron-left" size={15} style={mirrored ? "transform: rotate(180deg);" : undefined} /> {t("library.title")}
                        </button>
                        <div class="title-row">
                                <div class="title-left">
                                        {#if isDefaultIcon}
                                                <div
                                                        class="instance-icon instance-icon--accent"
                                                        role="img"
                                                        aria-label={instance.name}
                                                ></div>
                                        {:else}
                                                <img
                                                        class="instance-icon"
                                                        src={getDisplayIconSrc(instance.icon)}
                                                        alt={instance.name}
                                                        width="36"
                                                        height="36"
                                                />
                                        {/if}
                                        <h1 class="instance-title">{instance.name}</h1>
                                </div>
                                {#if dock === "top"}{@render folderActions()}{/if}
                        </div>

                        {#snippet folderActions()}
                                <div class="actions-row">
                                        <button
                                                type="button"
                                                class="action-btn"
                                                onclick={() => openDir()}
                                        >
                                                <Icon name="instance:folder" size={15} />
                                                <span class="action-label"
                                                        >{t("instanceView.options.folder")}</span
                                                >
                                        </button>
                                        <button
                                                type="button"
                                                class="action-btn"
                                                onclick={() => openDir("mods")}
                                        >
                                                <Icon name="instance:grid" size={15} />
                                                <span class="action-label"
                                                        >{t("instanceView.tabs.mods")}</span
                                                >
                                        </button>
                                        <button
                                                type="button"
                                                class="action-btn"
                                                onclick={() => openDir("screenshots")}
                                        >
                                                <Icon name="instance:image" size={15} />
                                                <span class="action-label"
                                                        >{t("instanceView.tabs.screenshots")}</span
                                                >
                                        </button>
                                        <button
                                                type="button"
                                                class="action-btn"
                                                onclick={() => openDir("resourcepacks")}
                                        >
                                                <Icon name="instance:resources" size={15} />
                                                <span class="action-label"
                                                        >{t("instanceView.tabs.resources")}</span
                                                >
                                        </button>
                                        <button
                                                type="button"
                                                class="action-btn"
                                                onclick={() => openLogs()}
                                        >
                                                <Icon name="instance:code" size={15} />
                                                <span class="action-label"
                                                        >{t("instanceView.tabs.logs")}</span
                                                >
                                        </button>
                                </div>
                        {/snippet}

                        <div class="extension-row">
                                <div class="extension-left">
                                        <span class="meta-chip">
                                                <img
                                                        src={loaderIcon}
                                                        alt={instance.loader}
                                                        class="loader-icon"
                                                />
                                                {instance.loader} · {instance.version}
                                        </span>
                                        <span class="meta-sep">·</span>
                                        <span class="inline-status {statusClass}">
                                                {#if bannerState === "Starting"}
                                                        <svg
                                                                class="status-spin"
                                                                width="12"
                                                                height="12"
                                                                viewBox="0 0 24 24"
                                                                fill="none"
                                                                stroke="currentColor"
                                                                stroke-width="2.5"
                                                        >
                                                                <circle
                                                                        cx="12"
                                                                        cy="12"
                                                                        r="10"
                                                                        stroke-dasharray="31.4"
                                                                        stroke-dashoffset="10"
                                                                />
                                                        </svg>
                                                {:else if bannerState === "Started"}
                                                        <Icon name="ui:check" size={12} />
                                                {/if}
                                                <span class="status-text">{statusLabel}</span>
                                                {#if bannerState === "Idle" || bannerState === "Error"}
                                                        <span class="log-snippet"
                                                                >{t("instanceView.status.offlineLog")}</span
                                                        >
                                                {:else}
                                                        <span class="log-snippet" title={lastLog}
                                                                >{lastLog}</span
                                                        >
                                                {/if}
                                        </span>
                                </div>
                                <div class="launch-area" class:dock-controls={dock === "bottom"}>
                                        {@render launchButton(false)}
                                        {#if dock === "bottom"}{@render folderActions()}{/if}
                                </div>
                        </div>

                        <div class="details-row">
                                {#if modpack}{@render modpack()}{/if}
                                <div class="path-row">
                                        <Icon name="instance:folder" size={12} />
                                        <span class="path-text" title={instance.path}
                                                >{instance.path}</span
                                        >
                                        <button
                                                type="button"
                                                class="icon-btn"
                                                onclick={() => openDir()}
                                                title={t("instanceView.details.location")}
                                        >
                                                <Icon name="instance:external-link" size={13} />
                                        </button>
                                </div>
                                {#if hasPlaytime(instance.playtime_seconds)}
                                        <div class="playtime">
                                                <Icon name="instance:clock" size={12} />
                                                <span
                                                        >{t("instanceView.playtime", {
                                                                time: playtimeLabel,
                                                        })}</span
                                                >
                                        </div>
                                {/if}
                        </div>
                </div>
                <div
                        class="compact-content"
                        class:visible={compact}
                        inert={!compact}
                >
                        <button
                                type="button"
                                class="back-btn"
                                aria-label={t("instanceView.tabs.details")}
                                onclick={() => (activeSection = "detalles")}
                        >
                                <Icon name={mirrored ? "ui:chevron-right" : "ui:chevron-left"} size={18} />
                        </button>
                        {#if isDefaultIcon}
                                <div
                                        class="compact-icon compact-icon--accent"
                                        role="img"
                                        aria-label={instance.name}
                                ></div>
                        {:else}
                                <img
                                        class="compact-icon"
                                        src={getDisplayIconSrc(instance.icon)}
                                        alt={instance.name}
                                        width="28"
                                        height="28"
                                />
                        {/if}
                        <div class="compact-title-area">
                                <span class="compact-title">{instance.name}</span>
                                <span class="compact-version">{instance.version}</span>
                        </div>
                        <div class="compact-spacer"></div>
                        <div class="launch-area">
                                {@render launchButton(true)}
                        </div>
                </div>
        </div>
</header>

<style>
        .instance-header {
                position: relative;
                flex-shrink: 0;
                /* Не перекрывает быстрый переключатель аккаунтов в боковой панели. */
                z-index: 1;
                container-type: inline-size;
                isolation: isolate;
                margin: 12px 14px 8px;
                border: 1px solid var(--border);
                border-radius: var(--launcher-radius);
                overflow: clip;
        }
        .header-body {
                position: relative;
                display: grid;
                grid-template-rows: max-content;
                overflow: clip;
                transition: height var(--header-motion) cubic-bezier(0.25, 0.8, 0.25, 1);
        }

        .header-bg {
                position: absolute;
                inset: 0;
                z-index: -1;
                pointer-events: none;
                overflow: hidden;
                background: var(--launcher-surface);
        }
        .header-bg img {
                position: absolute;
                inset: 0;
                width: 100%;
                height: 100%;
                object-fit: cover;
                opacity: .22;
                mask-image: linear-gradient(90deg, transparent 15%, black);
                transition: opacity var(--header-motion) ease;
        }
        .compact .header-bg img { opacity: 0; }
        .mirrored .header-bg img { mask-image: linear-gradient(270deg, transparent 15%, black); }
        :global(html[data-no-background-image]) .header-bg img { display: none; }
        .instance-header.dock-bottom { margin-block: 4px 8px; }
        .dock-bottom .header-body { align-content: end; }
        .dock-bottom .compact-content { inset: auto 0 0; }
        /* Акцентное свечение следует за положением панели: сторона старта сверху,
           а для нижней панели — снизу. */
        .instance-header { --glow-x: 85%; --glow-y: -20%; }
        .instance-header.mirrored { --glow-x: 15%; --header-direction: -1; }
        .mirrored :is(.header-content, .compact-content) { direction: rtl; }
        .mirrored .path-text { direction: ltr; text-align: right; }
        .instance-header.dock-bottom { --glow-y: 120%; }

        .back-library {
                display: inline-flex; align-items: center; gap: 5px; align-self: flex-start;
                border: 0; border-radius: var(--launcher-control-radius); background: transparent;
                color: var(--text-secondary); padding: 5px 7px; margin-inline-start: -7px;
                cursor: pointer; font: inherit; font-size: .75rem; font-weight: 700;
        }
        .back-library:hover { background: var(--launcher-selected); color: var(--text-primary); }
        .header-content {
                transition:
                        transform var(--header-motion) ease,
                        opacity var(--header-motion) ease;
                opacity: 1;
                transform: translateX(0);
        }
        .header-content:not(.visible) {
                opacity: 0;
                transform: translateX(calc(-8px * var(--header-direction, 1)));
                pointer-events: none;
        }

        .title-row {
                display: flex;
                align-items: center;
                justify-content: space-between;
                gap: 10px;
        }

        .title-left {
                display: flex;
                align-items: center;
                gap: 8px;
                min-width: 0;
        }

        .instance-icon {
                width: 56px;
                height: 56px;
                border-radius: var(--launcher-control-radius);
                object-fit: contain;
                flex-shrink: 0;
        }

        .instance-title {
                font-size: clamp(1.4rem, 4cqi, 2.7rem);
                font-weight: var(--font-weight-bold);
                color: var(--text-primary);
                margin: 0;
                letter-spacing: -.045em;
                white-space: nowrap;
                overflow: hidden;
                text-overflow: ellipsis;
        }

        .extension-row {
                display: flex;
                align-items: center;
                justify-content: space-between;
                gap: 16px;
        }

        .extension-left {
                display: flex;
                align-items: center;
                gap: 6px;
                min-width: 0;
                overflow: hidden;
        }

        .meta-chip {
                display: inline-flex;
                align-items: center;
                gap: 5px;
                font-size: 0.7rem;
                font-weight: 600;
                padding: 3px 8px;
                border-radius: 5px;
                background: var(--bg-card);
                color: var(--text-secondary);
                border: 1px solid var(--border);
                white-space: nowrap;
        }

        .loader-icon {
                width: 14px;
                height: 14px;
                object-fit: contain;
                flex-shrink: 0;
                border-radius: 2px;
        }

        .meta-sep {
                color: var(--text-tertiary);
                font-size: 0.7rem;
        }

        .inline-status {
                display: flex;
                align-items: center;
                gap: 5px;
                min-width: 0;
                overflow: hidden;
        }

        .inline-status.status-idle {
                color: var(--text-tertiary);
        }

        .inline-status.status-starting {
                color: var(--color-status-starting);
        }

        .inline-status.status-started {
                color: var(--color-status-started);
        }

        .inline-status.status-error {
                color: var(--color-error);
        }

        .inline-status .status-text {
                font-size: 0.7rem;
                font-weight: 600;
                white-space: nowrap;
                flex-shrink: 0;
        }

        .log-snippet {
                font-size: 0.65rem;
                font-weight: 500;
                color: inherit;
                opacity: 0;
                max-width: 0;
                white-space: nowrap;
                overflow: hidden;
                text-overflow: ellipsis;
                transition:
                        max-width 0.25s ease,
                        opacity 0.25s ease,
                         margin-inline-start 0.25s ease;
                pointer-events: none;
        }

        .inline-status:hover .log-snippet {
                opacity: 0.7;
                max-width: 360px;
                margin-inline-start: 2px;
        }

        .status-spin {
                animation: spin 1.2s linear infinite;
        }

        @keyframes spin {
                from {
                        transform: rotate(0deg);
                }
                to {
                        transform: rotate(-360deg);
                }
        }

        .launch-control { display: flex; align-items: stretch; gap: 4px; }
        /* Бегающая полоска у нижнего края кнопки «Играть», пока окно Minecraft не появилось.
           Внутри кнопки, а не снаружи: шапка обрезает всё, что выходит за её границы. */
        .launch-btn { position: relative; overflow: hidden; }
        .launch-progress {
                position: absolute;
                left: 0;
                right: 0;
                bottom: 0;
                height: 3px;
                overflow: hidden;
                pointer-events: none;
                background: rgba(255, 255, 255, 0.22);
        }
        .launch-progress::before {
                content: "";
                position: absolute;
                top: 0;
                bottom: 0;
                left: -40%;
                width: 40%;
                background: rgba(255, 255, 255, 0.9);
                animation: launch-sweep 1.1s ease-in-out infinite;
        }
        @keyframes launch-sweep {
                from { left: -40%; }
                to { left: 100%; }
        }
        @media (prefers-reduced-motion: reduce) {
                .launch-progress::before { animation: none; left: 30%; }
        }
        .process-toggle { min-width: 43px; padding: 0 6px; display: flex; gap: 4px; align-items: center; justify-content: center; border: 1px solid var(--border); background: var(--launcher-surface); color: var(--text-secondary); border-radius: var(--launcher-control-radius); cursor: pointer; }
        .process-toggle:hover, .process-toggle[aria-expanded="true"] { color: var(--text-primary); background: var(--launcher-selected); }
        .process-toggle:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
        .process-chevron { display: grid; place-items: center; }
        .process-toggle.up .process-chevron { transform: rotate(180deg); }
        .process-count { font-size: .78rem; font-weight: 750; font-variant-numeric: tabular-nums; }
        .process-menu { position: fixed; z-index: 200; transform-origin: top center; overflow-y: auto; overscroll-behavior: contain; box-sizing: border-box; width: min(220px, calc(100vw - 16px)); padding: 10px; border: 1px solid var(--border); border-radius: var(--launcher-radius); background: var(--bg-main); box-shadow: 0 12px 32px #0006; }
        .process-menu.process-menu--up { transform-origin: bottom center; }
        .process-menu-title { display: flex; align-items: center; justify-content: space-between; gap: 6px; color: var(--text-primary); font-size: .9rem; }
        .process-menu-title span, .process-number { padding: 3px 6px; border-radius: 6px; color: var(--text-primary); background: var(--launcher-selected); font-size: .68rem; }
        .process-instance { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin: 3px 0 9px; color: var(--text-secondary); font-size: .73rem; }
        .process-list { display: grid; gap: 4px; margin: 0; padding: 0; list-style: none; }
        .process-row { display: grid; grid-template-columns: auto 1fr auto; align-items: center; gap: 6px; padding: 5px 6px; background: var(--launcher-surface); border-radius: var(--launcher-control-radius); color: var(--text-primary); font-size: .76rem; }
        .process-close { display: grid; place-items: center; width: 29px; height: 29px; border: 0; border-radius: 6px; padding: 0; color: var(--text-secondary); background: transparent; cursor: pointer; }
        .process-launch-more { display: flex; align-items: center; justify-content: center; gap: 6px; width: 100%; margin-top: 8px; padding: 9px 6px; border: 0; border-radius: var(--launcher-control-radius); background: var(--accent); color: var(--accent-text); font: inherit; font-size: .76rem; font-weight: 700; cursor: pointer; }
        .process-launch-more:hover:not(:disabled) { background: var(--accent-hover); }
        .process-launch-more:disabled { opacity: .55; cursor: not-allowed; }
        .process-close:hover { color: var(--danger, #ef5555); background: color-mix(in srgb, var(--danger, #ef5555) 12%, transparent); }
        .launch-area {
                flex-shrink: 0;
        }

        .launch-btn {
                box-sizing: border-box;
                display: inline-flex;
                align-items: center;
                justify-content: center;
                gap: 8px;
                background: var(--accent);
                color: var(--accent-text);
                border: 1px solid transparent;
                min-width: clamp(94px, 21cqi, 162px);
                min-height: 42px;
                padding: 8px clamp(9px, 2.5cqi, 22px);
                border-radius: var(--launcher-control-radius);
                font-size: 0.9rem;
                font-weight: 800;
                text-transform: uppercase;
                cursor: pointer;
                box-shadow: var(--play-button-shadow, none);
                transition:
                        background 0.2s ease,
                        box-shadow 0.2s ease;
        }

        .launch-btn:hover:not(:disabled) {
                background: var(--accent-hover);
                box-shadow: var(--play-button-shadow-hover, none);
        }

        .launch-btn:active:not(:disabled) {
                transform: scale(0.98);
        }

        .launch-btn.retry-download:not(:disabled) {
                background: transparent;
                color: var(--text-primary);
                border: 2px solid var(--accent);
                box-shadow: none;
        }

        .launch-btn.retry-download:hover:not(:disabled) {
                background: transparent;
                border-color: var(--accent-hover, var(--accent));
                box-shadow: none;
        }

        .launch-btn:disabled {
                opacity: 1;
                background: var(--surface-hover);
                color: var(--text-muted);
                cursor: not-allowed;
                box-shadow: none;
                border: 1px solid var(--border);
        }

        .launch-btn.downloading {
                position: relative;
                color: var(--text-secondary);
        }

        .launch-label {
                position: relative;
                display: inline-flex;
                align-items: center;
                gap: 7px;
                width: max-content;
                flex-shrink: 0;
                white-space: nowrap;
        }

        .launch-icon {
                display: flex;
        }

        .downloading .launch-label {
                padding-inline-end: 4ch;
        }

        .download-border {
                position: absolute;
                inset: 0;
                width: 100%;
                height: 100%;
                overflow: visible;
                pointer-events: none;
        }

        .download-border rect {
                fill: none;
                stroke: var(--accent);
                stroke-width: 4px;
        }

        .download-percent {
                position: absolute;
                inset-inline-end: 0;
                inline-size: 4ch;
                text-align: start;
                font-variant-numeric: tabular-nums;
                color: var(--accent);
        }

        .details-row {
                display: flex;
                flex-wrap: nowrap;
                align-items: center;
                gap: 12px;
                min-width: 0;
        }

        .path-row {
                display: flex;
                align-items: center;
                gap: 5px;
                min-width: 0;
                color: var(--text-tertiary);
                font-size: 0.65rem;
        }

        .path-text {
                white-space: nowrap;
                overflow: hidden;
                text-overflow: ellipsis;
                max-width: 300px;
        }

        .icon-btn {
                background: transparent;
                border: none;
                color: var(--text-tertiary);
                cursor: pointer;
                padding: 2px;
                display: flex;
                align-items: center;
                transition: color 0.15s;
                flex-shrink: 0;
        }

        .icon-btn:hover {
                color: var(--text-primary);
        }

        .playtime {
                display: flex;
                align-items: center;
                gap: 5px;
                font-size: 0.65rem;
                color: var(--text-tertiary);
                white-space: nowrap;
        }

        .playtime :global(.icon-svg) {
                flex-shrink: 0;
        }

        .actions-row {
                display: flex;
                gap: 4px;
        }

        .action-btn {
                height: 30px;
                padding: 0 6px;
                border-radius: 7px;
                background: transparent;
                border: 1px solid var(--border);
                color: var(--text-tertiary);
                cursor: pointer;
                display: flex;
                align-items: center;
                justify-content: center;
                transition: all 0.15s ease;
        }

        .action-btn :global(.icon-svg) {
                flex-shrink: 0;
        }

        .action-btn:hover {
                background: var(--surface-hover);
                color: var(--text-primary);
                border-color: var(--border-hover);
        }

        .action-label {
                font-size: 0.68rem;
                font-weight: 500;
                white-space: nowrap;
                min-width: 0;
                max-width: 0;
                margin-inline-start: 0;
                opacity: 0;
                overflow: hidden;
                transition:
                        max-width 0.4s cubic-bezier(0.4, 0, 0.2, 1),
                        opacity 0.4s cubic-bezier(0.4, 0, 0.2, 1);
                pointer-events: none;
        }

        .action-btn:hover .action-label {
                max-width: 120px;
                margin-inline-start: 4px;
                opacity: 1;
        }

        .compact-content {
                position: absolute;
                inset: 0 0 auto;
                height: 60px;
                display: flex;
                align-items: center;
                gap: 8px;
                padding: 8px 14px;
                transition:
                        transform var(--header-motion) ease,
                        opacity var(--header-motion) ease;
                opacity: 1;
                transform: translateX(0);
        }
        .compact-content:not(.visible) {
                opacity: 0;
                transform: translateX(calc(8px * var(--header-direction, 1)));
                pointer-events: none;
        }

        /* Subview controls follow the same reading order as the full header. */
        .back-btn {
                width: 32px;
                height: 32px;
                flex-shrink: 0;
                display: grid;
                place-items: center;
                padding: 0;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                background: transparent;
                color: var(--text-tertiary);
                font-family: inherit;
                cursor: pointer;
                transition:
                        color 0.15s,
                        border-color 0.15s,
                        background 0.15s;
        }
        .back-btn:hover {
                color: var(--text-primary);
                border-color: var(--border-hover);
                background: var(--surface-hover);
        }
        .back-btn:focus-visible {
                outline: 2px solid var(--accent);
                outline-offset: 2px;
        }
        .compact-spacer {
                flex: 1 1 auto;
                min-width: 8px;
        }

        /* Основная шапка — вертикальная: заголовок, версия/лоадер,
           кнопки папок и сведения об игре идут отдельными строками. */
        .instance-header .title-row {
                display: flex;
                align-items: center;
                justify-content: space-between;
                gap: 10px;
        }
        .instance-header .actions-row { display: flex; flex-wrap: wrap; gap: 4px; }

        .dock-controls {
                display: flex;
                flex-direction: column;
                align-items: center;
                gap: 6px;
                max-width: 100%;
        }
        .dock-controls .actions-row { justify-content: center; }
        .instance-header .extension-row { display: flex; align-items: center; gap: 10px; }
        .instance-header .details-row { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
        .instance-header.compact .launch-area { align-self: center; }

        /* Нижняя панель: та же компактная плотность, что и у верхней. */
        .instance-header.dock-bottom .header-content {
                padding: 4px 12px 8px;
        }
        .instance-header.dock-bottom .title-row,
        .instance-header.dock-bottom .extension-row,
        .instance-header.dock-bottom .details-row {
                gap: 8px;
        }
        .instance-header.dock-bottom .dock-controls {
                gap: 4px;
        }
        .instance-header.dock-bottom .dock-controls .actions-row {
                gap: 2px;
        }

        /* Раскладка полной шапки по сетке: кнопка «Библиотека» и инструменты — на одной строке,
           название и «Играть» — на следующей. Строки-обёртки становятся прозрачными для сетки. */
        .instance-header .header-content {
                display: grid;
                padding: clamp(8px, 1.6cqi, 16px);
                grid-template-columns: minmax(0, 1fr) auto;
                grid-template-areas:
                        "back actions"
                        "title launch"
                        "details details"
                        "ext ext";
                align-items: center;
                align-content: start;
                column-gap: 12px;
                row-gap: 6px;
        }
        .instance-header .title-row,
        .instance-header .extension-row {
                display: contents;
        }
        .header-content .back-library { grid-area: back; justify-self: start; margin-block: -5px; }
        .header-content .title-left { grid-area: title; min-width: 0; }
        .header-content .extension-left { grid-area: ext; min-width: 0; }
        .header-content .details-row { grid-area: details; }
        .header-content .launch-area { display: contents; }
        .header-content .launch-control { grid-area: launch; justify-self: end; }
        .header-content .actions-row { grid-area: actions; justify-self: end; }

        /* Верхняя панель: сокращаем зазор между «Библиотекой» и названием, как в нижней. */
        .instance-header:not(.dock-bottom) .header-content {
                row-gap: 4px;
        }
        .instance-header:not(.dock-bottom) .title-left {
                margin-top: -2px;
        }

        /* Нижняя панель: «Библиотека» и инструменты уходят в нижнюю строку. */
        .instance-header.dock-bottom .header-content {
                grid-template-areas:
                        "details details"
                        "ext ext"
                        "title launch"
                        "back actions";
        }

        @container (max-width: 510px) {
                .instance-header .title-row { flex-wrap: wrap; gap: 8px; }
                .instance-icon { width: 42px; height: 42px; }
                .header-content .extension-row { flex-wrap: wrap; gap: 8px; }
                .compact-content { padding: 6px 9px; gap: 6px; }
                .compact-spacer { min-width: 0; }
                .action-btn:hover .action-label { max-width: 0; margin-inline-start: 0; opacity: 0; }
                .path-text {
                        max-width: 160px;
                }
        }
</style>
