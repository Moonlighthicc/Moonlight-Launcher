import { expect, test } from "bun:test";
import {
	interfacePreferences,
	INTERFACE_SCALES,
	DEFAULT_INTERFACE_PREFERENCES,
} from "../../../src/lib/utils/interfacePreferences.ts";
import {
	applyInterfaceScale,
	applyInterfaceDensity,
	applyInterfaceFont,
} from "../../../src/lib/api/interfaceAppearance.ts";

test("invalid and legacy interface preferences recover without altering the source", () => {
	for (const value of [
		undefined,
		null,
		{},
		{ scale: -1, density: "unknown" },
		{ scale: NaN },
		{ scale: Infinity },
	]) {
		expect(interfacePreferences(value)).toEqual({
			scale: 100,
			density: "theme",
			font: "theme",
		});
	}
	const saved = Object.freeze({
		scale: 250,
		density: "comfortable",
		font: "Cantarell",
	});
	expect(interfacePreferences(saved)).toBe(saved);
	for (const scale of [90, 100, 110, 150, 180, 200, 250, 300]) {
		expect(interfacePreferences({ scale }).scale).toBe(scale);
	}
	expect(INTERFACE_SCALES).toEqual([90, 100, 110, 150, 180, 200, 250, 300]);
	expect(interfacePreferences({ scale: 125 }).scale).toBe(110);
	expect(interfacePreferences()).toBe(DEFAULT_INTERFACE_PREFERENCES);
	expect(interfacePreferences({ scale: 90 })).toEqual({
		scale: 90,
		density: "theme",
		font: "theme",
	});
	expect(interfacePreferences({ scale: 90, font: "  " }).font).toBe("theme");
});

test("native zoom serializes changes, deduplicates successful requests and recovers after failure", async () => {
	const windowDescriptor = Object.getOwnPropertyDescriptor(
		globalThis,
		"window",
	);
	const tauriDescriptor = Object.getOwnPropertyDescriptor(
		globalThis,
		"isTauri",
	);
	const calls = [];
	let rejectNext = false;
	let inFlight = 0;
	let maxInFlight = 0;
	Object.defineProperty(globalThis, "isTauri", {
		configurable: true,
		value: true,
	});
	Object.defineProperty(globalThis, "window", {
		configurable: true,
		value: {
			__TAURI_INTERNALS__: {
				metadata: {
					currentWindow: { label: "main" },
					currentWebview: { label: "main" },
				},
				async invoke(command, payload) {
					calls.push({ command, ...payload });
					maxInFlight = Math.max(maxInFlight, ++inFlight);
					await new Promise((resolve) => setTimeout(resolve, 2));
					inFlight--;
					if (rejectNext) {
						rejectNext = false;
						throw Error("Zoom unavailable");
					}
				},
			},
		},
	});
	try {
		const requests = [90, 110, 200, 250, 300, 100, 100].map(
			applyInterfaceScale,
		);
		expect(requests[5]).toBe(requests[6]);
		await Promise.all(requests);
		expect(calls.map((call) => call.value)).toEqual([
			0.9, 1.1, 2, 2.5, 3, 1,
		]);
		expect(maxInFlight).toBe(1);
		expect(
			calls.every(
				(call) =>
					call.command === "plugin:webview|set_webview_zoom" &&
					call.label === "main",
			),
		).toBe(true);
		rejectNext = true;
		await expect(applyInterfaceScale(250)).rejects.toThrow(
			"Zoom unavailable",
		);
		await applyInterfaceScale(250);
		await applyInterfaceScale(999);
		expect(calls.slice(-3).map((call) => call.value)).toEqual([
			2.5, 2.5, 1,
		]);

		const beforePreview = calls.length;
		const obsolete = new AbortController();
		const skipped = applyInterfaceScale(300, {
			delay: 25,
			duration: 120,
			signal: obsolete.signal,
		});
		obsolete.abort();
		const transition = applyInterfaceScale(200, {
			delay: 25,
			duration: 120,
		});
		expect(calls).toHaveLength(beforePreview);
		await Promise.all([skipped, transition]);
		const frames = calls.slice(beforePreview).map((call) => call.value);
		expect(frames.length).toBeGreaterThan(1);
		expect(frames.at(-1)).toBe(2);
		expect(
			frames.every(
				(value, i) => value >= (frames[i - 1] ?? 1) && value <= 2,
			),
		).toBe(true);
		expect(maxInFlight).toBe(1);
	} finally {
		if (windowDescriptor)
			Object.defineProperty(globalThis, "window", windowDescriptor);
		else delete globalThis.window;
		if (tauriDescriptor)
			Object.defineProperty(globalThis, "isTauri", tauriDescriptor);
		else delete globalThis.isTauri;
	}
});

test("density updates do not rewrite an unchanged root attribute", () => {
	const descriptor = Object.getOwnPropertyDescriptor(globalThis, "document");
	let attribute = null,
		writes = 0;
	Object.defineProperty(globalThis, "document", {
		configurable: true,
		value: {
			documentElement: {
				getAttribute: () => attribute,
				setAttribute: (_, value) => {
					attribute = value;
					writes++;
				},
				removeAttribute: () => {
					attribute = null;
					writes++;
				},
			},
		},
	});
	try {
		for (let i = 0; i < 100; i++) applyInterfaceDensity("theme");
		expect(writes).toBe(0);
		for (let i = 0; i < 100; i++) applyInterfaceDensity("compact");
		expect(writes).toBe(1);
		applyInterfaceDensity("comfortable");
		applyInterfaceDensity("theme");
		applyInterfaceDensity("theme");
		expect(writes).toBe(3);
		expect(attribute).toBeNull();
	} finally {
		if (descriptor)
			Object.defineProperty(globalThis, "document", descriptor);
		else delete globalThis.document;
	}
});

test("font override sets and clears the root --font-family", () => {
	const descriptor = Object.getOwnPropertyDescriptor(globalThis, "document");
	const properties = new Map();
	Object.defineProperty(globalThis, "document", {
		configurable: true,
		value: {
			documentElement: {
				style: {
					setProperty: (key, value) => properties.set(key, value),
					removeProperty: (key) => properties.delete(key),
				},
			},
		},
	});
	try {
		applyInterfaceFont("Cantarell");
		expect(properties.get("--font-family")).toBe(
			'"Cantarell", system-ui, sans-serif',
		);
		applyInterfaceFont("theme");
		expect(properties.has("--font-family")).toBe(false);
	} finally {
		if (descriptor)
			Object.defineProperty(globalThis, "document", descriptor);
		else delete globalThis.document;
	}
});
