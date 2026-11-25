/** Reduce the font size until the text fits its (fixed-width) element. */
export function fitText(node: HTMLElement, options?: string | { text: string; minSize: number }) {
	const range = document.createRange();
	function fit() {
		node.style.fontSize = "";
		node.style.whiteSpace = "nowrap";
		const base = parseFloat(getComputedStyle(node).fontSize) || 12;
		const avail = node.clientWidth;
		range.selectNodeContents(node);
		const need = range.getBoundingClientRect().width;
		if (avail > 0 && need > avail) {
			const minSize = typeof options === "object" ? options.minSize : 6;
			node.style.fontSize = `${Math.max(minSize, (base * (avail - 0.5)) / need)}px`;
		}
	}
	fit();
	const observer =
		typeof ResizeObserver === "function"
			? new ResizeObserver(() => fit())
			: null;
	observer?.observe(node);
	return {
		update(next: typeof options) {
			options = next;
			fit();
		},
		destroy() {
			observer?.disconnect();
		},
	};
}
