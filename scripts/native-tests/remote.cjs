const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const crypto = require('node:crypto').webcrypto;
const code = fs.readFileSync('crates/feishin-core/src/remote.js', 'utf8');

test('remote worker caches only the public shell, updates online, and falls back offline', async () => {
    const handlers = {}, stored = new Map(), removed = [], waits = [];
    let online = true, skipped = 0, claimed = 0, version = 'fresh';
    const response = value => ({ ok: true, type: 'basic', value, clone() { return response(value); } });
    const cache = { async addAll(paths) { for (const path of paths) stored.set(`https://phone.example${path}`, response('installed')); }, async put(request, value) { stored.set(request.url, value); } };
    const context = vm.createContext({ URL, self: { location: { origin: 'https://phone.example' }, clients: { async claim() { claimed++; } }, async skipWaiting() { skipped++; }, addEventListener: (event, handler) => handlers[event] = handler },
        caches: { async open() { return cache; }, async keys() { return ['feishin-native-remote-old', 'feishin-native-remote-v1', 'unrelated']; }, async delete(key) { removed.push(key); }, async match(request) { return stored.get(request.url); } },
        async fetch() { if (!online) throw new Error('Offline'); return response(version); } });
    vm.runInContext(fs.readFileSync('crates/feishin-core/src/remote-worker.js', 'utf8'), context);
    const event = url => ({ request: { method: 'GET', url }, waitUntil(promise) { waits.push(promise); }, respondWith(promise) { this.result = promise; } });
    const install = event(''); handlers.install(install); await Promise.all(waits.splice(0)); assert.equal(skipped, 1);
    handlers.activate(event('')); await Promise.all(waits.splice(0)); assert.equal(claimed, 1); assert.deepEqual(removed, ['feishin-native-remote-old']);
    const script = event('https://phone.example/remote.js'); handlers.fetch(script); assert.equal((await script.result).value, 'fresh'); await Promise.all(waits.splice(0));
    version = 'newer'; handlers.fetch(script); assert.equal((await script.result).value, 'newer'); await Promise.all(waits.splice(0));
    online = false; handlers.fetch(script); assert.equal((await script.result).value, 'newer');
    for (const url of ['https://phone.example/artwork/token/private', 'https://phone.example/ws', 'https://phone.example/remote.js?password=secret', 'https://other.example/remote.js']) {
        const privateRequest = event(url); handlers.fetch(privateRequest); assert.equal(privateRequest.result, undefined);
    }
    const post = event('https://phone.example/'); post.request.method = 'POST'; handlers.fetch(post); assert.equal(post.result, undefined);
});

test('remote requests correlate overlapping replies, errors, disconnects, and timeouts', async () => {
    const sent = [], timers = new Map();
    let nextTimer = 0;
    const context = vm.createContext({ module: { exports: {} }, crypto,
        setTimeout: fn => { timers.set(++nextTimer, fn); return nextTimer; }, clearTimeout: id => timers.delete(id) });
    vm.runInContext(code, context);
    const requests = new context.module.exports.RemoteRequests(message => { sent.push(message); return true; });
    const first = requests.request({ event: 'tracks-request' }, 'tracks-response');
    const second = requests.request({ event: 'play-track' });
    assert.notEqual(sent[0].requestId, sent[1].requestId);
    assert.equal(requests.receive('operation-ack', { requestId: sent[0].requestId }), false);
    requests.receive('operation-ack', { requestId: sent[1].requestId });
    await second;
    requests.receive('tracks-response', { requestId: sent[0].requestId, items: ['song'] });
    assert.equal((await first).items[0], 'song');
    const failed = requests.request({ event: 'clear-queue' });
    requests.receive('operation-ack', { requestId: sent[2].requestId, error: 'Server refused' });
    await assert.rejects(failed, /Server refused/);
    const timeout = requests.request({ event: 'play-album' });
    [...timers.values()][0]();
    await assert.rejects(timeout, /timed out/);
    assert.equal(requests.receive('operation-ack', { requestId: sent[3].requestId }), false);
    const disconnected = requests.request({ event: 'play-playlist' });
    requests.close();
    await assert.rejects(disconnected, /Connection closed/);
    const unavailable = new context.module.exports.RemoteRequests(() => false);
    await assert.rejects(unavailable.request({ event: 'next' }), /Connection closed/);
});

