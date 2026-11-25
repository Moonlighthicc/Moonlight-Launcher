<script lang="ts">
        import { onDestroy, tick } from "svelte";
        import { flip } from "svelte/animate";
        import { instanceNav } from "$lib/state/instanceNavState.svelte";
        import { getActiveUser } from "$lib/api/launcherService";
        import { pinInstance } from "$lib/api/cubicApi";
        import { launcherStore } from "$lib/state/state.svelte";
        import { accountAvatar } from "$lib/state/avatarCache.svelte";
        import type { InstanceDto } from "$lib/types/types";
        import { getAccountProvider } from "$lib/utils/accountProviders";
        import { t } from "$lib/i18n";
        import { fitText } from "$lib/utils/fitText";
        import { horizontalWheel } from "$lib/utils/horizontalWheel";
        import Icon from "$lib/icons/Icon.svelte";
        import DownloadQueue from "../DownloadQueue/DownloadQueue.svelte";
        import VirtualList from "$lib/components/layout/VirtualList.svelte";
        import InstanceItem from "./InstanceItem.svelte";
        import UserProfile from "./UserProfile.svelte";
        import SortFilter from "./SortFilter.svelte";
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
                onopeneditinstance: (instance: InstanceDto) => void;
                ondeleteinstance: (instance: InstanceDto) => void;
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
                onopeneditinstance,
                ondeleteinstance,
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
        const H_GAP = 8;
        const H_ROW_GAP = 2;
        const H_SCROLLBAR = 4;
        const H_CARD_MAX_SCALE = 1.55;
        const H_THREE_ROW_MIN_SCALE = 0.85;
        let hListHeight = $state(0);
        let hBaseRow = $state(52);
        const hCardsHeight = $derived(Math.max(0, hListHeight - 2 * H_SCROLLBAR));
        const hRowCapacity = $derived(
                hCardsHeight >= 3 * hBaseRow * H_THREE_ROW_MIN_SCALE + 2 * H_ROW_GAP ? 3 : 2,
        );

        let asideEl: HTMLElement | undefined = $state();

        /** Минимум для списка экземпляров в развёрнутой горизонтальной панели. */
        const MIN_HBAR_LIST = 200;
        /** Должно совпадать с max-width у `.horizontal .sidebar-sections` в CSS. */
        const HBAR_MAX_SHARE = 0.52;
        /**
         * Развёрнутая горизонтальная панель: масштабируются только кнопки блока
         * «инструменты / загрузки / профиль» (через --ctl), а не весь блок. Подбираем
         * наибольший масштаб, при котором блок вмещается рядом с логотипом и списком.
         */
        function fitBar() {
                const el = asideEl;
                if (!horizontal || !el) return;
                const sections = el.querySelector<HTMLElement>(".sidebar-sections");
                const header = el.querySelector<HTMLElement>(".sidebar-header");
                if (!sections || !header) return;
                const cs = getComputedStyle(el);
                hBaseRow = parseFloat(cs.getPropertyValue("--sidebar-row-height")) || 52;
                const pad = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
                const gap = parseFloat(cs.columnGap) || 0;
                // Блок ограничен и остатком до списка, и своей долей (max-width в CSS):
                // иначе замер «влезает», а реальный блок уже, и кнопки вылезают наружу.
                const room = Math.min(
                        el.clientWidth - pad - gap * 2 - header.offsetWidth - MIN_HBAR_LIST,
                        (el.clientWidth - pad) * HBAR_MAX_SHARE,
                );
                // Естественная ширина блока при текущем масштабе кнопок (maxWidth снимаем на время замера).
                const measure = () => {
                        sections.style.maxWidth = "none";
                        const width = sections.offsetWidth;
                        sections.style.maxWidth = "";
                        return width;
                };
                let scale = 1;
                el.style.setProperty("--ctl", "1");
                while (measure() > room && scale > 0.5) {
                        scale = Math.max(0.5, +(scale - 0.05).toFixed(2));
                        el.style.setProperty("--ctl", String(scale));
                }
        }
        $effect(() => {
                if (asideEl && horizontal) fitBar();
        });
        // Размер самой панели (не только окна) и загрузка шрифтов меняют ширину кнопок.
        $effect(() => {
                const el = asideEl;
                if (!el || !horizontal) return;
                const ro = new ResizeObserver(() => fitBar());
                ro.observe(el);
                void document.fonts?.ready.then(() => fitBar());
                return () => ro.disconnect();
        });
        // Смена языка, имени пользователя или статуса меняет естественную ширину блока.
        $effect(() => {
                void [
                        t("sidebar.createInstance"),
                        t("sidebar.downloadVersions"),
                        t("sidebar.settings"),
                        username,
                        userTypeLabel,
                        isPremium,
                ];
                if (asideEl && horizontal) void tick().then(fitBar);
        });
        let listEl: HTMLElement | undefined = $state();
        let hlistEl: HTMLElement | undefined = $state();

        $effect(() => {
                rememberFirstSeen(launcherStore.loadedInstances);
        });

        const ordered = $derived(orderInstances(launcherStore.loadedInstances));
        const hRows = $derived(Math.min(hRowCapacity, Math.max(1, ordered.length)));
        // Scale to the measured list height regardless of how many rows fit.
        const hCardScale = $derived(hCardsHeight > (hRows - 1) * H_ROW_GAP
                ? Math.min(
                        H_CARD_MAX_SCALE,
                        (hCardsHeight - (hRows - 1) * H_ROW_GAP) / (hRows * hBaseRow),
                )
                : 1);
        function scroller(): HTMLElement | null {
                return horizontal
                        ? (hlistEl ?? null)
                        : (listEl?.querySelector<HTMLElement>(".virtual-list-container") ??
                                        null);
        }

        /** Индекс в `ordered` по точке. Порядок элементов не меняется: зеркалится только блок (CSS). */
        function indexAt(x: number, y: number): number {
                const count = ordered.length;
                const box = scroller();
                if (!box || count === 0) return 0;
                const rect = box.getBoundingClientRect();
                if (!horizontal) {
                        const wrapper = box.querySelector<HTMLElement>(
                                ".virtual-list-item-wrapper",
                        );
                        const rowH = wrapper?.getBoundingClientRect().height || 52;
                        const raw = Math.floor((y - rect.top + box.scrollTop) / rowH);
                        return Math.min(count - 1, Math.max(0, raw));
                }
                const first = box.querySelector<HTMLElement>("[data-instance-uuid]");
                if (!first) return 0;
                const cell = first.getBoundingClientRect();
                const col = Math.max(
                        0,
                        Math.floor(
                                (align === "end" ? cell.right - x : x - cell.left) /
                                        (cell.width + H_GAP),
                        ),
                );
                const row = Math.min(
                        hRows - 1,
                        Math.max(0, Math.floor((y - cell.top) / (cell.height + H_ROW_GAP))),
                );
                return Math.min(count - 1, col * hRows + row);
        }

        function inPinZone(x: number, y: number): boolean {
                if (!asideEl || !listEl) return false;
                const bar = asideEl.getBoundingClientRect();
                const list = listEl.getBoundingClientRect();
                if (x < bar.left || x > bar.right || y < bar.top || y > bar.bottom)
                        return false;
                // Zona de anclado: cabecera/logo. En la variante espejada de
                // las barras horizontales la cabecera está a la derecha de la lista,
                // así que la zona debe comprobar el borde derecho, no el izquierdo.
                if (horizontal) {
                        return align === "end" ? x > list.right : x < list.left;
                }
                return y < list.top;
        }

        function autoScroll(x: number, y: number): boolean {
                const box = scroller();
                if (!box) return false;
                const rect = box.getBoundingClientRect();
                const { zone, speed } = DRAG_EDGE;
                let delta = 0;
                if (horizontal) {
                        if (x < rect.left + zone) delta = -speed;
                        else if (x > rect.right - zone) delta = speed;
                        const before = box.scrollLeft;
                        box.scrollLeft += delta;
                        return box.scrollLeft !== before;
                }
                if (y < rect.top + zone) delta = -speed;
                else if (y > rect.bottom - zone) delta = speed;
                const before = box.scrollTop;
                box.scrollTop += delta;
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
        onDestroy(() => drag.destroy());

        /** Порядок для отрисовки. Данные и сохранение порядка не меняются — только показ. */
        const shown = $derived(drag.display);

        /** Agarrar un hueco de la lista o el rótulo mueve la barra entera. */
        function grabPanel(event: PointerEvent) {
                const target = event.target as HTMLElement;
                if (
                        target.closest(
                                "[data-instance-uuid], button, a, input, [data-no-panel-drag]",
                        )
                )
                        return;
                onpaneldrag?.(event);
        }


        let activeUser = $derived(getActiveUser());
        let username = $derived(activeUser?.username ?? "Steve");
        let isPremium = $derived(activeUser?.user_type === "Microsoft");
        let userTypeLabel = $derived(
                t(
                        `userMenu.short.${activeUser ? getAccountProvider(activeUser) : "offline"}`,
                ),
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

<svelte:window onresize={fitBar} />

<aside
        class="sidebar dock-{dock}"
        class:horizontal
        class:mirrored={align === "end"}
        class:dragging-instance={drag.active}
        bind:this={asideEl}
        data-part="sidebar-panel"
        data-dock={dock}
>
        <div
                class="sidebar-header"
                class:pin-target={drag.active}
                class:hot={drag.pinZone}
                data-tutorial="sidebar-header"
                data-part="sidebar-header"
        >
                {#if drag.active && drag.dragged}
                        <span class="pin-zone-label">
                                <Icon name="ui:pin" size={13} />
                                {drag.dragged.pinned
                                        ? t("sidebar.unpinDrop")
                                        : t("sidebar.pinDrop")}
                        </span>
                {:else}
                        <button type="button" class="library-nav" aria-current={!selectedInstance && !profileOpen ? "page" : undefined}
                                title={t("library.title")} aria-label={t("library.title")} onclick={onopenlibrary}>
                                <Icon name="instance:grid" size={18} />
                                <span>{t("library.title")}</span>
                        </button>
                {/if}
        </div>

        <div class="sidebar-content" data-part="sidebar-content">
                <div
                        class="instances-section"
                        role="region"
                        aria-label={t("sidebar.yourInstances")}
                        aria-busy={launcherStore.instancesLoading}
                        oncontextmenu={oninstancemenu}
                >
                        {@render instanceFilter()}
                        <div
                                class="instance-list"
                                class:horizontal-list={horizontal}
                                data-tutorial="instance-list"
                                bind:this={listEl}
                                bind:clientHeight={hListHeight}
                                role="presentation"
                                onpointerdown={grabPanel}
                        >
                                {#if launcherStore.instancesLoading && launcherStore.loadedInstances.length === 0}
                                        <div
                                                class="instance-skeleton sk-delayed"
                                                aria-hidden="true"
                                        >
                                                {#each { length: 6 } as _, index (index)}
                                                        <div class="instance-skeleton-row">
                                                                <span
                                                                        class="sk-block instance-skeleton-icon"
                                                                        style="width: 24px; height: 24px;"
                                                                ></span>
                                                                <div class="sk-stack instance-skeleton-text">
                                                                        <span class="sk-block sk-line"></span>
                                                                        <span class="sk-block sk-line sk-line-sm"
                                                                        ></span>
                                                                </div>
                                                        </div>
                                                {/each}
                                        </div>
                                        <span class="sk-sr-only" role="status"
                                                >{t("common.loading")}</span
                                        >
                                {:else if launcherStore.loadedInstances.length === 0}
                                        <div class="empty-instances">
                                                {t("sidebar.noInstances")}
                                        </div>
                                {:else if horizontal}
                                        <div
                                                class="hlist"
                                                style:--h-rows={hRows}
                                                style:--h-card-scale={hCardScale}
                                                style:--h-row-gap={`${H_ROW_GAP}px`}
                                                style:--h-scrollbar={`${H_SCROLLBAR}px`}
                                                bind:this={hlistEl}
                                                use:horizontalWheel
                                        >
                                                {#each shown as instance (instance.uuid)}
                                                        <div
                                                                class="hcell"
                                                                animate:flip={{ duration: drag.active ? 160 : 0 }}
                                                        >
                                                                <InstanceItem
                                                                        {instance}
                                                                        selected={selectedInstance?.uuid ===
                                                                                instance.uuid}
                                                                        dragging={drag.active &&
                                                                                drag.uuid === instance.uuid}
                                                                        onpointerdown={(e) => drag.start(e, instance)}
                                                                        onselect={() => selectInstance(instance)}
                                                                        onedit={() => onopeneditinstance?.(instance)}
                                                                        ondelete={() => ondeleteinstance(instance)}
                                                                />
                                                        </div>
                                                {/each}
                                        </div>
                                {:else}
                                        <VirtualList
                                                class="dnd-list"
                                                items={shown}
                                                itemHeight={52}
                                                itemHeightVar="--sidebar-row-height"
                                                padding={26}
                                                keyFn={(i) => i.uuid}
                                                hideScrollbar={false}
                                        >
                                                {#snippet children(instance)}
                                                        <InstanceItem
                                                                {instance}
                                                                rightDock={dock === "right"}
                                                                selected={selectedInstance?.uuid ===
                                                                        instance.uuid}
                                                                dragging={drag.active &&
                                                                        drag.uuid === instance.uuid}
                                                                onpointerdown={(e) => drag.start(e, instance)}
                                                                onselect={() => selectInstance(instance)}
                                                                onedit={() => onopeneditinstance?.(instance)}
                                                                ondelete={() => ondeleteinstance(instance)}
                                                        />
                                                {/snippet}
                                        </VirtualList>
                                {/if}
                        </div>
                </div>
        </div>

        <div class="sidebar-sections" data-part="sidebar-sections">
                <div class="hs-dl">
                        <DownloadQueue popover={horizontal} popoverUp={dock === "bottom"} popoverRight={align === "start"} popoverSide={horizontal ? "none" : dock === "right" ? "left" : "right"} dismissOnOutsideClick={horizontal} />
                </div>

                        <div class="tools-group" class:tools-row={horizontal} data-tutorial="tools-group" role="group" aria-label={t("sidebar.tools")}>
                                <button
                                        type="button"
                                        class="tools-btn"
                                        onclick={onopencreateinstance}
                                        data-tutorial="create-instance"
                                >
                                        <Icon name="nav:create" size={14} />
                                        <span class="tools-label">{t("sidebar.createInstance")}</span>
                                </button>
                                <button
                                        type="button"
                                        class="tools-btn"
                                        onclick={onopenversiondownloader}
                                        data-tutorial="download-versions"
                                >
                                        <span class="tools-icon-wrap">
                                                <Icon name="ui:download" size={14} />
                                                
                                        </span>
                                        <span class="tools-label">{t("sidebar.downloadVersions")}</span>
                                </button>
                                <button
                                        type="button"
                                        class="tools-btn"
                                        onclick={onopenquickmenu}
                                        data-tutorial="settings"
                                >
                                        <Icon name="nav:settings" size={14} />
                                        <span class="tools-label">{t("sidebar.settings")}</span>
                                </button>
                        </div>

                <div data-tutorial="user-profile" class="user-slot">
                        <UserProfile
                                {username}
                                {avatarSvg}
                                {isPremium}
                                {userTypeLabel}
                                placement={horizontal && dock === "top" ? "down" : "up"}
                                arrowBefore={dock === "right"}
                                onclick={() => onopenprofileview?.()}
                        />
                </div>
        </div>

</aside>

{#if drag.active && drag.dragged}
        <InstanceDragGhost
                instance={drag.dragged}
                x={drag.x}
                y={drag.y}
                hot={drag.pinZone}
        />
{/if}

{#snippet instanceFilter()}
        <div class="section-head" role="presentation" onpointerdown={grabPanel}>
                {#if horizontal}
                        <span class="section-label" use:fitText={t("sidebar.yourInstances")}>{t("sidebar.yourInstances")}</span>
                {:else}
                        <span class="section-label">{t("sidebar.yourInstances")}</span>
                {/if}
                <SortFilter />
        </div>
{/snippet}

<style>
        .sidebar {
                width: 100%;
                flex-shrink: 0;
                background: var(--bg-sidebar-gradient, var(--bg-sidebar));
                border-right: 1px solid var(--border);
                display: flex;
                flex-direction: column;
                padding: var(--sidebar-padding-top, 14px) var(--sidebar-padding-x, 12px) 0
                        var(--sidebar-edge-inset, 5px);
                z-index: 10;
                user-select: none;
                position: relative;
        }

        .sidebar.dock-right {
                --instance-selection-offset: -3px;
                border-right: none;
                border-left: 1px solid var(--border);
                padding-inline: var(--sidebar-padding-x, 12px) var(--sidebar-edge-inset, 5px);
        }

        /* Зеркальный режим (панель брошена в «end»-половину края): полный разворот
           порядка блоков — у вертикальной панели логотип оказывается внизу, блок
           инструментов вверху; горизонтальная отражается по горизонтали. */
        .sidebar.mirrored:not(.horizontal) {
                flex-direction: column-reverse;
        }

        .sidebar.mirrored.horizontal {
                flex-direction: row-reverse;
        }

        /* В зеркальном режиме сетка начинает размещение от правого края.
           Направление содержимого карточек остаётся обычным, а порядок данных
           (слева направо в исходной модели) не переворачивается программно. */
        .sidebar.mirrored.horizontal .hlist {
                /*
                 * В RTL inline-start находится СПРАВА. Поэтому start прижимает
                 * сетку карточек к правому краю области экземпляров.
                 * center здесь был причиной того, что карточки оставались
                 * посередине/слева даже после переноса панели.
                 */
                width: 100%;
                justify-content: start;
                direction: rtl;
        }

        .sidebar.mirrored.horizontal .hcell {
                direction: ltr;
        }

        .sidebar.mirrored.horizontal .section-head {
                flex-direction: row-reverse;
        }

        .sidebar.mirrored.horizontal .section-label {
                text-align: right;
        }

        /* Выделение с той же стороны, что и у вертикальной зеркальной панели. */
        .sidebar.mirrored.horizontal {
                --instance-selection-offset: -3px;
        }

        /* Горизонтальная зеркальная панель: блок инструментов — справа, загрузки и профиль — слева. */
        /* Зеркальный вариант: колонка инструментов фиксирована масштабом, а не текстом. */
        .sidebar.mirrored.horizontal .sidebar-sections {
                grid-template-columns: minmax(0, 1fr) calc(180px * var(--ctl, 1));
                grid-template-areas:
                        "dl tools"
                        "user tools";
        }

        /* Инверсия по вертикали: футер (аккаунт, инструменты, загрузки) наверху,
           заголовок «Ваши экземпляры» и список — снизу, кнопки инструментов — по порядку снизу вверх. */
        .sidebar.mirrored:not(.horizontal) .sidebar-sections,
        .sidebar.mirrored:not(.horizontal) .instances-section {
                flex-direction: column-reverse;
        }
        .sidebar.mirrored:not(.horizontal) .tools-group {
                flex-direction: column-reverse;
        }

        .sidebar.mirrored:not(.horizontal) .sidebar-header {
                border-bottom: none;
                border-top: 1px solid var(--border);
                padding-bottom: 0;
                margin-bottom: 0;
                padding-top: 10px;
                margin-top: 6px;
        }

        .sidebar-header {
                padding-bottom: 10px;
                margin-bottom: 6px;
                border-bottom: 1px solid var(--border);
                display: flex;
                align-items: center;
                justify-content: space-between;
        }

        .sidebar-header {
                gap: 8px;
        }

        .library-nav { width: 100%; justify-content: flex-start; }

        .section-head {
                display: flex;
                align-items: center;
                justify-content: space-between;
                gap: 8px;
                margin-bottom: 6px;
                min-height: 18px;
                cursor: grab;
        }

        .section-label {
                flex: 1;
                min-width: 0;
                overflow: hidden;
                font-size: 0.65rem;
                font-weight: 700;
                color: var(--text-secondary);
                text-overflow: ellipsis;
                text-transform: uppercase;
                letter-spacing: 1.5px;
                white-space: nowrap;
        }

        .sidebar:not(.horizontal) .section-head {
                flex-direction: column;
                align-items: stretch;
                gap: 5px;
        }

        .sidebar:not(.horizontal) .section-label {
                flex: none;
                text-align: left;
                white-space: normal;
                overflow-wrap: anywhere;
        }

        .sidebar:not(.horizontal) .section-head :global(.sort-btn) {
                align-self: flex-start;
                margin-inline-end: 0;
        }

        .sidebar.dock-right:not(.horizontal) .section-label {
                text-align: right;
        }

        .sidebar.dock-right:not(.horizontal) .section-head :global(.sort-btn) {
                align-self: flex-end;
        }

        /* Mientras se arrastra una instancia, la cabecera pasa a ser la zona de anclado. */
        .sidebar-header.pin-target {
                margin-left: -4px;
                margin-right: -4px;
                padding: 0 4px 10px;
                border-bottom-color: transparent;
        }

        .pin-zone-label {
                display: flex;
                flex: 1;
                align-items: center;
                justify-content: center;
                gap: 6px;
                min-height: 26px;
                padding: 4px 8px;
                border: 1px dashed color-mix(in srgb, var(--accent) 60%, transparent);
                border-radius: var(--border-radius-sm);
                background: color-mix(in srgb, var(--accent) 10%, transparent);
                color: var(--accent);
                font-size: 0.65rem;
                font-weight: 700;
                letter-spacing: 1.2px;
                text-transform: uppercase;
                white-space: nowrap;
                animation: pin-zone-in 0.16s ease-out;
                transition:
                        background 0.12s ease,
                        color 0.12s ease,
                        border-color 0.12s ease;
        }

        .sidebar-header.hot .pin-zone-label {
                border-style: solid;
                border-color: var(--accent);
                background: var(--accent);
                color: var(--bg-main, #fff);
        }

        @keyframes pin-zone-in {
                from {
                        opacity: 0;
                        transform: translateY(-4px);
                }
                to {
                        opacity: 1;
                        transform: none;
                }
        }

        /* Las filas se apartan con suavidad mientras se arrastra. */
        .instance-list :global(.dnd-list .virtual-list-item-wrapper) {
                transition: transform 0.16s cubic-bezier(0.2, 0.7, 0.3, 1);
        }

        :global(html.instance-dragging),
        :global(html.instance-dragging *) {
                cursor: grabbing !important;
                user-select: none !important;
        }

        .sidebar-content {
                flex: 1;
                overflow: hidden;
                min-height: 0;
                padding: 4px 0;
                display: flex;
                flex-direction: column;
        }

        .instances-section {
                flex: 1;
                display: flex;
                flex-direction: column;
                min-height: 0;
        }

        .empty-instances {
                padding: 10px;
                font-size: 0.8rem;
                color: var(--text-muted);
                text-align: center;
                border: 1px dashed var(--border);
                border-radius: var(--border-radius);
                margin-top: 4px;
                flex: 1;
                display: flex;
                align-items: center;
                justify-content: center;
        }

        .instance-list {
                flex: 1;
                min-height: 0;
                display: flex;
                flex-direction: column;
                /* Keep cards clear of the divider and leave breathing room below the last row. */
                padding: 0 6px 6px;
                box-sizing: border-box;
                overflow: hidden;
        }

        .dock-left:not(.horizontal) .instance-list { padding-left: 2px; }
        .dock-right:not(.horizontal) .instance-list { padding-right: 2px; }

        .instance-skeleton {
                display: flex;
                flex-direction: column;
                gap: 8px;
                padding: 2px;
        }

        .instance-skeleton-row {
                display: flex;
                align-items: center;
                gap: 12px;
                height: calc(
                        var(--sidebar-row-height, 52px) - var(--sidebar-row-gap, 8px)
                );
                flex-shrink: 0;
        }

        .instance-skeleton-icon {
                margin-left: 12px;
                border-radius: var(--border-radius-sm);
        }

        .instance-skeleton-text {
                flex: 1;
                padding-right: 10px;
        }

        /* Theme-neutral fade toward the adjacent action/download controls. */
        .sidebar:not(.horizontal):not(.mirrored) .instance-list { -webkit-mask-image: linear-gradient(to bottom, #000 0%, #000 88%, transparent 100%); mask-image: linear-gradient(to bottom, #000 0%, #000 88%, transparent 100%); }
        .sidebar:not(.horizontal).mirrored .instance-list { -webkit-mask-image: linear-gradient(to top, #000 0%, #000 88%, transparent 100%); mask-image: linear-gradient(to top, #000 0%, #000 88%, transparent 100%); }
        .sidebar.horizontal:not(.mirrored) .instance-list { -webkit-mask-image: linear-gradient(to right, #000 0%, #000 90%, transparent 100%); mask-image: linear-gradient(to right, #000 0%, #000 90%, transparent 100%); }
        .sidebar.horizontal.mirrored .instance-list { -webkit-mask-image: linear-gradient(to left, #000 0%, #000 90%, transparent 100%); mask-image: linear-gradient(to left, #000 0%, #000 90%, transparent 100%); }

        .sidebar-sections {
                border: 1px solid var(--border-color);
                border-radius: var(--border-radius-sm);
                overflow: hidden;
                width: calc(100% + 2 * var(--sidebar-padding-x, 12px));
                margin-left: calc(-1 * var(--sidebar-padding-x, 12px));
                margin-right: calc(-1 * var(--sidebar-padding-x, 12px));
        }

        .tools-group {
                display: flex;
                flex-direction: column;
                gap: 4px;
                width: 100%;
        }

        .sidebar:not(.horizontal) .tools-group { padding: 7px; }

        .sidebar:not(.horizontal) .tools-btn { min-height: 34px; }

        .tools-btn[data-tutorial="create-instance"] {
                background: var(--launcher-selected);
                color: var(--text-primary);
        }

        .tools-btn {
                background: transparent;
                border: 1px solid var(--border);
                color: var(--text-secondary);
                padding: 5px 8px;
                border-radius: var(--launcher-control-radius);
                font-size: 0.7rem;
                font-weight: 500;
                cursor: pointer;
                display: flex;
                align-items: center;
                gap: 6px;
                width: 100%;
                transition:
                        background 0.15s ease,
                        color 0.15s ease;
        }

        .tools-btn:is(:hover, :focus-visible),
        .tools-btn[data-tutorial="create-instance"]:is(:hover, :focus-visible) {
                background: var(--launcher-item-hover);
                border-color: var(--launcher-hover-border);
                color: var(--text-primary);
        }

        .tools-icon-wrap {
                position: relative;
                display: inline-flex;
                flex-shrink: 0;
        }

        .tools-btn :global(.icon-svg) {
                flex-shrink: 0;
        }

        /* ---- Barra acoplada arriba o abajo ---- */
        .sidebar.horizontal {
                width: 100%;
                height: 100%;
                flex-direction: row;
                align-items: stretch;
                gap: 14px;
                padding: var(--sidebar-edge-inset, 5px) 18px;
                border-right: none;
                border-block: 1px solid transparent;
        }

        .sidebar.horizontal.dock-top {
                border-bottom-color: var(--border);
        }

        .sidebar.horizontal.dock-bottom {
                border-top-color: var(--border);
        }

        .horizontal .sidebar-header {
                flex-direction: column;
                justify-content: center;
                flex-shrink: 0;
                width: 112px;
                margin: 0;
                padding: 0 8px 0 0;
                border-bottom: none;
                border-right: 1px solid var(--border);
        }

        .horizontal .sidebar-header.pin-target {
                margin: 0;
                padding: 0 8px 0 0;
                border-right-color: transparent;
        }

        /* Заголовок находится у правого края панели, поэтому разделитель
           должен быть слева от логотипа — между заголовком и списком сборок. */
        .sidebar.mirrored.horizontal .sidebar-header {
                padding: 0 0 0 8px;
                border-right: none;
                border-left: 1px solid var(--border);
        }

        .sidebar.mirrored.horizontal .sidebar-header.pin-target {
                padding: 0 0 0 8px;
                border-left-color: transparent;
        }

        .horizontal .sidebar-header {
                align-items: center;
                gap: 6px;
        }

        /* В горизонтальной панели кнопка занимает всю ширину и высоту шапки
           и выглядит как плитка сборки, а не как обрезанная кнопка с рамкой. */
        .horizontal .library-nav {
                flex: 1 1 auto;
                width: 100%;
                height: 100%;
                align-self: stretch;
                min-height: 0;
                flex-direction: column;
                justify-content: center;
                gap: 4px;
                padding: 6px 4px;
                box-sizing: border-box;
                font-size: .7rem;
                background: transparent;
                border-color: transparent;
        }

        .horizontal .library-nav:is(:hover, :focus-visible),
        .horizontal .library-nav[aria-current="page"] {
                background: var(--launcher-item-hover);
                border-color: var(--launcher-hover-border);
        }

        .horizontal .pin-zone-label {
                flex: 1;
                flex-direction: column;
                white-space: normal;
                text-align: center;
                line-height: 1.3;
        }

        .horizontal .sidebar-content {
                min-width: 0;
                padding: 0;
        }

        .horizontal .instances-section {
                display: grid;
                grid-template-columns: minmax(0, 1fr);
        }

        .dock-top .instances-section { grid-template-rows: minmax(0, 1fr) 18px; }
        .dock-bottom .instances-section { grid-template-rows: 18px minmax(0, 1fr); }

        .horizontal .instance-list {
                padding: 0 2px;
                width: 100%;
                min-width: 0;
        }

        .dock-top .instance-list { grid-row: 1; }
        .dock-bottom .instance-list { grid-row: 2; }
        .dock-top .section-head { grid-row: 2; }
        .dock-bottom .section-head { grid-row: 1; }

        .horizontal .section-head {
                width: 100%;
                align-self: center;
                margin-block: 0;
        }

        .hlist {
                display: grid;
                /* Слева — список прижат к левому краю; зеркальный режим переопределяет ниже. */
                justify-content: start;
                grid-auto-flow: column;
                grid-template-rows: repeat(var(--h-rows, 2), calc(var(--sidebar-row-height, 52px) * var(--h-card-scale)));
                grid-auto-columns: minmax(calc(190px * var(--h-card-scale)), calc(220px * var(--h-card-scale)));
                column-gap: 8px;
                row-gap: var(--h-row-gap);
                padding-block-start: var(--h-scrollbar);
                flex: 1 1 auto;
                min-height: 0;
                box-sizing: border-box;
                overflow-x: scroll;
                overflow-y: hidden;
        }


        /* Оставляем у внешнего края только тонкий зазор, а не половину свободной высоты. */
        .dock-top .hlist { align-content: start; }
        .dock-bottom .hlist { align-content: end; }

        /* Сверху оставляем такой же зазор, как полоса прокрутки снизу. */
        .sidebar .hlist::-webkit-scrollbar { height: var(--h-scrollbar); }
        .sidebar .hlist::-webkit-scrollbar-track { background: transparent; }
        .sidebar .hlist::-webkit-scrollbar-thumb {
                background: color-mix(in srgb, var(--accent, var(--primary)) 72%, transparent);
                border-radius: 999px;
        }
        .sidebar :global(.virtual-list-container)::-webkit-scrollbar { width: 4px; }
        .sidebar :global(.virtual-list-container)::-webkit-scrollbar-track { background: transparent; }
        .sidebar :global(.virtual-list-container)::-webkit-scrollbar-thumb {
                background: color-mix(in srgb, var(--accent, var(--primary)) 72%, transparent);
                border-radius: 999px;
        }
        /* Firefox не понимает ::-webkit-scrollbar: используем нативный тонкий скролл. */
        @supports (-moz-appearance: none) {
                .sidebar .hlist,
                .sidebar :global(.virtual-list-container) {
                        scrollbar-width: thin;
                        scrollbar-color: var(--accent, var(--primary)) transparent;
                }
        }

        .hcell {
                --virtual-row-height: var(--sidebar-row-height, 52px);
                min-width: 0;
        }

        .hcell :global(.instance-item) {
                margin-bottom: 0;
                width: 100%;
                height: var(--sidebar-row-height, 52px);
                zoom: var(--h-card-scale);
        }

        /* Arriba/abajo: un bloque único. Izquierda: descargas y cuenta; derecha: herramientas. */
        .horizontal .sidebar-sections {
                display: grid;
                /* Колонки задаются масштабом, а не содержимым: длинный текст кнопок
                   не должен растягивать колонку инструментов. */
                grid-template-columns: calc(180px * var(--ctl, 1)) minmax(0, 1fr);
                grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
                grid-template-areas:
                        "tools dl"
                        "tools user";
                height: 100%;
                max-height: 100%;
                box-sizing: border-box;
                gap: 8px;
                flex-shrink: 0;
                align-self: center;
                width: auto;
                max-width: 52%;
                margin: 0;
                padding: 8px;
                overflow: visible;
        }

        .horizontal .hs-dl {
                grid-area: dl;
                display: flex;
        }

        .horizontal .hs-dl :global(.sd-root) {
                flex: 1;
                display: flex;
        }

        .horizontal .sidebar-sections :global(.sd-header) {
                flex: 1;
                width: 100%;
                min-height: calc(44px * var(--ctl, 1));
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
        }

        .horizontal .tools-row {
                grid-area: tools;
                flex-direction: column;
                width: auto;
                min-height: 0;
                height: 100%;
                gap: 8px;
        }

        .horizontal .tools-btn {
                flex: 1 1 0;
                min-height: 0;
                height: auto;
                gap: calc(6px * var(--ctl, 1));
                padding: 0 calc(14px * var(--ctl, 1));
                font-size: calc(0.7rem * var(--ctl, 1));
                white-space: nowrap;
                min-width: 0;
                overflow: visible;
        }

        /* Загрузки и профиль масштабируются мягко: отступы, шрифт и аватарка через --ctl. */
        .horizontal .sidebar-sections :global(.sd-header) {
                padding: calc(8px * var(--ctl, 1)) calc(10px * var(--ctl, 1));
        }

        .horizontal .sidebar-sections :global(.sd-label) {
                font-size: calc(0.75rem * var(--ctl, 1));
        }

        .horizontal .user-slot :global(.user-profile) {
                gap: calc(10px * var(--ctl, 1));
                --profile-tile-padding: calc(8px * var(--ctl, 1));
        }

        .horizontal .user-slot :global(.user-avatar-wrapper) {
                width: calc(28px * var(--ctl, 1));
                height: calc(28px * var(--ctl, 1));
                min-width: calc(28px * var(--ctl, 1));
                min-height: calc(28px * var(--ctl, 1));
                flex-basis: calc(28px * var(--ctl, 1));
        }

        .horizontal .user-slot :global(.user-name) {
                font-size: calc(0.82rem * var(--ctl, 1));
        }

        .horizontal .user-slot :global(.user-type) {
                font-size: calc(0.72rem * var(--ctl, 1));
        }

        /* Запасной вариант, если даже при минимальном масштабе не влезает. */
        .horizontal .tools-label {
                min-width: 0;
                overflow: hidden;
                text-overflow: ellipsis;
        }

        .horizontal .user-slot {
                grid-area: user;
                display: flex;
        }

        .horizontal .user-slot :global(.user-profile) {
                flex: 1;
                margin-top: 0;
                border: 1px solid var(--border);
                border-radius: var(--border-radius-sm);
        }




        @media (max-width: 650px) {
                .sidebar:not(.horizontal) {
                        padding: 15px 10px 15px var(--sidebar-edge-inset, 5px);
                }

                .sidebar.dock-right:not(.horizontal) {
                        padding-inline: 10px var(--sidebar-edge-inset, 5px);
                }

                .sidebar:not(.horizontal) .sidebar-sections {
                        margin-inline: calc(-1 * var(--sidebar-edge-inset, 5px)) -10px;
                        width: calc(100% + 10px + var(--sidebar-edge-inset, 5px));
                }

                .sidebar.dock-right:not(.horizontal) .sidebar-sections {
                        margin-inline: -10px calc(-1 * var(--sidebar-edge-inset, 5px));
                }
        }

</style>
