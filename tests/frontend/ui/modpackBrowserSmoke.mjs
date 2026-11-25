// Run with Bun; the generated page is self-contained so it can be opened in Edge.
import { compile, compileModule } from "svelte/compiler";
import { dirname, resolve } from "node:path";
import { tmpdir } from "node:os";

const root = resolve(import.meta.dir, "../../..");
const components = "./src/lib/components/instances/CreateInstanceModal/";
const entry = `
import { mount, tick } from 'svelte';
import Details from '${components}ModpackDetails.svelte';
import { packVersionLabel } from '$lib/utils/modpackVersions';
const versions = Array.from({length: 95}, (_, i) => ({value: String(i), label: packVersionLabel('Pack ' + i, i === 0 ? '1.21.1' : '1.20.1'), date: new Date(Date.UTC(2026, 0, 95 - i)).toISOString(), channel: i === 1 ? 'beta' : 'release', packLoaders: [i === 0 ? 'neoforge' : 'fabric']}));
mount(Details, {target: document.querySelector('main'), props: {item: {id: 1, title: 'Example', description: '', downloads: 1}, versions, gallery: []}});
const frame = async () => { await tick(); await new Promise(resolve => setTimeout(resolve, 30)); };
const click = (button) => { if (!button) throw Error('Button missing'); button.click(); };
const button = (base, key) => [...base.querySelectorAll('button')].find(x => x.textContent.includes(key));
const assert = (value, message) => { if (!value) throw Error(message); };
const setQuery = async (input, query) => { input.value = query; input.dispatchEvent(new Event('input', {bubbles: true})); await frame(); };
try {
  await frame();
  assert(document.querySelector('.version-summary').textContent.includes('NeoForge'), 'Selected version must show its loader next to Install');
  const tabs = document.querySelector('.tabs');
  const run = async (open, close, panelSelector, selector, maxHeight, vh) => {
    open(); await frame();
    const scroller = document.querySelector(panelSelector);
    const input = scroller.previousElementSibling.querySelector('.version-search');
    const count = () => scroller.querySelectorAll(selector).length;
    const scroll = async () => {
      assert(scroller.scrollHeight > scroller.clientHeight, 'List must actually overflow');
      scroller.scrollTop = scroller.scrollHeight;
      scroller.dispatchEvent(new Event('scroll'));
      await frame(); await frame();
      const top = button(scroller, 'common.scrollToTop');
      assert(top && top.getBoundingClientRect().top < scroller.getBoundingClientRect().bottom && top.getBoundingClientRect().bottom > scroller.getBoundingClientRect().top, 'Back to top must be visible');
      click(top); await frame(); await frame();
      assert(scroller.scrollTop === 0, 'Back to top must scroll its own list to 0');
    };
    assert(count() === 30, 'Initial list must have 30 rows');
    assert(scroller.querySelector(selector).textContent.trim().startsWith('1.21.1 — Pack 0'), 'Minecraft version must precede the release name');
    const tags = [...scroller.querySelectorAll(selector + ' .tags')];
    assert(tags[0].lastElementChild.textContent.trim() === 'NeoForge' && tags[0].lastElementChild.previousElementSibling.textContent === 'createInstance.pack.release', 'NeoForge must follow the release badge');
    assert(tags[1].lastElementChild.textContent.trim() === 'Fabric' && tags[1].lastElementChild.previousElementSibling.textContent === 'createInstance.pack.beta', 'Fabric must follow the beta badge');
    assert(Math.abs(parseFloat(getComputedStyle(scroller).maxHeight) - Math.min(maxHeight, innerHeight * vh)) < 1, 'Version list height must match its mode');
    click(button(scroller, 'createInstance.loadMore')); await frame();
    assert(count() === 60, 'Load more must add 30');
    await setQuery(input, 'Pack 94');
    assert(count() === 1 && scroller.textContent.includes('Pack 94'), 'Search must include rows past the first two pages');
    await setQuery(input, '');
    assert(count() === 30, 'Clearing search must collapse back to 30');
    await setQuery(input, 'Pack');
    assert(count() === 95, 'Search must not be paginated');
    click(button(scroller, 'showAllVersions')); await frame();
    await setQuery(input, '');
    assert(count() === 95, 'Show all must survive clearing search');
    await scroll();
    close(); await frame(); open(); await frame();
    assert(document.querySelector(panelSelector).querySelectorAll(selector).length === 30, 'Reopening must reset the list');
    close(); await frame();
  };
  const tab = key => click(button(tabs, key));
  const togglePicker = () => click(button(document.querySelector('.version-summary'), 'changeVersion'));
  await run(togglePicker, togglePicker, '.picker .version-scroll', '.picker-item', 230, 0.35);
  await run(() => tab('pack.versions'), () => tab('pack.description'), '.full-version-scroll', '.file', 270, 0.40);
  document.querySelector('pre').textContent = 'PASS: both version lists';
} catch (err) { document.querySelector('pre').textContent = 'FAIL: ' + err.message; }
`;

const output = await Bun.build({
	entrypoints: ["modpack-smoke"],
	target: "browser",
	conditions: ["browser"],
	plugins: [{ name: "smoke", setup(build) {
		build.onResolve({ filter: /^modpack-smoke$/ }, () => ({ path: "entry", namespace: "smoke" }));
		build.onResolve({ filter: /^\$lib\// }, ({ path }) => {
			if (path.includes("ScrollToTopButton") || path.includes("packVersionPager") || path.includes("modpackVersions") || path.includes("modpackLoaders") || path.includes("instanceName")) {
				const base = resolve(root, "src/lib", path.slice(5));
				return { path: base.endsWith(".svelte") && !base.endsWith("ScrollToTopButton.svelte") ? `${base}.ts` : base.endsWith(".svelte") ? base : `${base}.ts` };
			}
			return { path, namespace: "stub" };
		});
		build.onResolve({ filter: /^\.\/ModpackProperties\.svelte$/ }, ({ path }) => ({ path, namespace: "stub" }));
		build.onLoad({ filter: /.*/, namespace: "smoke" }, () => ({ contents: entry, loader: "js", resolveDir: root }));
		build.onLoad({ filter: /.*/, namespace: "stub" }, ({ path }) => ({ loader: "js", contents:
			path.endsWith("/i18n") ? "export const t = key => key;" :
			path.endsWith("/icons/logos") ? "export const getLoaderLogo = () => ''; export const getLoaderColorVar = () => 'currentColor';" :
			path.endsWith("/animations") ? "export const animDuration = () => 0; export const shouldReduceAnimations = () => true;" :
			path.endsWith("/state.svelte") ? "export const launcherStore = {settings: {}, loadedInstances: []};" :
			path.endsWith("/cubicApi") ? "export const openUrl = () => {};" : "export default function() {}" }));
		build.onLoad({ filter: /\.svelte\.ts$/ }, async ({ path }) => ({ loader: "js", resolveDir: dirname(path), contents: compileModule(new Bun.Transpiler({ loader: "ts" }).transformSync(await Bun.file(path).text()), { filename: path }).js.code }));
		build.onLoad({ filter: /\.svelte$/ }, async ({ path }) => ({ loader: "js", resolveDir: dirname(path), contents: compile(await Bun.file(path).text(), { filename: path, css: "injected" }).js.code }));
	} }],
});
const js = await output.outputs[0].text();
const page = resolve(tmpdir(), "moon-modpack-smoke.html");
await Bun.write(page, `<!doctype html><meta charset="utf-8"><main></main><pre>NOT_RUN</pre><script type="module">${js.replaceAll("</script", "<\\/script")}</script>`);
console.log(page);