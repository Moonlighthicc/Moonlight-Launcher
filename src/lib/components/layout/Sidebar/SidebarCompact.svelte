<script lang="ts">
        import { instanceNav } from "$lib/state/instanceNavState.svelte";
        import { getActiveUser } from "$lib/api/launcherService";
        import { pinInstance } from "$lib/api/cubicApi";
        import { launcherStore } from "$lib/state/state.svelte";
        import { accountAvatar } from "$lib/state/avatarCache.svelte";
        import {
                getActiveDownloadCount,
                downloadQueueUi,
        } from "$lib/state/downloadQueueState.svelte";
        import type { InstanceDto } from "$lib/types/types";
        import { t } from "$lib/i18n";
        import { horizontalWheel } from "$lib/utils/horizontalWheel";
        import { fitText } from "$lib/utils/fitText";
        import { getAccountProvider } from "$lib/utils/accountProviders";
        import AccountSwitcher, {
                type SwitcherPlacement,
        } from "./AccountSwitcher.svelte";
        import { onDestroy } from "svelte";
        import { flip } from "svelte/animate";
        import { fly } from "svelte/transition";
        import { animDuration } from "$lib/utils/animations";
        import { sidebarPopover } from "$lib/utils/popoverPlacement";
        import type { Component, Snippet } from "svelte";
        import { getDisplayIconSrc } from "$lib/icons/logos";
        import Icon from "$lib/icons/Icon.svelte";
        import AccountSwitchChevron from "./AccountSwitchChevron.svelte";
        import AccountAvatar from "$lib/components/profiles/AccountAvatar.svelte";
        import DownloadQueue from "../DownloadQueue/DownloadQueue.svelte";
        import VirtualList from "$lib/components/layout/VirtualList.svelte";
        import InstanceDragGhost from "./InstanceDragGhost.svelte";
        import { InstanceDrag, DRAG_EDGE } from "$lib/utils/instanceDrag.svelte";
        import {
                appendToPinOrder,
                commitManualOrder,
                commitPinOrder,
                isHorizontalDock,
                orderInstances,
                rememberFirstSeen,
                removeFromPinOrder,
                type SidebarAlign,
                type SidebarDock,
        } from "$lib/state/sidebarLayoutState.svelte";

        interface Props {
                selectedInstance: InstanceDto | null;
                onopenquickmenu?: () => void;
                onopenprofileview?: () => void;
                onopenlibrary: () => void;
                oninstancemenu: (event: MouseEvent) => void;
                onopencreateinstance?: () => void;
                onopenversiondownloader?: () => void;
                onselectinstance?: () => void;
                /** Vista de cuentas abierta: elegir una instancia solo cierra esa vista, sin deseleccionarla. */
                profileOpen?: boolean;
                /** Borde al que está acoplada la barra. */
                dock?: SidebarDock;
                /** Половина верхнего/нижнего края: end использует зеркальный порядок. */
                align?: SidebarAlign;
                /** Se llama al agarrar la barra por un hueco para moverla de borde. */
                onpaneldrag?: (event: PointerEvent) => void;
        }

        let {
                selectedInstance = $bindable(),
                onopenquickmenu,
                onopenprofileview,
                onopenlibrary,
                oninstancemenu,
                onopencreateinstance,
                onopenversiondownloader,
                onselectinstance,
                profileOpen = false,
                dock = "left",
                align = "start",
                onpaneldrag,
        }: Props = $props();

        const horizontal = $derived(isHorizontalDock(dock));
        const tooltipPlacement = $derived(
                dock === "left"
                        ? "right"
                        : dock === "right"
                                ? "left"
                                : dock === "top"
                                        ? "bottom"
                                        : "top",
        );

        let asideEl: HTMLElement | undefined = $state();
        let listEl: HTMLElement | undefined = $state();
        let hlistEl: HTMLElement | undefined = $state();

        /** Минимум для списка экземпляров, чтобы полоса не схлопывалась полностью. */
        const MIN_BAR_CONTENT = 160;
        /**
         * Горизонтальная панель: если инструменты и профиль не вмещаются, уменьшаем их
         * масштаб через --ctl, а не растягиваем окно. Ширины считаются по расположению
         * (offsetWidth при --ctl = 1), без CSS zoom, который искажает измерения.
         */
        function fitBar() {
                const el = asideEl;
                if (!horizontal || !el) return;
                el.style.setProperty("--ctl", "1");
                const header = el.querySelector<HTMLElement>(".sc-header");
                const tools = el.querySelector<HTMLElement>(".sc-tools");
                const user = el.querySelector<HTMLElement>(".sc-user");
                if (!header || !tools || !user) return;
                const cs = getComputedStyle(el);
                const pad = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
                const gap = parseFloat(cs.columnGap) || 0;
                const available = el.clientWidth - pad - gap * 3 - MIN_BAR_CONTENT;
                const natural = tools.offsetWidth + user.offsetWidth;
                const room = available - header.offsetWidth;
                const scale = natural > room ? Math.max(0.5, room / natural) : 1;
                el.style.setProperty("--ctl", scale.toFixed(3));
        }
        $effect(() => {
                if (asideEl && horizontal) fitBar();
        });

        $effect(() => {
                rememberFirstSeen(launcherStore.loadedInstances);
        });

        const ordered = $derived(orderInstances(launcherStore.loadedInstances));


        function scroller(): HTMLElement | null {
                return horizontal
                        ? (hlistEl ?? null)
                        : (listEl?.querySelector<HTMLElement>(".virtual-list-container") ??
                                        null);
        }

        /** Индекс в `ordered`. Порядок не меняется: зеркалится блок целиком (CSS). */
        function indexAt(x: number, y: number): number {
                const count = ordered.length;
                const box = scroller();
                if (!box || count === 0) return 0;
                const rect = box.getBoundingClientRect();
                if (!horizontal) {
                        const wrapper = box.querySelector<HTMLElement>(
                                ".virtual-list-item-wrapper",
                        );
                        const rowH = wrapper?.getBoundingClientRect().height || 50;
                        const raw = Math.floor((y - rect.top + box.scrollTop) / rowH);
                        return Math.min(count - 1, Math.max(0, raw));
                }
                const first = box.querySelector<HTMLElement>("[data-instance-uuid]");
                if (!first) return 0;
                const cell = first.getBoundingClientRect();
                const step = cell.width + 8;
                // В зеркальном режиме первая карточка стоит у правого края (RTL): считаем от него.
                // Работаем с координатами экрана, поэтому прокрутка не влияет на результат.
                const raw =
                        align === "end"
                                ? Math.floor((cell.right - x + 4) / step)
                                : Math.floor((x - cell.left + 4) / step);
                return Math.min(count - 1, Math.max(0, raw));
        }

        function inPinZone(x: number, y: number): boolean {
                if (!asideEl || !listEl) return false;
                const bar = asideEl.getBoundingClientRect();
                const list = listEl.getBoundingClientRect();
                if (x < bar.left || x > bar.right || y < bar.top || y > bar.bottom)
                        return false;
                if (horizontal) return align === "end" ? x > list.right : x < list.left;
                return y < list.top;
        }

        function autoScroll(x: number, y: number): boolean {
                const box = scroller();
                if (!box) return false;
                const rect = box.getBoundingClientRect();
                const { zone, speed } = DRAG_EDGE;
                if (horizontal) {
                        const before = box.scrollLeft;
                        if (x < rect.left + zone) box.scrollLeft -= speed;
                        else if (x > rect.right - zone) box.scrollLeft += speed;
                        return box.scrollLeft !== before;
                }
                const before = box.scrollTop;
                if (y < rect.top + zone) box.scrollTop -= speed;
                else if (y > rect.bottom - zone) box.scrollTop += speed;
                return box.scrollTop !== before;
        }

        function pinnedUuids(except?: string): string[] {
                return ordered
                        .filter((i) => i.pinned && i.uuid !== except)
                        .map((i) => i.uuid);
        }

        async function setPinned(instance: InstanceDto, pinned: boolean) {
                if (pinned) appendToPinOrder(instance.uuid, pinnedUuids(instance.uuid));
                else removeFromPinOrder(instance.uuid);
                await pinInstance(instance.uuid, pinned);
        }

        const drag = new InstanceDrag({
                list: () => ordered,
                indexAt,
                inPinZone,
                autoScroll,
                onreorder: (instance, next) => {
                        if (instance.pinned)
                                commitPinOrder(next.filter((i) => i.pinned).map((i) => i.uuid));
                        else
                                commitManualOrder(
                                        next.filter((i) => !i.pinned).map((i) => i.uuid),
                                );
                },
                onpinchange: async (instance, pinned) => {
                        const previous = instance.pinned;
                        instance.pinned = pinned;
                        try {
                                await setPinned(instance, pinned);
                        } catch {
                                instance.pinned = previous;
                        }
                },
        });
        const shown = $derived(drag.display);
        onDestroy(() => drag.destroy());

        function grabPanel(event: PointerEvent) {
                const target = event.target as HTMLElement;
                if (target.closest("[data-instance-uuid], button, a, input")) return;
                onpaneldrag?.(event);
        }

        let activeUser = $derived(getActiveUser());
        let activeDownloads = $derived(getActiveDownloadCount());
        $effect(() => {
                // Не раскрывать очередь автоматически после возврата из compact-режима.
                downloadQueueUi.lastActiveCount = activeDownloads;
        });
        let plusMenuOpen = $state(false);
        let plusButtonEl: HTMLButtonElement | undefined = $state();
        let plusMenuEl = $state<HTMLDivElement>();
        const queuePopoverSide = $derived(horizontal ? "none" : dock === "right" ? "left" : "right");
        const queuePopoverUp = $derived(dock === "bottom");
        const queuePopoverRight = $derived(horizontal && align === "end");

        function positionPlusMenu(node: HTMLElement) {
                const viewport = { width: window.innerWidth, height: window.innerHeight };
                const maxHeight = Math.max(0, viewport.height - 16);
                const anchor = plusButtonEl?.getBoundingClientRect();
                if (!anchor) return { x: 0, y: 0 };
                const position = sidebarPopover(anchor, viewport,
                        { width: node.offsetWidth, height: Math.min(node.scrollHeight + 2, maxHeight), maxHeight },
                        { side: queuePopoverSide, up: queuePopoverUp, alignRight: queuePopoverRight });
                node.style.left = `${position.left}px`;
                node.style.top = position.top === undefined ? "" : `${position.top}px`;
                node.style.bottom = position.bottom === undefined ? "" : `${position.bottom}px`;
                node.style.maxHeight = `${position.maxHeight}px`;
                return { x: position.x, y: position.y };
        }

        function plusMenuFly(node: HTMLElement) {
                const offset = positionPlusMenu(node);
                return fly(node, { ...offset, duration: animDuration(150) });
        }

        function portalPlusMenu(node: HTMLElement) {
                document.body.appendChild(node);
                return { destroy: () => node.remove() };
        }

        $effect(() => {
                const panel = plusMenuEl;
                const anchor = plusButtonEl;
                if (!plusMenuOpen || !panel || !anchor) return;
                const update = () => { positionPlusMenu(panel); };
                update();
                const observer = new ResizeObserver(update);
                observer.observe(panel);
                observer.observe(anchor);
                window.addEventListener("resize", update);
                window.addEventListener("scroll", update, true);
                return () => {
                        observer.disconnect();
                        window.removeEventListener("resize", update);
                        window.removeEventListener("scroll", update, true);
                };
        });

        function closePlusMenuOutside(event: PointerEvent) {
                const target = event.target as Node;
                if (plusButtonEl?.contains(target)) return;
                if (plusMenuEl?.contains(target)) return;
                plusMenuOpen = false;
        }

        interface TooltipProps {
                open?: boolean;
                x?: number;
                y?: number;
                placement?: "right" | "left" | "top" | "bottom";
                children?: Snippet;
        }

        let Tooltip = $state<Component<TooltipProps, Record<string, never>> | null>(
                null,
        );
        let tooltipLoadPromise: Promise<void> | null = null;
        let loadingQueue: Promise<void> = Promise.resolve();

        let tooltipInstance = $state<InstanceDto | null>(null);
        let tooltipOpen = $state(false);
        let tooltipX = $state(0);
        let tooltipY = $state(0);

        function queueLoad(load: () => Promise<void>): Promise<void> {
                loadingQueue = loadingQueue.then(load).catch(() => {});
                return loadingQueue;
        }

        function startTooltipImport(): Promise<void> {
                tooltipLoadPromise = import("$lib/components/ui/Tooltip.svelte").then(
                        (mod) => {
                                Tooltip = mod.default;
                        },
                );
                return tooltipLoadPromise;
        }

        async function loadTooltip() {
                return queueLoad(async () => {
                        if (Tooltip) return;
                        if (tooltipLoadPromise) {
                                await tooltipLoadPromise;
                                return;
                        }
                        await startTooltipImport();
                });
        }

        function prefetchCompactOverlays() {
                if (typeof requestIdleCallback !== "undefined") {
                        requestIdleCallback(() => {
                                void loadTooltip();
                        });
                } else {
                        setTimeout(() => {
                                void loadTooltip();
                        }, 200);
                }
        }

        async function showTooltip(
                e: MouseEvent | FocusEvent,
                instance: InstanceDto,
        ) {
                const target = e.currentTarget as HTMLElement;
                const rect = target.getBoundingClientRect();
                tooltipX = dock === "right" ? rect.left : horizontal ? rect.left : rect.right;
                tooltipY = dock === "top" ? rect.bottom : rect.top;
                tooltipInstance = instance;
                tooltipOpen = true;
                await loadTooltip();
        }

        function hideTooltip() {
                tooltipOpen = false;
        }

        prefetchCompactOverlays();

        let username = $derived(activeUser?.username ?? "Steve");
        const providerLabel = $derived(t(`userMenu.short.${activeUser ? getAccountProvider(activeUser) : "offline"}`));
        let switcherOpen = $state(false);
        let userEl: HTMLElement | undefined = $state();
        const switcherPlacement = $derived<SwitcherPlacement>(
                dock === "top" ? "down" : "up",
        );

        const avatarSvg = $derived(accountAvatar(activeUser));

        function selectInstance(instance: InstanceDto) {
                const perform = () => {
                        // Con un submenú de "Datos" abierto, el primer clic sobre la instancia activa
                        // solo vuelve a su pantalla; recién el siguiente la deselecciona.
                        if (selectedInstance?.uuid === instance.uuid && instanceNav.section !== "detalles") {
                                instanceNav.section = "detalles";
                                onselectinstance?.();
                                return;
                        }
                        if (profileOpen) {
                                selectedInstance = instance;
                                onselectinstance?.();
                                return;
                        }
                        selectedInstance = selectedInstance?.uuid === instance.uuid ? null : instance;
                        onselectinstance?.();
                };
                if (instanceNav.editActionGuard?.(perform)) return;
                perform();
        }
