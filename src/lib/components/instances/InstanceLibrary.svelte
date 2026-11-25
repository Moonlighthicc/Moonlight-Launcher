<script lang="ts">
	import Icon from "$lib/icons/Icon.svelte";
	import { getPreviewIconSrc, getLoaderLogo } from "$lib/icons/logos";
	import { t } from "$lib/i18n";
	import { launcherStore, showErrorParsed } from "$lib/state/state.svelte";
	import { commitManualOrder, commitPinOrder, orderInstances } from "$lib/state/sidebarLayoutState.svelte";
	import { InstanceDrag } from "$lib/utils/instanceDrag.svelte";
	import SortFilter from "$lib/components/layout/Sidebar/SortFilter.svelte";
	import InstanceDragGhost from "$lib/components/layout/Sidebar/InstanceDragGhost.svelte";
	import { onDestroy } from "svelte";
	import { flip } from "svelte/animate";
	import { isInstanceDownloading, playInstance } from "$lib/api/instancePlay";
	import { InstState, type InstanceDto } from "$lib/types/types";
	import { formatPlaytime, hasPlaytime } from "$lib/utils/playtime";
	import { SvelteSet } from "svelte/reactivity";

	let { onselect, oncreate, oninstancemenu }: {
		onselect: (instance: InstanceDto) => void;
		oncreate: () => void;
		oninstancemenu: (event: MouseEvent, anchor?: HTMLElement) => void;
	} = $props();

	let search = $state("");
	const pending = new SvelteSet<string>();
	const badCovers = new SvelteSet<string>();
	const instances = $derived(orderInstances(launcherStore.loadedInstances));
	const query = $derived(search.trim().toLowerCase());

	/** Перетаскивание карточек меняет порядок так же, как в боковой панели. */
	const drag = new InstanceDrag({
		list: () => instances,
		indexAt: (x, y) => cardIndexAt(x, y),
		inPinZone: () => false,
		autoScroll: (x, y) => {
			const zone = 80;
			const speed = 14;
			const top = window.innerHeight * 0.0;
			let delta = 0;
			if (y < top + zone) delta = -speed;
			else if (y > window.innerHeight - zone) delta = speed;
			if (!delta) return false;
			const before = window.scrollY;
			window.scrollBy(0, delta);
			const scroller = document.querySelector<HTMLElement>(".library .cards")?.closest<HTMLElement>(".virtual-list-container, .library, .instance-surface");
			if (scroller && scroller.scrollHeight > scroller.clientHeight) {
				const sBefore = scroller.scrollTop;
				scroller.scrollTop += delta;
				return scroller.scrollTop !== sBefore || window.scrollY !== before;
			}
			return window.scrollY !== before;
		},
		onreorder: (instance, next) => {
			if (instance.pinned) commitPinOrder(next.filter((i) => i.pinned).map((i) => i.uuid));
			else commitManualOrder(next.filter((i) => !i.pinned).map((i) => i.uuid));
		},
		onpinchange: () => {},
	});
	onDestroy(() => drag.destroy());

	/**
	 * Индекс карточки, ближайшей к указателю, в порядке отображения.
	 * Считаем по offset-координатам (без transform), иначе анимация перестановки
	 * сдвигает карточки, меняет результат и вызывает дрожание соседних карточек.
	 */
	function cardIndexAt(x: number, y: number): number {
		const cards = Array.from(document.querySelectorAll<HTMLElement>(".library .cards [data-instance-uuid]"));
		if (!cards.length) return 0;
		const parent = cards[0].offsetParent as HTMLElement | null;
		if (!parent) return 0;
		const pr = parent.getBoundingClientRect();
		const px = x - pr.left + parent.scrollLeft;
		const py = y - pr.top + parent.scrollTop;
		let best = 0;
		let bestDist = Infinity;
		cards.forEach((el, index) => {
			const cx = el.offsetLeft + el.offsetWidth / 2;
			const cy = el.offsetTop + el.offsetHeight / 2;
			const dist = (cx - px) ** 2 + (cy - py) ** 2;
			if (dist < bestDist) {
				bestDist = dist;
				best = index;
			}
		});
		return best;
	}

	const shown = $derived(query ? instances.filter((item) =>
		`${item.name} ${item.version} ${item.loader}`.toLowerCase().includes(query)) : drag.display);

	async function quickPlay(instance: InstanceDto) {
		if (pending.has(instance.uuid)) return;
		pending.add(instance.uuid);
		try { await playInstance(instance); }
		catch (error) { showErrorParsed(error); }
		finally { pending.delete(instance.uuid); }
	}
</script>

