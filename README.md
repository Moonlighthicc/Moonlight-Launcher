# Moonlight Launcher

**A survival world, a server modpack, and a place to try new mods. Give each one its own setup.**

Moonlight Launcher is an open-source launcher for **Minecraft: Java Edition**. It calls each setup an *instance*. Instances keep their saves, mods, settings, and server lists separate, so you can experiment with one without changing the others.

[Start playing](#your-first-launch) · [Bring an existing setup](#bring-an-existing-setup) · [Build from source](#build-from-source)

## Your first launch

Run the Windows installer (`.exe`) if you have it, or [build it from this checkout](#build-from-source). Once Moonlight Launcher is open:

1. Sign in with Microsoft or a Yggdrasil provider, or create a local offline profile.
2. Choose **Create Instance**. Start with vanilla Minecraft, pick Fabric, Forge, NeoForge, Quilt, or OptiFine, or import a modpack.
3. Wait for the game files to finish downloading, then launch the instance.

Want to try a shader or swap out a mod? Browse Modrinth and CurseForge from inside the launcher. If a setup needs more memory or a different Java runtime, adjust its launch settings and check the game log if it doesn't start.

## Bring an existing setup

Use **Create Instance → Local → Migrate** to copy an installation from another launcher, such as Prism Launcher, MultiMC, or the official Minecraft Launcher. You can select the folder manually if you use a portable installation. The original stays untouched; Moonlight Launcher downloads the game versions and loaders separately, and you'll need to add your accounts again.

## Make the launcher yours

Import a `.moontheme` file to change the look of the launcher itself. The [Quiet Orbit example](examples/themes/quiet-orbit) is a starting point for your own palette, and the [theme guide](THEMES.md) explains how to package it.

<details>
<summary>Where does Moonlight Launcher keep my files?</summary>

By default, the launcher uses `~/.moonlauncher` (`%USERPROFILE%\.moonlauncher` on Windows). Your individual setups live in `instances/`; reusable downloads, including game versions and Java runtimes, live in `shared/`.

</details>

## Build from source

Moonlight Launcher uses **SvelteKit** for the interface and **Tauri v2** with **Rust** for the desktop backend. Install Bun, a stable Rust toolchain with Cargo, and the Tauri v2 system prerequisites for your platform. From the project root:

```sh
bun install --frozen-lockfile
bun run tauri dev
```

On Windows, `bun run tauri build` builds the NSIS installer. Before submitting changes, run `bun run check`, `bun run lint`, and `bun run test:all`. See [TESTING.md](TESTING.md) for individual test suites.

If you're reporting a bug, include the launcher version, your OS, and the steps that triggered it. For game launch failures, include the Minecraft version, the mod loader, and a relevant log excerpt with private information removed.

---

Based on [CubicLauncher](https://github.com/CubicLauncherDevs/CubicLauncher) · Licensed under [GPL-3.0-only](LICENSE)