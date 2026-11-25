<script lang="ts">
	import { launcherStore } from "$lib/state/state.svelte";
	import { t } from "$lib/i18n";
	import { switchUser } from "$lib/api/cubicApi";
	import { markLocalSettingsChange } from "$lib/api/launcherService";
	import { getAccountProvider } from "$lib/utils/accountProviders";
	import { windowSlide } from "$lib/utils/animations";
	import { accountAvatar, createAvatarRefreshSession } from "$lib/state/avatarCache.svelte";
	import AccountAvatar from "$lib/components/profiles/AccountAvatar.svelte";

	export type SwitcherPlacement = "up" | "down" | "left" | "right";

	let {
		anchor,
		placement,
		open,
		onclose,
	}: {
		anchor: HTMLElement;
		placement: SwitcherPlacement;
		/** Родитель держит меню смонтированным и управляет видимостью через open. */
		open: boolean;
		onclose: () => void;
	} = $props();

	const WIDTH = 230;

	// Render the popup at document.body level, like the launcher's Select menus.
	// This keeps it outside transformed/overflow-clipped sidebar containers, so
	// Svelte can run the full outro transition instead of the parent repainting it.
	function portal(el: HTMLElement) {
		document.body.appendChild(el);
		return {
			destroy() {
				el.remove();
			},
		};
	}
	let style = $state("");
	const users = $derived(launcherStore.settings.user);
	const activeIdx = $derived(launcherStore.settings.active_user_idx ?? 0);

	// Пересчитываем позицию при каждом открытии, чтобы она не устаревала
	// после смены стороны панели, изменения окна или перетаскивания.
	$effect(() => {
		if (!open) return;
		const rect = anchor.getBoundingClientRect();
		const vw = window.innerWidth;
		const vh = window.innerHeight;
		const clampLeft = Math.max(8, Math.min(rect.left, vw - WIDTH - 8));
		if (placement === "up")
			style = `left:${clampLeft}px;bottom:${vh - rect.top + 6}px;`;
		else if (placement === "down")
			style = `left:${clampLeft}px;top:${rect.bottom + 6}px;`;
		else if (placement === "right") {
			const left = Math.min(rect.right + 8, vw - WIDTH - 8);
			style = `left:${Math.max(8, left)}px;bottom:${Math.max(8, vh - rect.bottom)}px;`;
		} else {
			const right = Math.min(vw - rect.left + 8, vw - WIDTH - 8);
			style = `right:${Math.max(8, right)}px;bottom:${Math.max(8, vh - rect.bottom)}px;`;
		}
	});


	const refreshAvatars = createAvatarRefreshSession();
	$effect(() => refreshAvatars(open ? users : []));

	async function choose(idx: number) {
		onclose();
		if (idx === activeIdx) return;
		launcherStore.settings.active_user_idx = idx;
		markLocalSettingsChange();
		try {
			const user = await Promise.race([
				switchUser(idx),
				new Promise<null>((resolve) => window.setTimeout(() => resolve(null), 8000)),
			]);
			if (user) launcherStore.settings.user[idx] = user;
		} catch (error) {
			// Network/auth failures must not leave the switcher or launcher interaction blocked.
			console.warn("Account switch request failed:", error);
		}
	}

	function onPointerDown(e: PointerEvent) {
		if (!open) return;
		const target = e.target as Node;
		// Не считаем всю область профиля частью меню: любой клик по ней
		// (например, по кнопке открытия другого меню) должен закрыть список.
		// Исключение — только сама кнопка-переключатель, которая управляет им.
		const toggleButton = anchor.querySelector(".user-switch, .sc-switch");
		if (menuEl?.contains(target) || toggleButton?.contains(target)) return;
		onclose();
	}

	let menuEl: HTMLDivElement | undefined = $state();
	const offset = $derived(
		placement === "up"
			? "bottom"
			: placement === "down"
				? "top"
				: placement === "right"
					? "left"
					: "right",
	);
</script>

<svelte:window
	onpointerdown={onPointerDown}
	onkeydowncapture={(e) => {
		if (e.key !== "Escape" || e.defaultPrevented || !open) return;
		e.preventDefault();
		if (e.repeat) return;
		const focused = document.activeElement;
		if (focused instanceof HTMLElement && (anchor.contains(focused) || menuEl?.contains(focused))) {
			const toggle = anchor.querySelector<HTMLElement>(".user-switch, .sc-switch");
			if (menuEl?.contains(focused)) toggle?.focus({ preventScroll: true });
			const target = document.activeElement;
			// Esc can switch Chromium to keyboard modality even when opened by pointer.
			if (target instanceof HTMLElement) {
				target.classList.add("account-escape-focus");
				target.addEventListener("blur", () => target.classList.remove("account-escape-focus"), { once: true });
			}
		}
		onclose();
	}}
/>

{#if open}
<div
	use:portal
	class="switcher"
	bind:this={menuEl}
	in:windowSlide={{ edge: offset, distance: 12, fade: true }}
	out:windowSlide={{ edge: offset, distance: 12, fade: true }}
	style={style}
	role="menu"
	aria-label={t("userMenu.switchAccount")}
>
	{#each users as user, i (i)}
		<button
			type="button"
			class="row"
			class:active={i === activeIdx}
			role="menuitemradio"
			aria-checked={i === activeIdx}
			onclick={() => choose(i)}
		>
			<span class="ava">
				<AccountAvatar
					avatarSvg={accountAvatar(user)}
					username={user.username}
				/>
			</span>
			<span class="meta">
				<span class="name">{user.username}</span>
				<span class="type">
					{t(`userMenu.short.${getAccountProvider(user)}`)}
				</span>
			</span>
			{#if i === activeIdx}<span class="dot" aria-hidden="true"></span>{/if}
		</button>
	{/each}
</div>
{/if}

<style>
	:global(.account-escape-focus:focus-visible) { outline: none; }
	.switcher {
		position: fixed;
		z-index: 1000;
		width: 230px;
		max-height: min(50vh, 320px);
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 4px;
		background: var(--bg-sidebar);
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		box-shadow: var(--shadow-sm);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 8px;
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--border-radius-sm);
		color: var(--text-secondary);
		font-family: inherit;
		text-align: left;
		cursor: pointer;
		transition:
			background 0.15s,
			color 0.15s;
	}
	.row.active {
		background: var(--launcher-selected);
		border-color: var(--selected-border);
		color: var(--text-primary);
	}
	.row:is(:hover, :focus-visible) {
		background: var(--launcher-item-hover);
		border-color: var(--launcher-hover-border);
		color: var(--text-primary);
	}
	.ava {
		/* Keep the avatar a fixed square: the popup row must never stretch it. */
		flex: 0 0 24px;
		width: 24px;
		min-width: 24px;
		height: 24px;
		aspect-ratio: 1 / 1;
		display: block;
		box-sizing: border-box;
		line-height: 0;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		overflow: hidden;
	}
	.ava :global(img) {
		display: block;
		width: 100%;
		min-width: 0;
		max-width: none;
		height: 100%;
		min-height: 0;
		max-height: none;
		object-fit: cover;
		image-rendering: pixelated;
	}
	.meta {
		display: flex;
		flex-direction: column;
		min-width: 0;
		flex: 1;
	}
	.name {
		font-size: 0.8rem;
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.type {
		font-size: 0.66rem;
		color: var(--text-muted);
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--accent);
		flex-shrink: 0;
	}
</style>
