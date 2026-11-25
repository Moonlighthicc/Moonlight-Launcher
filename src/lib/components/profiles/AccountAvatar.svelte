<script lang="ts">
	interface Props {
		avatarSvg: string;
		username: string;
	}
	let { avatarSvg, username }: Props = $props();
	const fallbackSrc = "/images/moonlight-avatar.png";
	const avatarSrc = $derived(
		avatarSvg
			? `data:image/svg+xml,${encodeURIComponent(avatarSvg)}`
			: fallbackSrc,
	);
</script>

{#key avatarSrc}
	<img
		src={avatarSrc}
		alt={username}
		onerror={(event) => {
			if (event.currentTarget.getAttribute("src") !== fallbackSrc) {
				event.currentTarget.setAttribute("src", fallbackSrc);
			}
		}}
	/>
{/key}