class Element {
    constructor(tag = 'div') { this.tagName = tag; this.children = []; this.dataset = {}; this.attributes = {}; this.hidden = false; this.textContent = ''; this.value = ''; this.scrollTop = 0; this.clientHeight = 600; this.style = { values: {}, setProperty(key, value) { this.values[key] = value; }, removeProperty(key) { delete this.values[key]; } }; }
    append(...values) { for (const value of values) { value.parentElement = this; this.children.push(value); } }
    replaceChildren(...values) { this.children = values; }
    setAttribute(key, value) { this.attributes[key] = value; }
    getAttribute(key) { return this.attributes[key]; }
    closest(selector) { if (selector === this.tagName || (selector === '.row' && this.className?.split(' ').includes('row'))) return this; return this.parentElement?.closest(selector); }
    getBoundingClientRect() { const top = Number(this.dataset.index || 0) * 76 - (this.parentElement?.scrollTop || 0); return { top, bottom: top + (this.attributes.role === 'listitem' ? 76 : 600), height: this.attributes.role === 'listitem' ? 76 : 600 }; }
    setPointerCapture(pointer) { this.pointer = pointer; }
    hasPointerCapture(pointer) { return this.pointer === pointer; }
    releasePointerCapture() { this.pointer = undefined; }
    showModal() { this.open = true; }
    close() { this.open = false; this.onclose?.(); }
}
const flush = async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); };