</script>

{#snippet instanceButton(instance: InstanceDto)}
        <div class="sc-instance-item-wrapper">
                <button
                        type="button"
                        class="sc-instance-item"
                        class:dragging={drag.active && drag.uuid === instance.uuid}
                        onpointerdown={(e) => drag.start(e, instance)}
                        ondragstart={(e) => e.preventDefault()}
                        class:active={selectedInstance?.uuid ===
                                instance.uuid}
                        data-instance-uuid={instance.uuid}
                        onmouseenter={(e) => !drag.active && showTooltip(e, instance)}
                        onmouseleave={hideTooltip}
                        onfocus={(e) => showTooltip(e, instance)}
                        onblur={hideTooltip}
                        onclick={() => selectInstance(instance)}
                >
                        {#if instance.icon}
                                <img
                                        src={getDisplayIconSrc(instance.icon)}
                                        alt={instance.name}
                                        loading="lazy"
                                        decoding="async"
                                        width="32"
                                        height="32"
                                        draggable="false"
                                        ondragstart={(e) => e.preventDefault()}
                                />
                        {:else}
                                <Icon name="brand:cubic" size={32} />
                        {/if}
                        {#if instance.status === "started"}
                                <span
                                        class="sc-instance-running"
                                        aria-label={t(
                                                "instanceView.status.started",
                                        )}
                                ></span>
                        {/if}
                        {#if instance.pinned}
                                <span class="sc-instance-pin"
                                        ><Icon name="ui:pin" size={12} /></span
                                >
                        {/if}
                </button>
        </div>

{/snippet}

<svelte:window onresize={fitBar} onpointerdown={closePlusMenuOutside} onkeydown={(e) => { if (e.key === "Escape") plusMenuOpen = false; }} />

<aside
        class="sidebar-compact dock-{dock}"
        class:horizontal
        class:mirrored={align === "end"}
        bind:this={asideEl}
        data-part="sidebar-panel"
        data-dock={dock}
>
        <div
                class="sc-header"
                class:pin-target={drag.active}
                class:hot={drag.pinZone}
                data-tutorial="sidebar-header"
                role="presentation"
                onpointerdown={grabPanel}
                title={drag.active && drag.dragged
                        ? drag.dragged.pinned
                                ? t("sidebar.unpinDrop")
                                : t("sidebar.pinDrop")
                        : undefined}
        >
                {#if drag.active}
                        <span class="sc-pin-zone"><Icon name="ui:pin" size={16} /></span>
                {:else}
                        <button type="button" class="library-nav" aria-current={!selectedInstance && !profileOpen ? "page" : undefined}
                                title={t("library.title")} aria-label={t("library.title")} onclick={onopenlibrary}>
                                <Icon name="instance:grid" size={18} />
                        </button>
                {/if}
        </div>

        <div class="sc-content">
                <div
                        class="sc-instance-list"
                        role="region"
                        aria-label={t("sidebar.yourInstances")}
                        aria-busy={launcherStore.instancesLoading}
                        data-tutorial="instance-list"
                        bind:this={listEl}
                        onpointerdown={grabPanel}
                        oncontextmenu={(event) => { hideTooltip(); oninstancemenu(event); }}
                >
                        {#if launcherStore.instancesLoading && launcherStore.loadedInstances.length === 0}
                                <div class="sc-instance-skeleton sk-delayed" aria-hidden="true">
                                        {#each { length: 6 } as _, index (index)}
                                                <span
                                                        class="sk-block sc-skeleton-icon"
                                                        style="width: 42px; height: 42px;"
                                                ></span>
                                        {/each}
                                </div>
                                <span class="sk-sr-only" role="status"
                                        >{t("common.loading")}</span
                                >
                        {:else if launcherStore.loadedInstances.length === 0}
                                <div class="sc-empty" title={t("sidebar.noInstances")}>—</div>
                        {:else}
                                {#if horizontal}
                                        <div class="sc-hlist" bind:this={hlistEl} use:horizontalWheel>
                                                {#each shown as instance (instance.uuid)}
                                                        <div
                                                                class="sc-hcell"
                                                                animate:flip={{ duration: drag.active ? 160 : 0 }}
                                                        >
                                                                {@render instanceButton(instance)}
                                                        </div>
                                                {/each}
                                        </div>
                                {:else}
                                        <VirtualList
                                                class="dnd-list"
                                                items={shown}
                                                itemHeight={50}
                                                itemHeightVar="--sidebar-compact-row-height"
                                                keyFn={(i) => i.uuid}
                                                hideScrollbar={false}
                                        >
                                                {#snippet children(instance)}
                                                        {@render instanceButton(instance)}
                                                {/snippet}
                                        </VirtualList>
                                {/if}
                        {/if}
                </div>
        </div>

        <div class="sc-tools">
                <button
                        bind:this={plusButtonEl}
                        type="button"
                        class="sc-tool-btn sc-create"
                        class:open={plusMenuOpen}
                        onclick={() => (plusMenuOpen = !plusMenuOpen)}
                        aria-label={t("sidebar.tools")}
                        aria-expanded={plusMenuOpen}
                        title={t("sidebar.tools")}
                >
                        <span class="sc-plus-glyph" aria-hidden="true">+</span>
                </button>
                <DownloadQueue
                        popover={true}
                        popoverUp={queuePopoverUp}
                        popoverRight={queuePopoverRight}
                        popoverSide={queuePopoverSide}
                        dismissOnOutsideClick={true}
                        compactProgress={true}
                />
                <button
                        type="button"
                        class="sc-tool-btn"
                        onclick={onopenquickmenu}
                        data-tutorial="settings"
                        title={t("sidebar.settings")}
                >
                        <Icon name="nav:settings" size={16} />
                </button>
        </div>

        {#if plusMenuOpen}
                <div
                        class="sc-plus-menu"
                        bind:this={plusMenuEl}
                        use:portalPlusMenu
                        transition:plusMenuFly
                        role="menu"
                >
                        <button type="button" role="menuitem" class="sc-create-entry" onclick={() => { plusMenuOpen = false; onopencreateinstance?.(); }}>
                                <Icon name="nav:create" size={15} />
                                <span>{t("sidebar.createInstance")}</span>
                        </button>
                        <button type="button" role="menuitem" onclick={() => { plusMenuOpen = false; onopenversiondownloader?.(); }}>
                                <Icon name="ui:download" size={15} />
                                <span>{t("sidebar.downloadVersions")}</span>
                        </button>
                </div>
        {/if}

        <div
                class="sc-user"
                class:arrow-before={dock === "right" || dock === "bottom"}
                class:active={profileOpen}
                bind:this={userEl}
                data-tutorial="user-profile"
        >
                {#snippet arrow()}
                        <button
                                type="button"
                                class="sc-switch"
                                class:open={switcherOpen}
                                title={t("userMenu.switchAccount")}
                                aria-label={t("userMenu.switchAccount")}
                                aria-expanded={switcherOpen}
                                onclick={(event) => {
                                        event.stopPropagation();
                                        switcherOpen = !switcherOpen;
                                }}
                        >
                                <AccountSwitchChevron placement={switcherPlacement} open={switcherOpen} size={14} />
                        </button>
                {/snippet}
                {#if dock === "right" || dock === "bottom"}{@render arrow()}{/if}
                <div
                        class="sc-user-main"
                        role="button"
                        tabindex="0"
                        title={username}
                        aria-label={username}
                        onclick={() => onopenprofileview?.()}
                        onkeydown={(e) =>
                                (e.key === "Enter" || e.key === " ") && onopenprofileview?.()}
                >
                        <div class="sc-avatar-wrapper">
                                <AccountAvatar {avatarSvg} {username} />
                        </div>
                </div>
                {#if dock === "left" || dock === "top"}{@render arrow()}{/if}
                <div class="sc-name" aria-hidden="true">
                        <span class="sc-name-main" use:fitText={{ text: username, minSize: 1 }}>{username}</span>
                        <span class="sc-name-type" use:fitText={{ text: providerLabel, minSize: 1 }}>{providerLabel}</span>
                </div>
        </div>

        {#if userEl}
                <AccountSwitcher
                        anchor={userEl}
                        placement={switcherPlacement}
                        open={switcherOpen}
                        onclose={() => (switcherOpen = false)}
                />
        {/if}

</aside>

{#if drag.active && drag.dragged}
        <InstanceDragGhost
                instance={drag.dragged}
                x={drag.x}
                y={drag.y}
                compact
                hot={drag.pinZone}
        />
{/if}

{#if Tooltip}
        <Tooltip
                bind:open={tooltipOpen}
                x={tooltipX}
                y={tooltipY}
                placement={tooltipPlacement}
        >
                {#if tooltipInstance}
                        <div class="instance-tooltip">
                                <span class="instance-tooltip-name">{tooltipInstance.name}</span
                                >
                                <span class="instance-tooltip-detail">
                                        {t("instanceView.details.version")}: {tooltipInstance.version}
                                </span>
                                <span class="instance-tooltip-detail">
                                        {t("instanceView.details.loader")}: {tooltipInstance.loader}
                                </span>
                        </div>
                {/if}
        </Tooltip>
{/if}

<style>
        .sidebar-compact {
                width: 100%;
                flex-shrink: 0;
                background: var(--bg-sidebar-gradient, var(--bg-sidebar));
                border-right: 1px solid var(--border);
                display: flex;
                flex-direction: column;
                padding: 10px 0 0;
                z-index: 10;
                user-select: none;
                align-items: center;
                position: relative;
        }

        .sidebar-compact.dock-right {
                border-right: none;
                border-left: 1px solid var(--border);
        }

        /* Зеркальный режим: порядок блоков разворачивается целиком. */
        .sidebar-compact.mirrored:not(.horizontal) {
                flex-direction: column-reverse;
        }

        .sidebar-compact.mirrored.horizontal {
                flex-direction: row-reverse;
        }

        /* Горизонтальный компакт: кнопки инструментов идут в обратном порядке, как и блок.
           Разделители стоят с той же стороны относительно соседнего блока списка, что и в
           обычном режиме: в зеркальном порядке список справа, поэтому бордеры зеркалятся. */
        .sidebar-compact.mirrored.horizontal .sc-tools {
                flex-direction: row-reverse;
                border-left: none;
                border-right: 1px solid var(--border);
        }

        .sidebar-compact.mirrored.horizontal .sc-header {
                border-right: none;
                border-left: 1px solid var(--border);
                /* В зеркальном горизонтальном режиме разделитель находится слева
                   от логотипа: отступ должен быть со стороны разделителя, а не
                   с внешнего правого края панели. */
                padding: 0 0 0 12px;
        }

        /* Кнопки инструментов масштабируются вместе с --ctl, когда панель не вмещается. */
        .horizontal .sc-tool-btn {
                width: calc(32px * var(--ctl, 1));
                height: calc(32px * var(--ctl, 1));
        }

        /* Инверсия: кнопки инструментов идут снизу вверх, как и весь блок. */
        .sidebar-compact.mirrored:not(.horizontal) .sc-tools {
                flex-direction: column-reverse;
        }

        .sc-instance-item.dragging {
                opacity: 0.35;
                pointer-events: none;
        }

        .sc-header.pin-target {
                color: var(--accent);
        }

        .sc-pin-zone {
                display: flex;
                align-items: center;
                justify-content: center;
                width: 34px;
                height: 34px;
                border: 1px dashed color-mix(in srgb, var(--accent) 65%, transparent);
                border-radius: var(--border-radius-sm);
                background: color-mix(in srgb, var(--accent) 12%, transparent);
                color: var(--accent);
                animation: sc-pin-in 0.16s ease-out;
                transition:
                        background 0.12s ease,
                        color 0.12s ease;
        }

        .sc-header.hot .sc-pin-zone {
                border-style: solid;
                background: var(--accent);
                color: var(--bg-main, #fff);
        }

        @keyframes sc-pin-in {
                from {
                        opacity: 0;
                        transform: scale(0.8);
                }
                to {
                        opacity: 1;
                        transform: none;
                }
        }

        .sc-instance-list :global(.dnd-list .virtual-list-item-wrapper) {
                transition: transform 0.16s cubic-bezier(0.2, 0.7, 0.3, 1);
        }

        .sc-header {
                display: flex;
                flex-direction: column;
                align-items: center;
                gap: 10px;
                padding: 0 0 10px;
                margin-bottom: 6px;
                border-bottom: 1px solid var(--border);
                width: 100%;
        }

        .sc-header :global(.icon-svg) {
                color: var(--text-secondary);
        }

        .sc-header .library-nav { min-width: 34px; padding: 4px; }

        .sc-content {
                flex: 1;
                overflow: hidden;
                min-height: 0;
                width: 100%;
                padding: 2px 0;
                display: flex;
                flex-direction: column;
        }

        .sc-instance-list {
                flex: 1;
                min-height: 0;
                display: flex;
                flex-direction: column;
                align-items: stretch;
                gap: 4px;
                padding: 0 6px 6px;
                box-sizing: border-box;
                overflow: hidden;
        }

        .sc-instance-skeleton {
                display: flex;
                flex-direction: column;
                align-items: center;
                gap: 8px;
                padding-top: 2px;
        }

        .sc-skeleton-icon {
                border-radius: var(--border-radius-sm);
                flex-shrink: 0;
        }

        .sc-empty {
                font-size: 0.75rem;
                color: var(--text-muted);
                padding: 10px 0;
                flex: 1;
                display: flex;
                align-items: center;
                justify-content: center;
        }

        .sc-instance-item-wrapper {
                width: 100%;
                height: 100%;
                display: flex;
                align-items: center;
                justify-content: center;
        }

        .sc-instance-list :global(.virtual-list-container) { scrollbar-gutter: stable both-edges; }

        .sc-instance-item {
                position: relative;
                width: min(
                        var(--sidebar-compact-item-size, 42px),
                        calc(100% - var(--space-sm))
                );
                height: calc(
                        var(--virtual-row-height, 50px) - var(--sidebar-row-gap, 8px)
                );
                background: transparent;
                border: 1px solid transparent;
                border-radius: var(
                        --sidebar-compact-item-radius,
                        var(--border-radius)
                );
                cursor: pointer;
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 0;
                transition:
                        background 0.15s ease,
                        border-color 0.15s ease,
                        box-shadow 0.15s ease;
        }

        .sc-instance-item.active {
                background: var(--launcher-selected);
                border-color: var(--selected-border);
                box-shadow: inset 3px 0 var(--accent);
        }

        .dock-right .sc-instance-item.active { box-shadow: inset -3px 0 var(--accent); }
        .sidebar-compact.mirrored.horizontal .sc-instance-item.active { box-shadow: inset -3px 0 var(--accent); }

        .sc-instance-item:not(.dragging):is(:hover, :focus-visible) {
                background: var(--launcher-item-hover);
                border-color: var(--launcher-hover-border);
        }

        .sc-instance-item img {
                -webkit-user-drag: none;
                user-select: none;
                object-fit: contain;
        }

        .sc-instance-running {
                position: absolute;
                bottom: 3px;
                right: 3px;
                width: 8px;
                height: 8px;
                background: var(--color-status-started);
                border: 1.5px solid var(--bg-sidebar);
                border-radius: 50%;
                box-shadow: 0 0 4px rgba(var(--color-success-rgb), 0.5);
        }

        .sc-instance-pin {
                position: absolute;
                top: 3px;
                right: 3px;
                display: flex;
                align-items: center;
                justify-content: center;
                width: 16px;
                height: 16px;
                background: var(--bg-sidebar);
                border: 1px solid var(--border);
                border-radius: 50%;
                color: var(--color-warning);
                pointer-events: none;
                z-index: 1;
                box-shadow: var(--shadow-sm);
        }

        .sc-tools {
                display: flex;
                flex-direction: column;
                align-items: center;
                gap: 6px;
                padding: 8px 0;
                margin: 0 8px;
                border-top: 1px solid var(--border);
        }

        .sc-tools :global(.sd-root.popover) {
                width: 32px;
                flex: 0 0 32px;
                border: 0;
                position: relative;
        }
        .sc-tools :global(.sd-header) {
                width: 32px;
                height: 32px;
                min-height: 32px;
                padding: 0;
                justify-content: center;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                overflow: visible;
        }
        .sc-tools :global(.sd-header-left) {
                flex: 0 0 auto;
                justify-content: center;
        }
        .sc-tools :global(.sd-label),
        .sc-tools :global(.sd-chevron),
        .sc-tools :global(.sd-collapsed-progress) {
                display: none;
        }
        .sc-tools :global(.sd-count-badge) {
                position: absolute;
                top: -5px;
                right: -5px;
                min-width: 17px;
                height: 17px;
                border: 2px solid var(--bg-sidebar);
                font-size: 0.6rem;
        }
        .sc-tool-btn {
                width: 32px;
                height: 32px;
                background: transparent;
                border: 1px solid var(--border);
                color: var(--text-secondary);
                border-radius: var(--border-radius-sm);
                cursor: pointer;
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 0;
                transition:
                        background 0.15s ease,
                        color 0.15s ease;
        }

        .sc-tool-btn:is(:hover, :focus-visible) {
                background: var(--launcher-item-hover);
                color: var(--text-primary);
        }

        .sc-tool-btn.sc-create {
                background: var(--accent);
                color: var(--accent-text);
        }

        .sc-tool-btn.sc-create:is(:hover, .open) {
                background: var(--accent-hover);
                border-color: var(--launcher-hover-border);
                box-shadow: 0 0 0 1px var(--launcher-hover-border);
        }

        .sc-download-btn {
                position: relative;
        }

        .sc-download-progress {
                position: absolute;
                left: 50%;
                bottom: 2px;
                transform: translateX(-50%);
                width: 19px;
                height: 2px;
                overflow: hidden;
                border-radius: 2px;
                background: var(--border);
                pointer-events: none;
        }

        .sc-download-badge {
                position: absolute;
                top: -5px;
                right: -5px;
                min-width: 17px;
                height: 17px;
                padding: 0 4px;
                background: var(--accent);
                color: var(--accent-text);
                border-radius: 9px;
                font-size: 0.6rem;
                font-weight: 800;
                display: flex;
                align-items: center;
                justify-content: center;
                border: 2px solid var(--bg-sidebar);
                box-shadow: 0 0 0 0 rgba(var(--accent-rgb), 0.4);
                animation: dl-badge-pulse 1.6s ease-in-out infinite;
                z-index: 1;
        }

        @keyframes dl-badge-pulse {
                0%,
                100% {
                        box-shadow: 0 0 0 0 rgba(var(--accent-rgb), 0.4);
                }
                50% {
                        box-shadow: 0 0 0 4px rgba(var(--accent-rgb), 0);
                }
        }

        .sc-plus-menu {
                position: fixed;
                z-index: 1000;
                box-sizing: border-box;
                width: min(190px, calc(100vw - 16px));
                overflow-y: auto;
                padding: 5px;
                display: flex;
                flex-direction: column;
                gap: 3px;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                background: var(--bg-sidebar);
                box-shadow: var(--shadow-md, 0 8px 24px rgba(0,0,0,.28));
        }

        .sc-plus-menu button {
                display: flex;
                align-items: center;
                gap: 9px;
                width: 100%;
                min-height: 38px;
                padding: 7px 9px;
                border: 0;
                border-radius: var(--border-radius-sm);
                background: transparent;
                color: var(--text-primary);
                text-align: left;
                cursor: pointer;
                font: inherit;
                font-size: 0.76rem;
        }
        .sc-plus-menu .sc-create-entry { background: var(--launcher-selected); }

        .sc-plus-menu button:is(:hover, :focus-visible) {
                background: var(--launcher-item-hover);
                box-shadow: inset 0 0 0 1px var(--launcher-hover-border);
        }
        .sc-plus-glyph {
                display: block;
                font-size: 25px;
                line-height: 1;
                font-weight: 400;
                transform: translateY(-1px);
        }
        .sc-user {
                position: relative;
                isolation: isolate;
                box-sizing: border-box;
                width: 100%;
                max-width: 100%;
                min-width: 0;
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 8px 2px;
                margin-top: auto;
                background: transparent;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
                overflow: hidden;
                transition: background .15s ease;
        }

        /* Как у «Библиотеки»: подсвечен, пока открыт раздел аккаунта, а не при фокусе. */
        .sc-user:has(.sc-user-main:hover):not(:has(.sc-switch:hover)) {
                background: var(--launcher-item-hover);
                border-color: var(--launcher-hover-border);
        }

        .sc-user.active {
                background: var(--launcher-selected);
                border-color: color-mix(in srgb, var(--accent) 32%, var(--border));
        }

        .sc-user-main {
                display: flex;
                flex-direction: column;
                align-items: center;
                padding-bottom: 20px;
                flex: 1;
                min-width: 0;
                cursor: pointer;
        }

        .sc-name {
                position: absolute;
                inset-inline: 3px;
                bottom: 8px;
                height: 16px;
                overflow: hidden;
                font-size: 0.8rem;
                font-weight: 700;
                text-align: center;
                pointer-events: none;
        }

        .sc-name-main,
        .sc-name-type {
                position: absolute;
                inset: 0;
                line-height: 16px;
                overflow: hidden;
                white-space: nowrap;
                transition: transform 0.2s ease, opacity 0.2s ease;
        }

        .sc-name-type {
                color: var(--text-secondary);
                transform: translateY(100%);
                opacity: 0;
        }

        .sc-user:has(.sc-user-main:hover):not(:has(.sc-switch:hover)) .sc-name-main {
                transform: translateY(-100%);
                opacity: 0;
        }

        .sc-user:has(.sc-user-main:hover):not(:has(.sc-switch:hover)) .sc-name-type {
                transform: translateY(0);
                opacity: 1;
        }

        @media (prefers-reduced-motion: reduce) {
                .sc-name-main,
                .sc-name-type { transition: none; }
        }

        /* Кнопка смены аккаунта растянута на всю плитку аккаунта и касается её краёв. */
        .sc-switch {
                position: relative;
                z-index: 1;
                flex-shrink: 0;
                display: flex;
                align-items: center;
                justify-content: center;
                align-self: stretch;
                width: 18px;
                margin: 0;
                padding: 0;
                align-self: center;
                height: 100%;
                background: transparent;
                border: none;
                border-radius: var(--launcher-control-radius);
                box-shadow: inset 0 0 0 1px transparent;
                color: var(--text-secondary);
                cursor: pointer;
                transition: box-shadow 0.15s ease;
        }

        /* Как у «Библиотеки»: при открытом списке аккаунтов кнопка подсвечена,
           при закрытом остаётся обычной; подсветка только при наведении, без фокуса. */
        .sc-switch:hover {
                background: var(--launcher-item-hover);
                color: var(--text-primary);
                box-shadow: inset 0 0 0 1px var(--accent-hover, var(--accent));
        }

        .sc-switch.open {
                background: var(--launcher-selected);
                color: var(--text-primary);
        }

        .sc-avatar-wrapper {
                /* В вертикальном компакте оставляем ширину для стрелки и рамки плитки. */
                --sc-avatar-size: clamp(28px, calc(16px + var(--sidebar-size, 60px) * 0.2), 36px);
                width: var(--sc-avatar-size);
                height: var(--sc-avatar-size);
                border-radius: var(--border-radius-sm);
                border: 1px solid var(--border);
                flex-shrink: 0;
                background-color: var(--bg-card);
                overflow: hidden;
                display: flex;
                align-items: center;
                justify-content: center;
        }

        .sc-avatar-wrapper :global(svg),
        .sc-avatar-wrapper :global(img) {
                width: 100%;
                height: 100%;
                display: block;
                border-radius: inherit;
        }

        /* ---- Barra compacta acoplada arriba o abajo ---- */
        .sidebar-compact.horizontal {
                width: 100%;
                height: 100%;
                flex-direction: row;
                padding: 0 14px;
                gap: 10px;
                border-right: none;
        }

        .sidebar-compact.horizontal.dock-top {
                border-bottom: 1px solid var(--border);
        }

        .sidebar-compact.horizontal.dock-bottom {
                border-top: 1px solid var(--border);
        }

        .horizontal .sc-header {
                flex-direction: row;
                justify-content: center;
                align-self: stretch;
                min-width: 40px;
                padding: 0 12px 0 0;
                margin: 0;
                border-bottom: none;
                border-right: 1px solid var(--border);
                width: auto;
        }

        .horizontal .sc-content {
                flex: 1;
                min-width: 0;
                height: 100%;
                padding: 0;
        }

        .horizontal .sc-instance-list {
                flex: 1;
                justify-content: center;
                padding: var(--sidebar-edge-inset, 5px) 6px;
        }

        /* Theme-neutral fade toward the adjacent action/download controls. */
        .sidebar-compact:not(.horizontal):not(.mirrored) .sc-instance-list { -webkit-mask-image: linear-gradient(to bottom, #000 0%, #000 88%, transparent 100%); mask-image: linear-gradient(to bottom, #000 0%, #000 88%, transparent 100%); }
        .sidebar-compact:not(.horizontal).mirrored .sc-instance-list { -webkit-mask-image: linear-gradient(to top, #000 0%, #000 88%, transparent 100%); mask-image: linear-gradient(to top, #000 0%, #000 88%, transparent 100%); }
        .sidebar-compact.horizontal:not(.mirrored) .sc-instance-list { -webkit-mask-image: linear-gradient(to right, #000 0%, #000 90%, transparent 100%); mask-image: linear-gradient(to right, #000 0%, #000 90%, transparent 100%); }
        .sidebar-compact.horizontal.mirrored .sc-instance-list { -webkit-mask-image: linear-gradient(to left, #000 0%, #000 90%, transparent 100%); mask-image: linear-gradient(to left, #000 0%, #000 90%, transparent 100%); }

        .sc-hlist {
                display: flex;
                justify-content: flex-start;
                width: 100%;
                align-items: center;
                gap: 8px;
                height: 100%;
                overflow-x: auto;
                overflow-y: hidden;
        }

        /* Горизонтальный компакт, зеркальный режим: список прижимается к правому краю.
           RTL на контейнере задаёт старт справа; прокрутка при переполнении
           остаётся доступна (overflow уходит влево, а не за пределы экрана). */
        .sidebar-compact.mirrored.horizontal .sc-hlist {
                direction: rtl;
                justify-content: flex-start;
        }

        .sidebar-compact.mirrored.horizontal .sc-hcell {
                direction: ltr;
        }


        /* Скроллбары компакта — те же размеры и цвет, что у обычной панели. */
        .sidebar-compact .sc-hlist {
                scrollbar-gutter: stable;
        }
        .sidebar-compact .sc-hlist::-webkit-scrollbar { height: 4px; }
        .sidebar-compact .sc-hlist::-webkit-scrollbar-track { background: transparent; }
        .sidebar-compact .sc-hlist::-webkit-scrollbar-thumb {
                background: color-mix(in srgb, var(--accent, var(--primary)) 72%, transparent);
                border-radius: 999px;
        }
        .sidebar-compact :global(.virtual-list-container)::-webkit-scrollbar { width: 4px; }
        .sidebar-compact :global(.virtual-list-container)::-webkit-scrollbar-track { background: transparent; }
        .sidebar-compact :global(.virtual-list-container)::-webkit-scrollbar-thumb {
                background: color-mix(in srgb, var(--accent, var(--primary)) 72%, transparent);
                border-radius: 999px;
        }
        /* Firefox не понимает ::-webkit-scrollbar: используем нативный тонкий скролл. */
        @supports (-moz-appearance: none) {
                .sidebar-compact .sc-hlist,
                .sidebar-compact :global(.virtual-list-container) {
                        scrollbar-width: thin;
                        scrollbar-color: var(--accent, var(--primary)) transparent;
                }
        }

        .sc-hcell {
                flex-shrink: 0;
                zoom: var(--sidebar-card-scale, 1);
                width: var(--sidebar-compact-item-size, 42px);
                height: var(--sidebar-compact-item-size, 42px);
                --virtual-row-height: calc(
                        var(--sidebar-compact-item-size, 42px) + var(--sidebar-row-gap, 8px)
                );
        }

        .sc-hcell .sc-instance-item-wrapper {
                width: 100%;
                height: 100%;
        }

        .sc-hcell .sc-instance-item {
                width: 100%;
        }

        .horizontal .sc-tools {
                flex-direction: row;
                gap: calc(6px * var(--ctl, 1));
                padding: 0 calc(10px * var(--ctl, 1));
                margin: 0;
                border-top: none;
                border-left: 1px solid var(--border);
                align-self: center;
                height: 100%;
                align-items: center;
        }

        .horizontal .sc-user {
                width: auto;
                min-width: calc(76px * var(--ctl, 1));
                height: 100%;
                max-height: 100%;
                min-height: 0;
                align-self: stretch;
                margin: 0;
                padding: 2px calc(12px * var(--ctl, 1));
                flex-direction: column;
                gap: 0;
        }

        .horizontal .sc-user-main {
                flex: 0 0 auto;
                width: calc(64px * var(--ctl, 1));
                padding-bottom: 13px;
        }

        .horizontal .sc-avatar-wrapper {
                /* Оставляем по высоте место для строки имени, стрелки и отступов. */
                --sc-avatar-size: min(
                        calc(64px * var(--ctl, 1)),
                        clamp(22px, calc(var(--sidebar-size, 56px) - 36px), 44px)
                );
        }
        .horizontal .sc-name { bottom: 12px; }
        .horizontal .sc-user.arrow-before .sc-name { bottom: 0; }

        .sc-user.arrow-before .sc-switch {
                margin: 0;
        }

        .horizontal .sc-switch {
                align-self: stretch;
                width: auto;
                height: 16px;
                margin: 0 calc(-12px * var(--ctl, 1)) -2px;
        }

        .horizontal .sc-user.arrow-before .sc-switch {
                margin: -2px calc(-12px * var(--ctl, 1)) 0;
        }

        .instance-tooltip {
                display: flex;
                flex-direction: column;
                gap: 2px;
        }

        .instance-tooltip-name {
                font-size: 0.82rem;
                font-weight: 700;
                color: var(--text-primary);
                white-space: nowrap;
        }

        .instance-tooltip-detail {
                font-size: 0.72rem;
                color: var(--text-secondary);
                white-space: nowrap;
        }

</style>