<section class="library" aria-labelledby="library-title">
	<div class="library-inner">
		<header class="library-head">
			<div>
				<div class="eyebrow"><img src="/favicon.png" alt="" width="16" height="16" /> MOONLIGHT LAUNCHER</div>
				<h1 id="library-title">{t("library.title")}</h1>
				<p>{t("library.description")}</p>
			</div>
			<button type="button" class="btn-primary create launcher-create" onclick={oncreate}>
				<Icon name="nav:create" size={17} /> {t("sidebar.createInstance")}
			</button>
		</header>

		{#if instances.length}
			<div class="toolbar">
				<div class="heading">{t("sidebar.yourInstances")} <span class="total">{instances.length}</span></div>
				<div class="tools">
					<div class="library-sort"><SortFilter /></div>
					<label class="search">
						<Icon name="ui:search" size={16} />
						<input type="search" bind:value={search} placeholder={t("library.search")} aria-label={t("library.search")} />
					</label>
				</div>
			</div>
		{/if}

		{#if shown.length}
			<div class="cards">
				{#each shown as instance (instance.uuid)}
					{@const running = instance.status === InstState.Started}
					{@const starting = instance.status === InstState.Starting || pending.has(instance.uuid)}
					{@const downloading = isInstanceDownloading(instance)}
					{@const cover = instance.cover_image}
					<article class="card" class:dragging={drag.active && drag.uuid === instance.uuid} data-instance-uuid={instance.uuid} animate:flip={{ duration: drag.active ? 120 : 0 }} onpointerdown={(event) => drag.start(event, instance)}>
						<button type="button" class="open-card" aria-label={t("library.openDetails", { name: instance.name })}
							onclick={() => onselect(instance)} oncontextmenu={(event) => oninstancemenu(event)}></button>
						<div class="art" aria-hidden="true">
							<img class="emblem" src={getPreviewIconSrc(instance.icon)} alt="" loading="lazy" decoding="async" />
							{#if cover && !badCovers.has(cover)}
								<img class="cover" src={getPreviewIconSrc(cover)} alt="" loading="lazy" decoding="async" onerror={() => badCovers.add(cover)} />
							{/if}
						</div>
						<div class="tags">
							{#if instance.pinned}<span class="tag" title={t("sidebar.unpin")}><Icon name="ui:pin" size={13} /></span>{/if}
							{#if downloading || starting || running}
								<span class="tag status" class:running><i></i>{t(downloading ? "downloadProgress.statusGeneric" : starting ? "instanceView.launching" : "instanceView.status.started")}</span>
							{/if}
						</div>
						<button type="button" class="more" data-no-drag aria-haspopup="menu" title={t("library.actions", { name: instance.name })}
							aria-label={t("library.actions", { name: instance.name })}
							onclick={(event) => oninstancemenu(event, event.currentTarget)}
							oncontextmenu={(event) => oninstancemenu(event)}>
							<span aria-hidden="true">•••</span>
						</button>
						{#if !running && !starting && !downloading}
							<button type="button" class="btn-primary quick" data-no-drag aria-label={t("library.quickPlay", { name: instance.name })}
								onclick={() => void quickPlay(instance)} oncontextmenu={(event) => oninstancemenu(event)}>
								<Icon name="ui:play" size={15} /> {t("instanceView.playBtn")}
							</button>
						{/if}
						<div class="card-info">
							<h2 title={instance.name}>{instance.name}</h2>
							<div class="meta"><img src={getLoaderLogo(instance.loader)} alt="" width="15" height="15" /> <span>{instance.loader} · {instance.version}</span></div>
							{#if hasPlaytime(instance.playtime_seconds)}
								<div class="meta"><Icon name="instance:clock" size={14} /> <span>{t("instanceView.playtime", { time: formatPlaytime(instance.playtime_seconds, t) })}</span></div>
							{/if}
						</div>
					</article>
				{/each}
			</div>
		{:else}
			<div class="empty-library">
				<div class="empty-icon"><Icon name={query ? "ui:search" : "instance:grid"} size={28} /></div>
				<h2>{t(query ? "library.noResults" : "library.emptyTitle")}</h2>
				{#if query}
					<button type="button" class="btn-secondary" onclick={() => (search = "")}>{t("library.clearSearch")}</button>
				{:else}
					<p>{t("library.emptyDescription")}</p>
				{/if}
			</div>
		{/if}
	</div>
{#if drag.active && drag.dragged}
	<InstanceDragGhost instance={drag.dragged} x={drag.x} y={drag.y} />
{/if}
</section>

<style>
	.library {
		width: 100%; min-width: 0; overflow: auto; container-type: inline-size;
		background: radial-gradient(ellipse 85% 340px at 75% 0%, var(--launcher-glow), transparent),
			linear-gradient(var(--launcher-veil), transparent 460px);
	}
	.library-inner { max-width: 1520px; margin-inline: auto; padding: clamp(14px, 2cqi, 26px) clamp(18px, 4cqi, 52px) clamp(18px, 4cqi, 52px); }
	.library-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px 16px; margin-bottom: 14px; }
	.eyebrow { display: flex; align-items: center; gap: 7px; color: var(--text-secondary); font-size: .68rem; font-weight: 750; letter-spacing: .16em; }
	.eyebrow img { object-fit: contain; }
	h1, h2, p { margin: 0; }
	h1 { margin-block: 4px; font-size: clamp(1.75rem, 3.5cqi, 2.75rem); font-weight: 800; line-height: 1.07; letter-spacing: -.055em; }
	p { color: var(--text-secondary); line-height: 1.55; font-size: .9rem; }
	.create, .quick { display: inline-flex; align-items: center; justify-content: center; gap: 8px; border-radius: var(--launcher-control-radius); font-weight: 750; }
	.create { min-height: 40px; padding: 8px 14px; }
	.toolbar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 16px; padding-bottom: 12px; }
	.tools { display: flex; align-items: center; gap: 10px; min-width: 0; max-width: 100%; }
	.library-sort { min-width: 0; max-width: 45%; }
	.library-sort :global(.sort-btn) { margin: 0; min-height: 36px; }
	.card.dragging { opacity: 0.5; }
	.heading { display: flex; align-items: center; gap: 8px; font-weight: 750; }
	.total { padding: 3px 7px; border-radius: var(--launcher-control-radius); color: var(--text-secondary); background: var(--launcher-surface); font-size: .72rem; font-variant-numeric: tabular-nums; }
	.search { display: flex; align-items: center; gap: 8px; width: 250px; min-width: 0; box-sizing: border-box; padding-inline: 10px; border: 1px solid var(--border); border-radius: var(--launcher-control-radius); background: var(--launcher-surface); color: var(--text-secondary); }
	.search:focus-within { border-color: var(--accent); }
	.search input { flex: 1; width: 100%; min-width: 0; min-height: 36px; padding: 0; border: 0; background: transparent; color: var(--text-primary); font: inherit; font-size: .8rem; outline: none; }
	.search input::placeholder { color: var(--text-secondary); }
	.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 205px), 1fr)); gap: clamp(12px, 1.8cqi, 24px); }
	.card { position: relative; isolation: isolate; overflow: hidden; border: 1px solid var(--border); border-radius: var(--launcher-radius); background: var(--launcher-surface); transition: background-color .16s, border-color .16s, box-shadow .16s; }
	.card:hover, .card:focus-within { background-color: var(--launcher-item-hover); border-color: var(--launcher-hover-border); box-shadow: 0 0 0 1px var(--launcher-hover-border), 0 8px 22px #0003; }
	.art { position: relative; display: grid; place-items: center; overflow: hidden; aspect-ratio: 4 / 3; background: var(--bg-input); box-shadow: inset 0 -1px var(--border); }
	.emblem { width: 35%; max-height: 55%; object-fit: contain; }
	.cover { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
	.open-card { position: absolute; inset: 0; border: 0; background: transparent; padding: 0; cursor: pointer; z-index: 1; }
	.card-info { min-height: 78px; padding: 13px 14px 15px; }
	.card-info h2 { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 1rem; line-height: 1.3; letter-spacing: -.025em; }
	.meta { display: flex; align-items: center; gap: 6px; margin-top: 6px; color: var(--text-secondary); font-size: .73rem; }
	.meta img { object-fit: contain; }
	.meta span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.tags { position: absolute; top: 10px; inset-inline: 10px 48px; display: flex; flex-wrap: wrap; gap: 5px; pointer-events: none; }
	.tag, .more { display: inline-flex; align-items: center; justify-content: center; min-height: 28px; padding: 4px 7px; border-radius: 8px; background: #181922eb; color: #fafaff; border: 1px solid #ffffff29; font-size: .65rem; font-weight: 700; }
	.status { gap: 5px; }
	.status i { width: 6px; height: 6px; border-radius: 50%; background: #ffd080; }
	.status.running i { background: #75e0ab; }
	.more { position: absolute; top: 10px; inset-inline-end: 10px; width: 32px; height: 32px; padding: 0; font-size: .85rem; letter-spacing: 1px; cursor: pointer; z-index: 2; }
	.more:is(:hover, :focus-visible) { background: color-mix(in srgb, var(--accent) 24%, #181922); border-color: var(--launcher-hover-border); }
	.quick { position: absolute; top: min(32%, 112px); inset-inline-start: 50%; transform: translate(-50%, 5px); opacity: 0; white-space: nowrap; padding: 9px 14px; min-height: 36px; z-index: 2; transition: opacity .16s, transform .16s; box-shadow: 0 6px 20px #0005; }
	.card:hover .quick, .card:focus-within .quick { opacity: 1; transform: translate(-50%, 0); }
	.card button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
	.empty-library { min-height: 220px; display: grid; place-content: center; justify-items: center; gap: 12px; padding: 24px; border: 1px dashed var(--border); border-radius: var(--launcher-radius); background: var(--launcher-surface); text-align: center; }
	.empty-library h2 { font-size: 1.16rem; }
	.empty-icon { width: 56px; height: 56px; display: grid; place-items: center; border-radius: var(--launcher-radius); background: var(--launcher-glow); color: var(--text-primary); }
	@container (max-width: 540px) {
		.tools { flex-basis: 100%; }
		.search { flex: 1; }
	}
	@media (hover: none) { .quick { opacity: 1; transform: translate(-50%, 0); } }
</style>