function remoteBrowser({ systemDark = false, savedTheme = null, deniedStorage = false, timers = {} } = {}) {
    const html = fs.readFileSync('crates/feishin-core/src/remote.html', 'utf8');
    const elements = new Map([...html.matchAll(/id="([^"]+)"/g)].map(match => [match[1], new Element()]));
    const selectors = {};
    for (const attribute of ['event', 'tab', 'close']) selectors[`[data-${attribute}]`] = [...html.matchAll(new RegExp(`data-${attribute}="([^"]+)"`, 'g'))].map(match => {
        const element = new Element('button'); element.dataset[attribute] = match[1]; return element;
    });
    const document = { activeElement: null, documentElement: new Element(), scrollingElement: new Element(), getElementById: id => {
        assert.ok(elements.has(id), `Missing HTML element ${id}`); return elements.get(id);
    }, createElement: tag => new Element(tag), querySelectorAll: selector => selectors[selector] };
    class Socket {
        static OPEN = 1;
        static instances = [];
        constructor() { this.readyState = 1; this.sent = []; Socket.instances.push(this); }
        send(value) { this.sent.push(JSON.parse(value)); }
        close(code = 4001) { this.readyState = 3; this.onclose?.({ code }); }
        receive(event, data) { this.onmessage({ data: JSON.stringify({ event, data }) }); }
    }
    const confirmations = [];
    const context = vm.createContext({ document, module: { exports: {} }, crypto, setTimeout, clearTimeout, requestAnimationFrame: fn => setTimeout(fn, 0), cancelAnimationFrame: clearTimeout, WebSocket: Socket,
        location: { protocol: 'http:', host: '127.0.0.1:4333' }, btoa: value => Buffer.from(value, 'binary').toString('base64'),
        unescape, encodeURIComponent, FormData: class { constructor(form) { this.form = form; } get(key) { return this.form[key]; } },
        confirm: value => { confirmations.push(value); return true; }, URL, localStorage: { getItem() { if (deniedStorage) throw new Error('Storage denied'); return savedTheme; }, setItem(key, value) { if (deniedStorage) throw new Error('Storage denied'); savedTheme = value; } }, window: { innerHeight: 600, open() {}, matchMedia: () => ({ matches: systemDark }) }, CSS: { supports: (property, value) => property === 'color' && /^#[a-f0-9]{6}$/i.test(value) }, ...timers });
    vm.runInContext(code, context);
    return { elements, selectors, document, Socket, confirmations, savedTheme: () => savedTheme };
}

test('remote reconnect matches original close policies and cancels superseded attempts', () => {
    const reference = JSON.parse(fs.readFileSync('crates/feishin-core/assets/remote-settings-reference.json', 'utf8'));
    for (const expected of reference.closes) {
        const pending = new Map(); let id = 0;
        const { elements, Socket } = remoteBrowser({ timers: {
            setTimeout: (fn, delay) => { pending.set(++id, { fn, delay }); return id; },
            clearTimeout: id => pending.delete(id),
        } });
        const submit = password => elements.get('login').onsubmit({ preventDefault() {}, target: { username: 'phone', password } });
        submit('old'); const original = Socket.instances[0]; original.onopen();
        if (expected.natural) submit('new');
        original.onclose({ code: expected.code });
        assert.deepEqual([...pending.values()].map(timer => timer.delay), expected.delays, JSON.stringify(expected));
        if (expected.delays.length) {
            const retry = [...pending.values()][0].fn;
            pending.clear(); retry();
            assert.equal(Socket.instances.length, 2);
            Socket.instances[1].onopen();
            assert.deepEqual(Socket.instances[1].sent[0], original.sent[0]);
            original.onclose({ code: 1006 });
            assert.equal(pending.size, 0, 'An obsolete socket cannot schedule another retry');
        }
        Socket.instances.at(-1).close();
    }
    const pending = new Map(); let id = 0;
    const { elements, Socket } = remoteBrowser({ timers: {
        setTimeout: (fn, delay) => { pending.set(++id, { fn, delay }); return id; },
        clearTimeout: id => pending.delete(id),
    } });
    const submit = password => elements.get('login').onsubmit({ preventDefault() {}, target: { username: 'phone', password } });
    submit('old'); Socket.instances[0].onclose({ code: 1006 });
    const stale = [...pending.values()][0].fn;
    submit('new'); assert.equal(pending.size, 0);
    stale(); assert.equal(Socket.instances.length, 2);
    Socket.instances[1].onopen();
    assert.equal(Socket.instances[1].sent[0].header, 'Basic ' + Buffer.from('phone:new').toString('base64'));
    Socket.instances[1].close();
});

test('remote theme follows the system, retains valid choices, and applies live mode-specific accents', () => {
    for (const options of [
        { systemDark: true, expected: 'dark' },
        { systemDark: false, expected: 'light' },
        { systemDark: true, savedTheme: 'light', expected: 'light' },
        { systemDark: false, savedTheme: 'dark', expected: 'dark' },
        { systemDark: true, savedTheme: 'invalid', expected: 'dark' },
        { systemDark: false, deniedStorage: true, expected: 'light' },
    ]) {
        const { elements, document, Socket, savedTheme } = remoteBrowser(options);
        const root = document.documentElement;
        assert.equal(root.dataset.theme, options.expected);
        elements.get('login').onsubmit({ preventDefault() {}, target: { username: 'phone', password: 'secret' } });
        const socket = Socket.instances[0]; socket.onopen();
        const colors = { dark: '#0558fd', light: '#3574fc' };
        socket.receive('accent-color', colors);
        assert.equal(root.style.values['--accent'], colors[options.expected]);
        elements.get('theme').onclick();
        const next = options.expected === 'dark' ? 'light' : 'dark';
        assert.equal(root.dataset.theme, next);
        assert.equal(root.style.values['--accent'], colors[next]);
        if (!options.deniedStorage) assert.equal(savedTheme(), next);
        const updated = { dark: '#0049e2', light: '#0049e2' };
        socket.receive('accent-color', updated);
        assert.equal(root.style.values['--accent'], updated[next]);
        socket.receive('accent-color', { dark: 'url(private)', light: 42 });
        assert.equal(root.style.values['--accent'], undefined);
        socket.receive('accent-color', null);
        assert.equal(root.style.values['--accent'], undefined);
        socket.close();
    }
});

test('remote browser flows keep stale searches out, use server identities, and acknowledge queue actions', async () => {
    const { elements, selectors, Socket, confirmations } = remoteBrowser();
    elements.get('login').onsubmit({ preventDefault() {}, target: { username: 'phone', password: 'secret' } });
    const socket = Socket.instances[0]; socket.onopen();
    assert.equal(socket.sent[0].event, 'authenticate');
    socket.receive('state', { song: { id: 'old', _serverId: 'server-a', name: 'Old', duration: 60 }, volume: 35, position: 12, status: 'playing', repeat: 'one', shuffle: true });
    socket.receive('queue-state', { currentUniqueId: 'first', items: [
        { id: 'duplicate', name: 'Duplicate', uniqueId: 'first', _serverId: 'server-a' },
        { id: 'duplicate', name: 'Duplicate', uniqueId: 'second', _serverId: 'server-a' },
    ] });
    assert.equal(elements.get('play').hidden, true);
    assert.equal(elements.get('volume').value, 35);
    assert.equal(elements.get('repeat').textContent, 'Repeat: one');
    selectors['[data-tab]'].find(button => button.dataset.tab === 'albums').onclick();
    const albumRequest = socket.sent.at(-1);
    selectors['[data-tab]'].find(button => button.dataset.tab === 'tracks').onclick();
    const trackRequest = socket.sent.at(-1);
    socket.receive('albums-response', { requestId: albumRequest.requestId, items: [{ id: 'album', name: 'Stale album' }], hasMore: false });
    socket.receive('tracks-response', { requestId: trackRequest.requestId, items: [{ id: 'song', name: 'Song', _serverId: 'server-b', imageUrl: '/artwork/cap/key' }], hasMore: false });
    await flush();
    assert.equal(elements.get('list').children.length, 1);
    const row = elements.get('list').children[0];
    assert.equal(row.children.find(child => child.className === 'info').children[0].textContent, 'Song');
    assert.equal(row.children[0].src, '/artwork/cap/key');
    await row.children.find(child => child.textContent === '⋯').onclick();
    const shuffled = elements.get('action-buttons').children.find(button => button.textContent === 'Next shuffled');
    const operation = shuffled.onclick();
    const playback = socket.sent.at(-1);
    assert.equal(playback.event, 'play-track');
    assert.equal(playback.playType, 'nextShuffle');
    assert.equal(playback.serverId, 'server-b');
    assert.equal(shuffled.disabled, true);
    socket.receive('operation-ack', { requestId: playback.requestId }); await operation;
    assert.equal(elements.get('actions').open, false);
    await row.children.find(child => child.textContent === '⋯').onclick();
    const picker = elements.get('action-buttons').children.find(button => button.textContent === 'Add to playlist').onclick();
    const playlists = socket.sent.at(-1);
    assert.equal(playlists.serverId, 'server-b');
    socket.receive('playlists-response', { requestId: playlists.requestId, items: [{ id: 'playlist', name: 'Favorites' }], hasMore: false }); await picker;
    const addition = elements.get('playlist-choices').children[0].onclick();
    const add = socket.sent.at(-1);
    assert.equal(add.event, 'add-to-playlist'); assert.equal(add.songId, 'song'); assert.equal(add.playlistId, 'playlist');
    socket.receive('operation-ack', { requestId: add.requestId }); await addition;
    selectors['[data-tab]'].find(button => button.dataset.tab === 'queue').onclick();
    const second = elements.get('list').children[1];
    const reorder = second.children.find(child => child.className === 'moves').children[0].onclick();
    const move = socket.sent.at(-1);
    assert.equal(move.uniqueId, 'second'); assert.equal(move.targetUniqueId, 'first'); assert.equal(move.edge, 'top');
    socket.receive('operation-ack', { requestId: move.requestId }); await reorder;
    const clear = elements.get('clear').onclick();
    assert.match(confirmations.at(-1), /Clear/);
    socket.receive('operation-ack', { requestId: socket.sent.at(-1).requestId }); await clear;
    socket.receive('radio-status', { isActive: true, stationName: 'Station' });
    socket.receive('state', { song: { id: 'station', name: 'Live title', duration: 0 }, volume: 35, position: 12, status: 'playing', repeat: 'one', shuffle: true });
    assert.equal(elements.get('seek').disabled, true);
    assert.equal(elements.get('next').disabled, true);
    assert.equal(elements.get('repeat').disabled, true);
    assert.equal(elements.get('stop-radio').hidden, false);
    assert.match(elements.get('time').textContent, /LIVE/);
    selectors['[data-event]'].find(button => button.dataset.event === 'stop-radio').onclick();
    assert.equal(socket.sent.at(-1).event, 'stop-radio');
    socket.receive('radio-status', { isActive: false });
    assert.equal(elements.get('next').disabled, false);
    assert.equal(elements.get('stop-radio').hidden, true);
    socket.close();
    assert.equal(elements.get('app').hidden, true);
});


test('remote virtual queue bounds rows and preserves absolute duplicate identities while scrolling', async () => {
    const { elements, Socket } = remoteBrowser();
    elements.get('login').onsubmit({ preventDefault() {}, target: { username: 'phone', password: 'secret' } });
    const socket = Socket.instances[0]; socket.onopen();
    const items = Array.from({ length: 38000 }, (_, index) => ({ id: 'duplicate', uniqueId: `entry-${index}`, name: `Song ${index}` }));
    socket.receive('queue-state', { currentUniqueId: 'entry-20000', items });
    const list = elements.get('list');
    const rows = () => list.children.filter(child => child.attributes.role === 'listitem');
    assert.equal(list.dataset.virtual, 'true');
    assert(rows().length <= 16);
    assert.equal(rows()[0].dataset.uniqueId, 'entry-0');
    assert.equal(rows()[0].attributes['aria-setsize'], '38000');
    list.scrollTop = 20000 * 76; list.onscroll();
    assert.equal(rows()[0].dataset.index, '19996');
    const current = rows().find(row => row.className.includes('current'));
    assert.equal(current.dataset.uniqueId, 'entry-20000');
    assert.equal(current.attributes['aria-posinset'], '20001');
    const move = current.children.find(child => child.className === 'moves').children[0].onclick();
    const operation = socket.sent.at(-1);
    assert.equal(operation.uniqueId, 'entry-20000');
    assert.equal(operation.targetUniqueId, 'entry-19999');
    socket.receive('operation-ack', { requestId: operation.requestId }); await move;
    list.scrollTop = 38000 * 76; list.onscroll();
    assert.equal(rows().at(-1).dataset.uniqueId, 'entry-37999');
    assert(rows().length <= 16);
    socket.receive('queue-state', { currentUniqueId: 'entry-0', items: items.slice(0, 300) });
    assert.equal(rows().at(-1).dataset.uniqueId, 'entry-299');
    socket.receive('queue-state', { currentUniqueId: 'entry-0', items: items.slice(0, 2) });
    assert.equal(list.dataset.virtual, 'false');
    assert.equal(rows().length, 2);
    socket.close();
});


test('remote pointer gestures preserve duplicate identities, reject cancellations, and survive live queue pushes', async () => {
    let timerId = 0;
    const pending = new Map();
    const schedule = (fn, delay) => { pending.set(++timerId, { fn, delay }); return timerId; };
    const cancel = id => pending.delete(id);
    const fire = delay => { for (const [id, timer] of [...pending]) if (timer.delay === delay) { pending.delete(id); timer.fn(); } };
    const { elements, selectors, Socket } = remoteBrowser({ timers: {
        setTimeout: schedule, clearTimeout: cancel,
        requestAnimationFrame: fn => schedule(fn, 16), cancelAnimationFrame: cancel,
    } });
    elements.get('login').onsubmit({ preventDefault() {}, target: { username: 'phone', password: 'secret' } });
    const socket = Socket.instances[0]; socket.onopen();
    const items = [{ id: 'duplicate', uniqueId: 'first', name: 'First' }, { id: 'duplicate', uniqueId: 'second', name: 'Second' }];
    const publish = values => socket.receive('queue-state', { currentUniqueId: 'first', items: values });
    publish(items);
    const list = elements.get('list');
    const rows = () => list.children.filter(row => row.attributes.role === 'listitem');
    const pointer = (row, x = 200, y = 30) => ({ target: row, button: 0, pointerId: 7, clientX: x, clientY: y, preventDefault() {}, stopPropagation() {} });
    const acknowledge = () => socket.receive('operation-ack', { requestId: socket.sent.at(-1).requestId });
    let row = rows()[0];
    row.onpointerdown(pointer(row)); row.onpointerup(pointer(row)); row.onclick(pointer(row));
    assert.equal(socket.sent.at(-1).event, 'queue-jump'); assert.equal(socket.sent.at(-1).uniqueId, 'first');
    acknowledge(); await flush(); fire(16);
    const sent = socket.sent.length;
    row = rows()[0]; row.onpointerdown(pointer(row));
    publish(items.map(item => ({ ...item, name: 'Updated ' + item.name })));
    assert.equal(rows()[0], row, 'A queue push cannot replace a held row');
    fire(350); assert.equal(elements.get('actions').open, true);
    assert.equal(elements.get('action-title').textContent, 'First');
    row.onpointerup(pointer(row)); row.onclick(pointer(row));
    assert.equal(socket.sent.length, sent, 'Holding for actions must not also play');
    fire(16); elements.get('actions').close();
    assert.equal(rows()[0].children.find(child => child.className === 'info').children[0].textContent, 'Updated First');
    row = rows()[0]; row.onpointerdown(pointer(row)); row.onpointermove(pointer(row, 200, 55));
    fire(350); assert.equal(elements.get('actions').open, false);
    row.onpointerup(pointer(row)); row.onclick(pointer(row)); fire(16);
    assert.equal(socket.sent.length, sent, 'Scrolling must cancel hold and click actions');
    row = rows()[1]; row.onpointerdown(pointer(row)); row.onpointermove(pointer(row, 121));
    row.onpointerup(pointer(row)); row.onclick(pointer(row)); fire(16);
    assert.equal(socket.sent.length, sent, 'A 79px swipe must not remove a track');
    row = rows()[1]; row.onpointerdown(pointer(row)); row.onpointermove(pointer(row, 119));
    assert.match(row.style.transform, /-81px/);
    row.onpointerup(pointer(row)); row.onclick(pointer(row));
    assert.equal(socket.sent.at(-1).event, 'remove-from-queue'); assert.equal(socket.sent.at(-1).uniqueId, 'second');
    assert.equal(row.style.transform, ''); acknowledge(); await flush(); fire(16);
    row = rows()[0];
    row.children.find(child => child.className === 'moves').children.at(-1).onpointerdown(pointer(row));
    assert.equal(list.pointer, 7);
    list.onpointermove(pointer(row, 200, 140)); fire(16);
    assert.equal(rows()[1].dataset.drop, 'bottom');
    publish(items); assert.equal(list.pointer, 7, 'A queue push retains the list pointer capture');
    list.onpointerup(pointer(row, 200, 140));
    assert.equal(socket.sent.at(-1).event, 'reorder-queue');
    assert.equal(socket.sent.at(-1).uniqueId, 'first'); assert.equal(socket.sent.at(-1).targetUniqueId, 'second'); assert.equal(socket.sent.at(-1).edge, 'bottom');
    acknowledge(); await flush();
    const after = socket.sent.length;
    row = rows()[0]; row.children.find(child => child.className === 'moves').children.at(-1).onpointerdown(pointer(row));
    list.onpointercancel(); list.onpointerup(pointer(row)); assert.equal(socket.sent.length, after);
    row.onpointerdown(pointer(row)); row.onpointercancel(); fire(350);
    assert.equal(elements.get('actions').open, false);
    row = rows()[0]; row.onpointerdown(pointer(row));
    selectors['[data-tab]'].find(button => button.dataset.tab === 'albums').onclick();
    fire(350); assert.equal(elements.get('actions').open, false, 'Navigation cancels a held action');
    const query = socket.sent.at(-1); socket.receive('albums-response', { requestId: query.requestId, items: [], hasMore: false }); await flush();
    socket.close(); fire(300);
    assert.equal(pending.size, 0, 'Gestures and requests leave no timers or animation frames behind');
});


test('remote automatic paging and debounced searches preserve scroll and discard stale replies', async () => {
    let timerId = 0;
    const pending = new Map();
    const schedule = (fn, delay) => { pending.set(++timerId, { fn, delay }); return timerId; };
    const cancel = id => pending.delete(id);
    const fire = delay => { for (const [id, timer] of [...pending]) if (timer.delay === delay) { pending.delete(id); timer.fn(); } };
    const { elements, selectors, Socket } = remoteBrowser({ timers: { setTimeout: schedule, clearTimeout: cancel } });
    elements.get('login').onsubmit({ preventDefault() {}, target: { username: 'phone', password: 'secret' } });
    const socket = Socket.instances[0]; socket.onopen();
    const tracks = selectors['[data-tab]'].find(button => button.dataset.tab === 'tracks');
    tracks.onclick();
    const first = socket.sent.at(-1);
    assert.equal(first.event, 'tracks-request'); assert.equal(first.startIndex, 0);
    const page = offset => Array.from({ length: 50 }, (_, index) => ({ id: String(offset + index), name: `Track ${offset + index}` }));
    socket.receive('tracks-response', { requestId: first.requestId, items: page(0), hasMore: true }); await flush();
    const list = elements.get('list');
    assert.equal(list.dataset.virtual, 'true');
    assert.equal(socket.sent.at(-1), first, 'The initial page must not eagerly fetch the entire library');
    list.scrollTop = 43 * 76; list.onscroll();
    const next = socket.sent.at(-1), scroll = list.scrollTop;
    assert.equal(next.startIndex, 50);
    list.onscroll(); assert.equal(socket.sent.at(-1), next, 'One request at a time near the page boundary');
    socket.receive('tracks-response', { requestId: next.requestId, items: page(50), hasMore: true }); await flush();
    assert.equal(list.scrollTop, scroll, 'Appending a page preserves the current scroll');
    assert.equal(list.children.find(row => row.attributes.role === 'listitem').attributes['aria-setsize'], '100');
    const search = elements.get('search');
    search.query = 'Old'; search.oninput(); search.query = 'New'; search.oninput();
    assert.equal(socket.sent.at(-1), next);
    fire(300); const oldSearch = socket.sent.at(-1);
    assert.equal(oldSearch.searchTerm, 'New'); assert.equal(oldSearch.startIndex, 0); assert.equal(list.scrollTop, 0);
    search.query = 'Newest'; search.oninput(); fire(300); const fresh = socket.sent.at(-1);
    socket.receive('tracks-response', { requestId: oldSearch.requestId, items: [{ name: 'Stale search' }], hasMore: false });
    socket.receive('tracks-response', { requestId: fresh.requestId, items: [{ name: 'Fresh result' }], hasMore: false }); await flush();
    assert.equal(list.children[0].children.find(child => child.className === 'info').children[0].textContent, 'Fresh result');
    search.query = 'empty'; search.oninput(); fire(300); const empty = socket.sent.at(-1);
    socket.receive('tracks-response', { requestId: empty.requestId, items: [], hasMore: true }); await flush();
    assert.equal(socket.sent.at(-1), empty); assert.equal(elements.get('more').hidden, true, 'Empty pages cannot cause an automatic fetch loop');
    search.query = 'cancelled'; search.oninput();
    selectors['[data-tab]'].find(button => button.dataset.tab === 'albums').onclick();
    const album = socket.sent.at(-1); fire(300); assert.equal(socket.sent.at(-1), album, 'Navigation cancels a pending search');
    socket.receive('albums-response', { requestId: album.requestId, items: [], hasMore: false }); await flush();
    selectors['[data-tab]'].find(button => button.dataset.tab === 'queue').onclick();
    search.query = 'from queue'; search.oninput(); fire(300);
    assert.equal(tracks.attributes['aria-current'], 'page', 'Searching from the queue activates the songs tab');
    socket.receive('tracks-response', { requestId: socket.sent.at(-1).requestId, items: [], hasMore: false }); await flush();
    socket.close(); assert.equal(pending.size, 0);
});
