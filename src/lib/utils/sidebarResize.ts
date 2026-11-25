export const sidebarSizes = {
	vertical: { min: 60, compact: 70, compactMax: 100, threshold: 135, normalMin: 180, normal: 260, max: 420 },
	horizontal: { min: 56, compact: 66, compactMax: 86, threshold: 104, normalMin: 148, normal: 176, max: 244 },
} as const;

export type SidebarAxis = keyof typeof sidebarSizes;

export function limitSidebarSize(axis: SidebarAxis, size: number, compactOnly = false) {
	const { min, compactMax, max } = sidebarSizes[axis];
	return Math.max(min, Math.min(compactOnly ? compactMax : max, Math.round(size)));
}

export function sidebarGeometry(axis: SidebarAxis, requested: number, compactOnly = false) {
	const limits = sidebarSizes[axis];
	const compact = compactOnly || requested <= limits.threshold;
	const size = compact
		? limitSidebarSize(axis, requested, true)
		: Math.max(limits.normalMin, limitSidebarSize(axis, requested));
	const scale = Math.min(1, Math.max(0.85, size / (compact ? limits.compact : limits.normal)));
	return { compact, size, scale };
}