<script lang="ts">
	let { list = false }: { list?: boolean } = $props();
</script>

<div class="market-skeleton sk-delayed" class:list aria-hidden="true">
	<div class="skeleton-header">
		<span class="sk-block skeleton-icon"></span>
		<div class="skeleton-identity">
			<span class="sk-block skeleton-line title"></span>
			<span class="sk-block skeleton-line author"></span>
		</div>
	</div>
	<div class="skeleton-description">
		<span class="sk-block skeleton-line"></span>
		<span class="sk-block skeleton-line short"></span>
	</div>
	<div class="skeleton-footer">
		<span class="sk-block skeleton-line author"></span>
		<span class="sk-block skeleton-button"></span>
	</div>
</div>

<style>
	.market-skeleton {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		height: 100%;
		box-sizing: border-box;
		padding: var(--market-card-padding);
		border: var(--border-width) solid var(--border);
		border-radius: var(--market-card-radius);
		background: var(--surface-card);
	}
	.skeleton-header,
	.skeleton-footer {
		display: flex;
		align-items: center;
		gap: var(--market-item-gap);
	}
	.skeleton-icon {
		width: var(--market-icon-size);
		height: var(--market-icon-size);
		flex-shrink: 0;
	}
	.skeleton-identity,
	.skeleton-description {
		display: flex;
		flex-direction: column;
		gap: 8px;
		flex: 1;
	}
	.skeleton-line {
		height: 9px;
		width: 100%;
	}
	.skeleton-line.title {
		height: 13px;
		width: 80%;
	}
	.skeleton-line.author {
		width: 40%;
	}
	.skeleton-line.short {
		width: 70%;
	}
	.skeleton-footer {
		justify-content: space-between;
		padding-top: 10px;
		border-top: var(--border-width) solid var(--border);
	}
	.skeleton-button {
		width: 64px;
		height: 28px;
	}
	.list {
		gap: 6px;
		padding-block: 10px;
	}
	.list .skeleton-description {
		display: none;
	}
	.list .skeleton-footer {
		margin-top: auto;
		padding-top: 6px;
	}
	@container market (min-width: 620px) {
		.list {
			flex-direction: row;
			align-items: center;
			gap: 16px;
		}
		.list .skeleton-header {
			flex: 1;
		}
		.list .skeleton-footer {
			margin: 0;
			padding: 0 0 0 16px;
			border-top: 0;
			border-inline-start: var(--border-width) solid var(--border);
		}
		.list .skeleton-footer .author {
			width: 54px;
		}
	}
</style>
