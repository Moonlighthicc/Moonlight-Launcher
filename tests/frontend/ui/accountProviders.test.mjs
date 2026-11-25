import { expect, test } from "bun:test";
import {
	isElyByAccount,
	getAccountProvider,
	ACCOUNT_PROVIDERS,
} from "../../../src/lib/utils/accountProviders.ts";

test("account picker lists the four providers in display order", () => {
	expect(ACCOUNT_PROVIDERS).toEqual([
		"premium",
		"ely",
		"offline",
		"authinject",
	]);
});

test("saved accounts identify their provider without confusing custom servers", () => {
	for (const [user_type, server, provider] of [
		["Microsoft", null, "premium"],
		["Cracked", "https://auth.example.com", "offline"],
		["Yggdrasil", "https://account.ely.by/api/authlib-injector", "ely"],
		["Yggdrasil", "auth.example.com", "authinject"],
		["Yggdrasil", "https://AUTH.example.com/api/yggdrasil", "authinject"],
		["Yggdrasil", "invalid server", "authinject"],
		["Yggdrasil", null, "authinject"],
	]) {
		expect(
			getAccountProvider({ user_type, yggdrasil_server_url: server }),
		).toBe(provider);
	}
});

test("Ely.by skin panel is available only for official Ely.by accounts", () => {
	for (const server of [
		"ely.by",
		"https://ely.by/",
		"https://www.ely.by/",
		"https://authserver.ely.by/",
		"http://authserver.ely.by/",
		"https://AUTHSERVER.ELY.BY/",
		"https://account.ely.by/api/authlib-injector",
	]) {
		expect(
			isElyByAccount({
				user_type: "Yggdrasil",
				yggdrasil_server_url: server,
			}),
		).toBe(true);
	}
	for (const server of [
		null,
		"",
		"https://littlesk.in/",
		"https://authserver.ely.by.example.com/",
		"https://ely.by@example.com/",
		"https://example.com/ely.by",
		"https://authserver.ely.by:8080/",
		"ftp://ely.by/",
	]) {
		expect(
			isElyByAccount({
				user_type: "Yggdrasil",
				yggdrasil_server_url: server,
			}),
		).toBe(false);
	}
});

test("Microsoft and offline accounts do not use the Ely.by skin panel", () => {
	for (const user_type of ["Microsoft", "Cracked"]) {
		expect(
			isElyByAccount({
				user_type,
				yggdrasil_server_url: "https://ely.by",
			}),
		).toBe(false);
	}
});
