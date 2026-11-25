<script lang="ts">
	import AccountSwitcher, {
		type SwitcherPlacement,
	} from "./AccountSwitcher.svelte";
	import { t } from "$lib/i18n";
	import AccountSwitchChevron from "./AccountSwitchChevron.svelte";
	import AccountAvatar from "$lib/components/profiles/AccountAvatar.svelte";

	let {
		username,
		avatarSvg,
		isPremium,
		userTypeLabel,
		placement = "up",
		arrowBefore = false,
		onclick,
	}: {
		username: string;
		avatarSvg: string;
		isPremium: boolean;
		userTypeLabel: string;
		placement?: SwitcherPlacement;
		arrowBefore?: boolean;
		onclick: () => void;
	} = $props();

	let open = $state(false);
	let rootEl: HTMLElement | undefined = $state();
</script>

<div
	class="user-profile"
	class:arrow-before={arrowBefore}
	bind:this={rootEl}
	{onclick}
	role="button"
	tabindex="0"
	onkeydown={(e) => (e.key === "Enter" || e.key === " ") && onclick()}
	style="cursor: pointer;"
>
	<div class="user-avatar-wrapper">
		<AccountAvatar {avatarSvg} {username} />
	</div>
	<div class="user-info">
		<span class="user-name" title={`${username} (${userTypeLabel})`}>
			{username}
			<span class="user-type" class:premium={isPremium}>
				({userTypeLabel})
			</span>
		</span>
	</div>
	<button
		type="button"
		class="user-switch"
		class:open
		title={t("userMenu.switchAccount")}
		aria-label={t("userMenu.switchAccount")}
		aria-expanded={open}
		onclick={(e) => {
			e.stopPropagation();
			open = !open;
		}}
		onkeydown={(e) => e.stopPropagation()}
	>
		<AccountSwitchChevron {placement} {open} />
	</button>
</div>

{#if rootEl}
	<AccountSwitcher
		anchor={rootEl}
		{placement}
		{open}
		onclose={() => (open = false)}
	/>
{/if}

<style>
	.user-avatar-wrapper {
		width: 28px;
		height: 28px;
		min-width: 28px;
		min-height: 28px;
		aspect-ratio: 1;
		align-self: center;
		border-radius: var(--border-radius-sm);
		border: 1px solid var(--border);
		flex: 0 0 28px;
		background-color: var(--bg-card);
		overflow: hidden;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.user-avatar-wrapper :global(svg),
	.user-avatar-wrapper :global(img) {
		width: 100%;
		height: 100%;
		display: block;
		border-radius: inherit;
		object-fit: cover;
		flex: none;
	}

	.user-profile {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: var(--profile-tile-padding, 8px);
		overflow: hidden;
		margin-top: auto;
		background: transparent;
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		transition: background .15s ease;
	}

	.user-profile:is(:hover, :focus-within) {
		background: var(--launcher-hover-overlay);
		border-color: var(--launcher-hover-border);
	}

	.user-profile.arrow-before .user-switch {
		order: -1;
		margin: calc(-1 * var(--profile-tile-padding, 8px)) 0 calc(-1 * var(--profile-tile-padding, 8px)) calc(-1 * var(--profile-tile-padding, 8px));
	}

	.user-info {
		display: flex;
		flex: 1;
		min-width: 0;
		overflow: hidden;
	}

	.user-name {
		font-size: 0.82rem;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.user-type {
		font-size: 0.72rem;
		font-weight: 500;
		color: var(--text-secondary);
	}

	.user-type.premium {
		color: var(--accent);
		font-weight: 600;
	}

	.user-switch {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		/* Растянута на всю высоту плитки и касается её краёв. */
		align-self: stretch;
		width: 36px;
		margin: calc(-1 * var(--profile-tile-padding, 8px)) calc(-1 * var(--profile-tile-padding, 8px)) calc(-1 * var(--profile-tile-padding, 8px)) 0;
		padding: 0;
		background: transparent;
		border: none;
		border-radius: 0;
		color: var(--text-secondary);
		cursor: pointer;
		transition:
			background 0.15s,
			color 0.15s;
	}

	.user-switch:is(:hover, :focus-visible),
	.user-switch.open {
		background: var(--surface-hover);
		color: var(--text-primary);
	}

	@media (max-width: 650px) {
		.user-profile {
			justify-content: center;
		}

		.user-info {
			display: none;
		}
		.user-switch,
		.user-profile.arrow-before .user-switch { width: 22px; margin: 0; }
	}
</style>
