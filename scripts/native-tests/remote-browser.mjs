// Real production remote server, isolated headless Chrome, no desktop or audio.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';

const binary = resolve(process.argv[2] || 'target/debug/examples/remote-browser');
assert.equal(typeof WebSocket, 'function', 'This fixture requires Node 22 or newer');
const chrome = process.env.CHROME_BIN || (process.platform === 'darwin'
    ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
    : execFileSync('which', ['google-chrome'], { encoding: 'utf8' }).trim());
const root = await mkdtemp(join(tmpdir(), 'feishin-remote-browser-'));
const fixture = spawn(binary, [], { stdio: ['pipe', 'pipe', 'pipe'] });
const lines = [], readers = [];
createInterface({ input: fixture.stdout }).on('line', line => {
    const value = JSON.parse(line);
    if (readers.length) readers.shift()(value); else lines.push(value);
});
let fixtureErrors = '', browserErrors = '', browser, client;
fixture.stderr.on('data', data => { fixtureErrors += data; });
fixture.on('error', error => { fixtureErrors += error; });
function nextState() {
    if (lines.length) return Promise.resolve(lines.shift());
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('Server fixture timed out: ' + fixtureErrors)), 10_000);
        timer.unref(); readers.push(value => { clearTimeout(timer); resolve(value); });
    });
}
async function waitFor(check, description) {
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
        try { const result = await check(); if (result) return result; } catch { /* Retry navigation/startup. */ }
        await delay(50);
    }
    throw new Error('Timed out: ' + description);
}
class CDP {
    constructor(socket) {
        this.socket = socket; this.nextId = 0; this.pending = new Map(); this.operations = [];
        socket.addEventListener('message', event => {
            const response = JSON.parse(event.data);
            if (response.method === 'Network.webSocketFrameSent') {
                try {
                    const message = JSON.parse(response.params.response.payloadData);
                    if (message.event !== 'authenticate') this.operations.push(message);
                } catch { /* Non-JSON transport frames. */ }
            }
            const request = this.pending.get(response.id);
            if (!request) return;
            this.pending.delete(response.id); clearTimeout(request.timer);
            if (response.error) request.reject(new Error(JSON.stringify(response.error)));
            else request.resolve(response.result);
        });
    }
    call(method, params = {}) {
        const id = ++this.nextId;
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => { this.pending.delete(id); reject(new Error('CDP timeout: ' + method)); }, 20_000);
            timer.unref(); this.pending.set(id, { resolve, reject, timer });
            this.socket.send(JSON.stringify({ id, method, params }));
        });
    }
    async evaluate(expression) {
        const response = await this.call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
        if (response.exceptionDetails) throw new Error(JSON.stringify(response.exceptionDetails));
        return response.result.value;
    }
    close() {
        for (const request of this.pending.values()) { clearTimeout(request.timer); request.reject(new Error('Browser closed')); }
        this.pending.clear(); this.socket.close();
    }
}
async function terminate(process_) {
    if (!process_ || process_.exitCode !== null || process_.signalCode !== null) return;
    process_.kill('SIGTERM');
    await Promise.race([once(process_, 'exit'), delay(5000)]);
    if (process_.exitCode === null && process_.signalCode === null) {
        process_.kill('SIGKILL'); await once(process_, 'exit');
    }
}
try {
    const ready = await nextState(); assert.equal(ready.state, 'ready');
    const url = `http://127.0.0.1:${ready.port}/`;
    await waitFor(async () => (await fetch(url)).ok, 'production remote HTTP startup');
    browser = spawn(chrome, ['--headless', '--disable-gpu', '--no-first-run', '--no-default-browser-check',
        '--remote-debugging-port=0', '--user-data-dir=' + root, ...(process.env.FEISHIN_CHROME_NO_SANDBOX === '1' ? ['--no-sandbox'] : []), 'about:blank'],
        { stdio: ['ignore', 'ignore', 'pipe'] });
    browser.stderr.on('data', data => { browserErrors += data; });
    browser.on('error', error => { browserErrors += error; });
    const debuggerPort = await waitFor(async () => {
        const contents = await readFile(join(root, 'DevToolsActivePort'), 'utf8'); return contents.split('\n')[0];
    }, 'isolated browser startup');
    const tabs = await (await fetch(`http://127.0.0.1:${debuggerPort}/json/list`)).json();
    const socket = new WebSocket(tabs.find(tab => tab.type === 'page').webSocketDebuggerUrl);
    await once(socket, 'open'); client = new CDP(socket);
    await client.call('Page.enable'); await client.call('Network.enable');
    await client.call('Page.addScriptToEvaluateOnNewDocument', { source: 'const NativeWebSocket = WebSocket; globalThis.fixtureSockets = []; globalThis.WebSocket = class extends NativeWebSocket { constructor(...args) { super(...args); fixtureSockets.push(this); } };' });
    await client.call('Network.setCacheDisabled', { cacheDisabled: true });
    await client.call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
    await client.call('Page.navigate', { url });
    const pageReady = () => waitFor(() => client.evaluate('document.readyState === "complete" && !!document.getElementById("login") && typeof document.getElementById("login").onsubmit === "function"'), 'remote document and script');
    await pageReady();
    const theme = () => client.evaluate(`({mode:document.documentElement.dataset.theme,accent:getComputedStyle(document.documentElement).getPropertyValue('--accent').trim(),saved:localStorage.getItem('feishin-remote-theme')})`);
    assert.equal((await theme()).mode, 'light');
    await client.evaluate('(async()=>{await navigator.serviceWorker.ready; if(!navigator.serviceWorker.controller) await new Promise(resolve=>navigator.serviceWorker.addEventListener("controllerchange",resolve,{once:true}));return true})()');
    const manifest = await client.call('Page.getAppManifest');
    assert.deepEqual(manifest.errors, []);
    const definition = JSON.parse(manifest.data);
    assert.equal(definition.display, 'standalone');
    assert(definition.icons.some(icon => icon.sizes === '512x512'));
    const submit = password => client.evaluate(`(()=>{const form=document.getElementById('login'); form.elements.username.value='phone'; form.elements.password.value=${JSON.stringify(password)}; form.requestSubmit();return true})()`);
    await submit('incorrect-fixture-password');
    await waitFor(() => client.evaluate('document.getElementById("error").textContent.includes("Authentication failed")'), 'authentication rejection');
    await submit('fixture-password');
    await waitFor(() => client.evaluate('!document.getElementById("app").hidden && document.getElementById("volume").value === "50"'), 'authenticated production snapshot');
    assert(await client.evaluate('document.getElementById("previous").disabled && document.getElementById("next").disabled && document.getElementById("play").disabled'), 'An empty player disables track transport');
    await waitFor(async () => (await theme()).accent === '#3574fc', 'original light-mode accent');
    await client.evaluate('document.getElementById("theme").click()');
    assert.deepEqual(await theme(), { mode: 'dark', accent: '#0558fd', saved: 'dark' });
    fixture.stdin.write('accent\n'); assert.equal((await nextState()).state, 'updated');
    await waitFor(async () => (await theme()).accent === '#ff6464', 'live custom accent and shade');
    const cached = await client.evaluate('(async()=>{const cache=await caches.open("feishin-native-remote-v1"); return (await cache.keys()).map(request=>new URL(request.url).pathname).sort()})()');
    assert.deepEqual(cached, ['/', '/icon-256.png', '/icon-512.png', '/manifest.json', '/remote.js']);
    await client.evaluate('(async()=>{await fetch("/artwork/private-fixture/no-file"); await fetch("/?credential=fixture-private");return true})()');
    assert(await client.evaluate('(async()=>{const cache=await caches.open("feishin-native-remote-v1"); const requests=await cache.keys(); const html=await (await cache.match("/")).text(); return requests.every(request=>!new URL(request.url).search && !request.url.includes("artwork")) && !html.includes("fixture-password")})()'));
    fixture.stdin.write('stop\n'); assert.equal((await nextState()).state, 'stopped');
    await waitFor(() => client.evaluate('document.getElementById("app").hidden && !document.getElementById("login").hidden'), 'server shutdown closes authenticated browser state');
    await client.call('Network.emulateNetworkConditions', { offline: true, latency: 0, downloadThroughput: 0, uploadThroughput: 0 });
    await client.call('Page.reload', { ignoreCache: true }); await pageReady();
    assert.equal((await theme()).mode, 'dark', 'Saved preference overrides the light system theme offline');
    assert(await client.evaluate('!!navigator.serviceWorker.controller && !document.getElementById("login").hidden && document.getElementById("login").elements.password.value === ""'));
    fixture.stdin.write('restart\n'); const restarted = await nextState();
    assert.equal(restarted.state, 'ready'); assert.equal(restarted.port, ready.port);
    await client.call('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
    await client.call('Page.reload', { ignoreCache: true }); await pageReady();
    await submit('fixture-password');
    await waitFor(() => client.evaluate('!document.getElementById("app").hidden && document.getElementById("volume").value === "73"'), 'reconnect receives restarted server snapshot');
    await waitFor(async () => (await theme()).accent === '#0558fd', 'reconnected theme accent overrides the prior custom color');
    await client.evaluate('document.getElementById("theme").click()');
    assert.deepEqual(await theme(), { mode: 'light', accent: '#3996fe', saved: 'light' });
    await client.evaluate('localStorage.removeItem("feishin-remote-theme")');
    await client.call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] });
    await client.call('Page.reload', { ignoreCache: true }); await pageReady();
    assert.equal((await theme()).mode, 'dark', 'An unsaved theme follows a dark system preference');
    await submit('fixture-password');
    await waitFor(async () => (await theme()).accent === '#0558fd', 'dark system theme receives its original accent');
    fixture.stdin.write('credentials\n');
    assert.equal((await nextState()).port, ready.port);
    await waitFor(() => client.evaluate('document.getElementById("app").hidden && !document.getElementById("login").hidden'), 'changed credentials disconnect the browser');
    await submit('fixture-password');
    await waitFor(() => client.evaluate('document.getElementById("error").textContent.includes("Authentication failed")'), 'old credentials stop working without a listener restart');
    await submit('changed-password');
    await waitFor(() => client.evaluate('!document.getElementById("app").hidden'), 'new credentials work on the same port');
    fixture.stdin.write('unprotected\n');
    assert.equal((await nextState()).port, ready.port);
    await waitFor(() => client.evaluate('document.getElementById("app").hidden'), 'disabling authentication closes prior sessions');
    await client.evaluate(`(()=>{const form=document.getElementById('login'); form.elements.username.value=''; form.elements.password.value=''; form.requestSubmit();return true})()`);
    await waitFor(() => client.evaluate('!document.getElementById("app").hidden && document.getElementById("volume").value === "73"'), 'empty form can connect to an explicitly unprotected server');
    fixture.stdin.write('protected\n');
    assert.equal((await nextState()).port, ready.port);
    await waitFor(() => client.evaluate('document.getElementById("app").hidden'), 'enabling authentication disconnects unprotected sessions');
    await submit('fixture-password');
    await waitFor(() => client.evaluate('!document.getElementById("app").hidden'), 'authentication is restored without changing the port');
    const count = await client.evaluate('fixtureSockets.length');
    await client.evaluate('fixtureSockets.at(-1).close(1000)');
    await waitFor(() => client.evaluate('document.getElementById("app").hidden && document.getElementById("error").textContent.includes("Reconnecting")'), 'transient connection loss schedules a retry');
    await waitFor(() => client.evaluate(`fixtureSockets.length === ${count + 1} && !document.getElementById("app").hidden && document.getElementById("volume").value === "73"`), 'automatic reconnect authenticates and restores the production snapshot');
    fixture.stdin.write('large-queue\n'); assert.equal((await nextState()).state, 'updated');
    const queueWindow = () => client.evaluate(`(()=>{const list=document.getElementById('list'),rows=[...list.querySelectorAll('[role=listitem]')];return {virtual:list.dataset.virtual,count:rows.length,first:rows[0]?.dataset.index,last:rows.at(-1)?.dataset.index,total:rows[0]?.getAttribute('aria-setsize'),height:list.scrollHeight,overflow:list.scrollWidth>list.clientWidth}})()`);
    await waitFor(async () => (await queueWindow()).total === '38000', 'production 38,000-entry duplicate queue');
    let visible = await queueWindow();
    assert.equal(visible.virtual, 'true'); assert(visible.count <= 20); assert.equal(visible.first, '0');
    assert.equal(visible.height, 38000 * 76); assert.equal(visible.overflow, false);
    await client.evaluate('document.getElementById("list").scrollTop = 20000 * 76');
    await waitFor(async () => (await queueWindow()).first === '19996', 'scroll to a middle queue window');
    assert(await client.evaluate(`(()=>{const row=document.querySelector('#list [data-index="20000"]');return row.getAttribute('aria-posinset')==='20001' && row.querySelector('strong').textContent==='Queue track 20000' && !!row.dataset.uniqueId})()`));
    await client.evaluate('document.getElementById("list").scrollTop = document.getElementById("list").scrollHeight');
    await waitFor(async () => (await queueWindow()).last === '37999', 'scroll to the final queue entry');
    assert((await queueWindow()).count <= 20);
    await client.call('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
    await client.call('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 1 });
    await client.evaluate(`(()=>{const list=document.getElementById('list');list.scrollTop=20000*76;window.scrollBy(0,list.getBoundingClientRect().top-400)})()`);
    await waitFor(async () => (await queueWindow()).first === '19996', 'touch queue window');
    const rowPoint = (index, handle = false) => client.evaluate(`(()=>{const row=document.querySelector('#list [data-index="${index}"]'),target=row.querySelector(${JSON.stringify(handle ? '.drag-handle' : '.info')}),bounds=target.getBoundingClientRect();return {x:bounds.left+Math.min(20,bounds.width/2),y:bounds.top+bounds.height/2,key:row.dataset.uniqueId,name:row.querySelector('strong').textContent}})()`);
    const touch = (type, point) => client.call('Input.dispatchTouchEvent', { type, touchPoints: point ? [{ x: point.x, y: point.y, id: 1 }] : [] });
    const held = await rowPoint(20000);
    await touch('touchStart', held); await delay(400);
    await waitFor(() => client.evaluate('document.getElementById("actions").open'), 'touch hold opens actions');
    assert.equal(await client.evaluate('document.getElementById("action-title").textContent'), held.name);
    await touch('touchEnd');
    assert(!client.operations.some(operation => operation.event === 'queue-jump'), 'A held row must not also play');
    await client.evaluate('document.querySelector("[data-close=actions]").click()');
    const swiped = await rowPoint(20000);
    // Start farther into the title so the 85px horizontal gesture stays onscreen.
    swiped.x += 90;
    await touch('touchStart', swiped); await touch('touchMove', { ...swiped, x: swiped.x - 85 }); await touch('touchEnd');
    await waitFor(() => client.operations.some(operation => operation.event === 'remove-from-queue' && operation.uniqueId === swiped.key), 'touch swipe sends the duplicate entry identity');
    await waitFor(async () => (await queueWindow()).total === '37999', 'server applies the swipe removal');
    const moved = await rowPoint(20001, true), target = await rowPoint(20003);
    await touch('touchStart', moved); await touch('touchMove', { ...target, y: target.y - 15 }); await touch('touchEnd');
    await waitFor(() => client.operations.some(operation => operation.event === 'reorder-queue' && operation.uniqueId === moved.key && operation.targetUniqueId === target.key && operation.edge === 'top'), 'touch handle sends target-relative ordering');
    await waitFor(() => client.evaluate(`document.querySelector('#list [data-index="20002"]')?.dataset.uniqueId === ${JSON.stringify(moved.key)}`), 'server places a forward drag before its target');
    const scrollBefore = await client.evaluate('document.getElementById("list").scrollTop');
    const reorderCount = client.operations.filter(operation => operation.event === 'reorder-queue').length;
    await touch('touchStart', await rowPoint(20002, true)); await touch('touchMove', { x: moved.x, y: 830 });
    await waitFor(() => client.evaluate(`document.getElementById('list').scrollTop > ${scrollBefore}`), 'touch drag scrolls beyond the visible rows');
    await touch('touchCancel');
    const stoppedScroll = await client.evaluate('document.getElementById("list").scrollTop');
    await delay(100);
    assert.equal(await client.evaluate('document.getElementById("list").scrollTop'), stoppedScroll, 'Cancelled drag stops automatic scrolling');
    assert.equal(client.operations.filter(operation => operation.event === 'reorder-queue').length, reorderCount, 'Cancelled drag must not mutate the queue');
    assert(await client.evaluate('!document.querySelector("#list [data-drop]")'), 'Cancelled drag clears its indicator');
    await client.evaluate('document.getElementById("list").scrollTop=20000*76');
    await waitFor(async () => (await queueWindow()).first === '19996', 'queue returns after cancelled drag');
    const tapped = await rowPoint(20002);
    await touch('touchStart', tapped); await touch('touchEnd');
    await waitFor(() => client.evaluate(`document.querySelector('#list [data-index="20002"]')?.classList.contains('current') && document.getElementById('title').textContent === ${JSON.stringify(tapped.name)}`), 'a normal touch plays the chosen duplicate entry');
    await client.evaluate('document.querySelector("[data-tab=tracks]").click()');
    await waitFor(async () => (await queueWindow()).total === '50', 'initial library page');
    await client.evaluate('document.getElementById("list").scrollTop=document.getElementById("list").scrollHeight');
    await waitFor(async () => (await queueWindow()).total === '100', 'scrolling automatically fetches the next library page');
    assert.equal(client.operations.filter(operation => operation.event === 'tracks-request' && operation.startIndex === 50).length, 1, 'Only one request for the next page');
    assert(await client.evaluate('document.getElementById("list").scrollTop > 0'), 'Appending a page keeps its scroll position');
    await client.evaluate('document.getElementById("search").elements.query.focus()');
    await client.call('Input.insertText', { text: 'tracks 319' });
    await waitFor(() => client.evaluate('document.querySelector("#list strong")?.textContent === "Library tracks 319"'), 'typing triggers the debounced library search');
    assert.equal(client.operations.filter(operation => operation.event === 'tracks-request' && operation.searchTerm === 'tracks 319').length, 1);
    assert.equal(await client.evaluate('document.getElementById("list").scrollTop'), 0, 'A new query resets scrolling');
    await client.evaluate('document.getElementById("search").elements.query.select()');
    await client.call('Input.insertText', { text: 'missing-fixture-item' });
    await waitFor(() => client.evaluate('document.getElementById("list").textContent === "No tracks found"'), 'empty search results are explicit');
    await client.evaluate('document.querySelector("[data-tab=queue]").click()');
    await waitFor(async () => (await queueWindow()).total === '37999', 'the queue remains intact after library searches');
    for (const [width, height] of [[1280,720], [768,1024], [390,844]]) {
        await client.call('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 500 });
        assert(await client.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Remote layout overflows at ' + width);
        assert.equal((await queueWindow()).overflow, false, 'Queue row overflows at ' + width);
    }
    const screenshot = await client.call('Page.captureScreenshot', { format: 'png' });
    const output = resolve(process.env.FEISHIN_REMOTE_SCREENSHOT || 'target/native-remote-browser.png');
    await mkdir(dirname(output), { recursive: true }); await writeFile(output, Buffer.from(screenshot.data, 'base64'));
    console.log('Remote browser: manifest, authentication and live credential rotation, optional authentication, system/saved themes, original mode-specific accents, live updates, public-only cache, offline reload, server shutdown/restart/reconnect, virtual 38,000-entry duplicate queue, touch hold/swipe/reorder/automatic scrolling/cancellation/tap, automatic paging/debounced search/empty results and three viewport layouts passed; screenshot ' + output);
} catch (error) {
    console.error(browserErrors.slice(-3000)); throw error;
} finally {
    if (client) { await client.call('Browser.close').catch(() => {}); client.close(); }
    await terminate(browser); fixture.stdin.end('quit\n'); await terminate(fixture);
    await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}
