<script lang="ts">
	import { t } from "$lib/i18n";
	import AccountAvatar from "./AccountAvatar.svelte";
	interface Account {
		username: string;
		user_type: string;
	}

	interface Props {
		user: Account;
		typeLabel: string;
		isActive: boolean;
		isSelected: boolean;
		avatarSvg: string;
		onselect?: () => void;
	}

	let { user, typeLabel, isActive, isSelected, avatarSvg, onselect }: Props =
		$props();

	function handleClick() {
		onselect?.();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" || e.key === " ") {
			e.preventDefault();
			onselect?.();
		}
	}
</script>

<div
	class="account-item"
	class:active={isActive}
	class:selected={isSelected}
	onclick={handleClick}
	role="button"
	tabindex="0"
	onkeydown={handleKeydown}
>
	<div class="account-info">
		<span class="account-name-row">
			<span class="account-name">{user.username}</span>
			{#if isActive}
				<span class="active-badge">{t("userMenu.active")}</span>
			{/if}
		</span>
		<span class="account-type" title={typeLabel}>{typeLabel}</span>
	</div>
	<div class="account-avatar">
		<AccountAvatar avatarSvg={avatarSvg} username={user.username} />
	</div>
</div>

<style>
	.account-item {
		display: flex;
		align-items: stretch;
		gap: 12px;
		min-height: 46px;
		padding: 0 0 0 12px;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--border-radius-sm);
		overflow: hidden;
		cursor: pointer;
		transition:
			border-color 0.15s ease,
			background-color 0.15s ease;
	}

	.account-item:hover {
		background: var(--surface-hover);
	}

	.account-item:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}

	.account-item.selected {
		background: var(--bg-item-active);
		border-color: var(--accent);
	}

	.account-info {
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 1px;
		min-width: 0;
		flex: 1;
		padding: 6px 0;
	}

	.account-name-row {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
	}

	.account-name {
		font-size: 0.88rem;
		font-weight: 600;
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.account-type {
		font-size: 0.64rem;
		font-weight: 600;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	/* Keep the card square at the avatar edge before any image has loaded. */
	.account-avatar {
		position: relative;
		flex: none;
		align-self: center;
		width: 26%;
		max-width: 110px;
		aspect-ratio: 1;
		box-sizing: border-box;
		border-left: 1px solid var(--border);
		background-color: var(--bg-input);
		overflow: hidden;
	}

	.account-avatar :global(img) {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		display: block;
		object-fit: contain;
		image-rendering: pixelated;
	}

	.active-badge {
		flex-shrink: 0;
		font-size: 0.55rem;
		background: var(--accent);
		color: var(--accent-text);
		padding: 2px 6px;
		border-radius: 999px;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.3px;
	}
</style>
