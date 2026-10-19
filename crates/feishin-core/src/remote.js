'use strict';

// Correlate every reply, including overlapping searches and delayed operations.
class RemoteRequests {
    constructor(send) { this.send = send; this.pending = new Map(); }
    request(message, responseEvent = 'operation-ack') {
        const requestId = crypto.randomUUID();
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
                this.pending.delete(requestId);
                reject(new Error('Operation timed out. Refresh to check its result.'));
            }, 31000);
            this.pending.set(requestId, { resolve, reject, timer, responseEvent });
            if (!this.send({ ...message, requestId })) {
                clearTimeout(timer);
                this.pending.delete(requestId);
                reject(new Error('Connection closed. Reconnect to continue.'));
            }
        });
    }
    receive(event, data) {
        const pending = this.pending.get(data?.requestId);
        if (!pending || event !== pending.responseEvent) return false;
        clearTimeout(pending.timer);
        this.pending.delete(data.requestId);
        if (data.error) pending.reject(new Error(data.error));
        else pending.resolve(data);
        return true;
    }
    close() {
        for (const pending of this.pending.values()) {
            clearTimeout(pending.timer);
            pending.reject(new Error('Connection closed. Refresh after reconnecting.'));
        }
        this.pending.clear();
    }
}

if (typeof module !== 'undefined') module.exports = { RemoteRequests };
if (typeof document !== 'undefined') (() => {
    if (typeof navigator !== 'undefined' && 'serviceWorker' in navigator && globalThis.isSecureContext) {
        navigator.serviceWorker.register('/worker.js').catch(() => {});
    }
    const el = id => document.getElementById(id);
    let socket, tab = 'queue', offset = 0, term = '', listing = [], queue = [], currentKey;
    let retryTimer;
    let radioActive = false, playerState;
    let accentColors;
    const applyTheme = () => {
        const color = accentColors?.[document.documentElement.dataset.theme];
        if (typeof color === 'string' && CSS.supports('color', color)) document.documentElement.style.setProperty('--accent', color);
        else document.documentElement.style.removeProperty('--accent');
    };
    let song, duration = 0, confirmChanges = true, viewRevision = 0, listingBusy = false;
    let actionTarget, actionRevision = 0, playlistOffset = 0, playlistTerm = '', searchTimer;
    let heldRow, drag, dragFrame = 0, selectionTimer;
    const send = message => {
        if (socket?.readyState !== WebSocket.OPEN) return false;
        socket.send(JSON.stringify(message));
        return true;
    };
    const requests = new RemoteRequests(send);
    const error = value => { el('error').textContent = value?.message || String(value); };
    const time = seconds => `${Math.floor((seconds || 0) / 60)}:${String(Math.floor((seconds || 0) % 60)).padStart(2, '0')}`;
    const confirmReplace = name => !confirmChanges || queue.length === 0 || confirm(`Replace the current queue and play ${name}?`);
    const button = (label, action, aria) => {
        const button = document.createElement('button');
        button.textContent = label;
        if (aria) button.setAttribute('aria-label', aria);
        button.onclick = async () => {
            button.disabled = true;
            try { await action(); } catch (value) { error(value); }
            finally { button.disabled = false; }
        };
        return button;
    };
    const clearSelection = () => window.getSelection?.()?.removeAllRanges();
    function cancelHold(cancelled = true) {
        if (!heldRow) return;
        clearTimeout(heldRow.timer); heldRow.row.style.transform = ''; heldRow.cancelled = cancelled; heldRow = undefined;
        clearTimeout(selectionTimer);
        selectionTimer = setTimeout(() => { clearSelection(); document.removeEventListener?.('selectionchange', clearSelection); }, 300);
    }
    function cancelDrag() {
        if (drag?.indicator) delete drag.indicator.dataset.drop;
        if (drag && el('list').hasPointerCapture(drag.pointer)) el('list').releasePointerCapture(drag.pointer);
        drag = undefined; cancelAnimationFrame(dragFrame); dragFrame = 0;
    }
    function updateDrag() {
        if (!drag) return;
        if (drag.indicator) delete drag.indicator.dataset.drop;
        const rows = [...el('list').children].filter(row => row.getAttribute('role') === 'listitem');
        const row = rows.find(row => row.getBoundingClientRect().bottom >= drag.y) || rows.at(-1);
        if (!row) return;
        const bounds = row.getBoundingClientRect();
        drag.target = row.dataset.uniqueId;
        drag.edge = drag.y > bounds.top + bounds.height / 2 ? 'bottom' : 'top';
        if (drag.target !== drag.key) { row.dataset.drop = drag.edge; drag.indicator = row; }
    }
    function autoScrollDrag() {
        dragFrame = 0;
        if (!drag) return;
        const list = el('list'), virtual = list.dataset.virtual === 'true';
        const bounds = list.getBoundingClientRect();
        const top = Math.max(0, bounds.top), bottom = Math.min(window.innerHeight, bounds.bottom);
        const direction = drag.y < top + 48 ? -1 : drag.y > bottom - 48 ? 1 : 0;
        const scroller = virtual ? list : document.scrollingElement;
        const previous = scroller.scrollTop;
        scroller.scrollTop += direction * 12;
        if (virtual && previous !== scroller.scrollTop) render();
        updateDrag();
        if (direction && previous !== scroller.scrollTop) dragFrame = requestAnimationFrame(autoScrollDrag);
    }
    function rowGestures(row, item, kind) {
        let press;
        row.onpointerdown = event => {
            if (event.button !== 0 || event.target.closest('button')) return;
            cancelHold(); clearTimeout(selectionTimer); clearSelection();
            document.addEventListener?.('selectionchange', clearSelection);
            press = heldRow = { row, key: item.uniqueId, x: event.clientX, y: event.clientY };
            press.timer = setTimeout(() => {
                press.fired = true; clearSelection();
                if (typeof navigator !== 'undefined') navigator.vibrate?.(10);
                showActions(item, kind);
            }, 350);
        };
        row.onpointermove = event => {
            if (!press || heldRow !== press) return;
            press.dx = event.clientX - press.x;
            const dy = event.clientY - press.y;
            if (Math.hypot(press.dx, dy) > 10 && !press.fired) {
                clearTimeout(press.timer); press.moved = true;
                if (kind === 'queue' && press.dx < -10 && Math.abs(press.dx) > Math.abs(dy) + 10) {
                    press.swipe = true; row.setPointerCapture(event.pointerId);
                    row.style.transform = `translateX(${Math.max(-140, press.dx)}px)`;
                }
            }
        };
        row.onpointerup = () => {
            if (!press || heldRow !== press) return;
            const remove = press.swipe && press.dx < -80;
            cancelHold(false);
            if (remove && queue.some(entry => entry.uniqueId === item.uniqueId)) requests.request({ event: 'remove-from-queue', uniqueId: item.uniqueId }).catch(error);
            requestAnimationFrame(render);
        };
        row.onpointercancel = () => { cancelHold(); render(); };
        row.onpointerleave = () => { if (press && heldRow === press && !press.swipe) { cancelHold(); render(); } };
        row.onclick = event => {
            if (event.target.closest('button')) return;
            if (press?.fired || press?.moved || press?.cancelled) return;
            (kind === 'queue' ? requests.request({ event: 'queue-jump', uniqueId: item.uniqueId }) : play(item, kind, 'now')).catch(error);
        };
        row.oncontextmenu = event => { event.preventDefault(); cancelHold(); showActions(item, kind); };
    }
    function renderPlayer(state) {
        playerState = state;
        song = state.song;
        duration = Math.max(0, song?.duration || 0);
        el('title').textContent = song?.name || 'Nothing playing';
        el('artist').textContent = [song?.artistName, song?.album].filter(Boolean).join(' · ');
        el('time').textContent = `${time(state.position)} / ${radioActive ? 'LIVE' : time(duration)}`;
        if (document.activeElement !== el('seek')) el('seek').value = duration ? 1000 * state.position / duration : 0;
        el('seek').disabled = radioActive || !duration;
        for (const id of ['previous', 'next']) el(id).disabled = radioActive || !song?.id;
        for (const id of ['repeat', 'shuffle']) el(id).disabled = radioActive;
        for (const id of ['play', 'pause']) el(id).disabled = !song?.id && !radioActive;
        el('stop-radio').hidden = !radioActive;
        if (document.activeElement !== el('volume')) el('volume').value = state.volume;
        el('play').hidden = state.status === 'playing';
        el('pause').hidden = state.status !== 'playing';
        el('shuffle').setAttribute('aria-pressed', String(!!state.shuffle));
        el('repeat').textContent = `Repeat: ${state.repeat}`;
        el('favorite').disabled = !song || !duration;
        el('favorite').setAttribute('aria-pressed', String(!!song?.userFavorite));
        el('rating').disabled = !song || !duration;
        el('rating').value = song?.userRating || 0;
        el('artwork').hidden = !song?.imageUrl;
        if (song?.imageUrl && el('artwork').getAttribute('src') !== song.imageUrl) el('artwork').src = song.imageUrl;
    }
    function playEvent(kind) {
        return kind === 'albums' ? 'play-album' : kind === 'playlists' ? 'play-playlist' : kind === 'radio' ? 'play-radio' : 'play-track';
    }
    async function play(item, kind, playType, radio = false) {
        if (kind !== 'radio' && (playType === 'now' || playType === 'shuffle') && !confirmReplace(item.name)) return;
        await requests.request({ event: radio ? 'play-track-radio' : playEvent(kind), id: item.id, serverId: item._serverId, playType });
        el('actions').close();
    }
    function render() {
        if (heldRow) return;
        const values = tab === 'queue' ? queue : listing;
        const list = el('list'), virtual = values.length > 20, rowHeight = 76;
        list.dataset.virtual = String(virtual);
        list.setAttribute('role', 'list');
        if (virtual) list.scrollTop = Math.min(list.scrollTop, Math.max(0, values.length * rowHeight - list.clientHeight));
        const first = virtual ? Math.max(0, Math.floor(list.scrollTop / rowHeight) - 4) : 0;
        const end = virtual ? Math.min(values.length, Math.ceil((list.scrollTop + list.clientHeight) / rowHeight) + 4) : values.length;
        list.replaceChildren();
        if (!values.length) {
            const empty = document.createElement('p');
            empty.textContent = tab === 'queue' ? 'Queue is empty' : `No ${tab === 'radio' ? 'radio stations' : tab} found`;
            list.append(empty);
        }
        const spacer = count => {
            const space = document.createElement('div');
            space.style.height = `${count * rowHeight}px`;
            space.setAttribute('aria-hidden', 'true');
            list.append(space);
        };
        if (virtual) spacer(first);
        el('clear').hidden = tab !== 'queue';
        for (let index = first; index < end; index++) {
            const item = values[index];
            const row = document.createElement('div');
            row.className = 'row' + (tab === 'queue' && item.uniqueId === currentKey ? ' current' : '');
            row.dataset.index = String(index);
            if (item.uniqueId) row.dataset.uniqueId = item.uniqueId;
            row.setAttribute('role', 'listitem');
            row.setAttribute('aria-posinset', String(index + 1));
            row.setAttribute('aria-setsize', String(values.length));
            rowGestures(row, item, tab);
            if (item.imageUrl) {
                const image = document.createElement('img');
                image.src = item.imageUrl; image.alt = ''; image.loading = 'lazy';
                row.append(image);
            }
            const info = document.createElement('div'), title = document.createElement('strong'), artist = document.createElement('small');
            info.className = 'info'; title.textContent = item.name;
            artist.textContent = [item.artistName || item.albumArtistName, item.album, item.duration ? time(item.duration) : ''].filter(Boolean).join(' · ');
            info.append(title, artist); row.append(info);
            row.append(button('▶', () => tab === 'queue' ? requests.request({ event: 'queue-jump', uniqueId: item.uniqueId }) : play(item, tab, 'now'), `Play ${item.name}`));
            row.append(button('⋯', () => showActions(item, tab), `Actions for ${item.name}`));
            if (tab === 'queue') {
                row.draggable = true;
                row.ondragstart = event => event.dataTransfer.setData('text/plain', item.uniqueId);
                row.ondragover = event => event.preventDefault();
                row.ondrop = event => {
                    event.preventDefault();
                    const uniqueId = event.dataTransfer.getData('text/plain');
                    if (queue.some(entry => entry.uniqueId === uniqueId) && uniqueId !== item.uniqueId)
                        requests.request({ event: 'reorder-queue', uniqueId, targetUniqueId: item.uniqueId, edge: event.clientY > row.getBoundingClientRect().top + row.offsetHeight / 2 ? 'bottom' : 'top' }).catch(error);
                };
                const moves = document.createElement('div'); moves.className = 'moves';
                if (index > 0) moves.append(button('↑', () => requests.request({ event: 'reorder-queue', uniqueId: item.uniqueId, targetUniqueId: values[index - 1].uniqueId, edge: 'top' }), `Move ${item.name} up`));
                if (index + 1 < values.length) moves.append(button('↓', () => requests.request({ event: 'reorder-queue', uniqueId: item.uniqueId, targetUniqueId: values[index + 1].uniqueId, edge: 'bottom' }), `Move ${item.name} down`));
                const handle = button('↕', () => {}, `Drag ${item.name} to reorder`);
                handle.className = 'drag-handle';
                handle.onpointerdown = event => {
                    if (event.button !== 0) return;
                    event.preventDefault(); event.stopPropagation(); cancelHold(); cancelDrag();
                    drag = { key: item.uniqueId, pointer: event.pointerId, y: event.clientY };
                    list.setPointerCapture(event.pointerId); updateDrag();
                };
                moves.append(handle);
                row.append(moves);
            }
            list.append(row);
        }
        if (virtual) spacer(values.length - end);
        if (drag) updateDrag();
    }
    async function load(more = false) {
        const revision = viewRevision, selected = tab;
        if (tab === 'queue') { el('more').hidden = true; render(); return; }
        if (listingBusy) return;
        listingBusy = true; el('more').disabled = true;
        if (!more) { offset = 0; listing = []; el('list').scrollTop = 0; render(); }
        try {
            const page = await requests.request({ event: `${selected}-request`, searchTerm: term, startIndex: offset, limit: 50 }, `${selected}-response`);
            if (revision !== viewRevision) return;
            listing.push(...page.items); offset += page.items.length;
            el('more').hidden = !page.hasMore || page.items.length === 0; render();
        } catch (value) { if (revision === viewRevision) error(value); }
        finally { if (revision === viewRevision) { listingBusy = false; el('more').disabled = false; maybeLoadMore(); } }
    }
    function maybeLoadMore() {
        const list = el('list');
        if (tab === 'queue' || listingBusy || el('more').hidden || !listing.length) return;
        const nearEnd = list.dataset.virtual === 'true'
            ? Math.ceil((list.scrollTop + list.clientHeight) / 76) >= listing.length - 8
            : list.getBoundingClientRect().bottom <= window.innerHeight + 8 * 76;
        if (nearEnd) load(true);
    }
    function selectTab(selected) {
        tab = selected;
        document.querySelectorAll('[data-tab]').forEach(button => button.setAttribute('aria-current', button.dataset.tab === tab ? 'page' : 'false'));
    }
    async function playlistPage(more = false) {
        const revision = actionRevision, target = actionTarget;
        if (!more) { playlistOffset = 0; el('playlist-choices').replaceChildren(); }
        el('playlist-more').disabled = true;
        try {
            const page = await requests.request({ event: 'playlists-request', serverId: target._serverId, searchTerm: playlistTerm, startIndex: playlistOffset, limit: 50 }, 'playlists-response');
            if (revision !== actionRevision) return;
            for (const playlist of page.items) el('playlist-choices').append(button(playlist.name, async () => {
                await requests.request({ event: 'add-to-playlist', songId: target.id, serverId: target._serverId, playlistId: playlist.id });
                el('actions').close();
            }));
            playlistOffset += page.items.length; el('playlist-more').hidden = !page.hasMore;
        } catch (value) { error(value); }
        finally { el('playlist-more').disabled = false; }
    }
    function showActions(item, kind) {
        actionRevision++; actionTarget = item;
        el('action-title').textContent = item.name;
        el('action-buttons').replaceChildren(); el('playlist-picker').hidden = true;
        const modes = [['Play', 'now'], ['Next', 'next'], ['Last', 'last'], ['Play shuffled', 'shuffle'], ['Next shuffled', 'nextShuffle'], ['Last shuffled', 'lastShuffle']];
        for (const [label, mode] of modes) {
            if (kind === 'radio' && mode !== 'now') continue;
            el('action-buttons').append(button(label, () => play(item, kind, mode)));
        }
        if (kind === 'tracks' || kind === 'queue') {
            for (const [label, mode] of modes.slice(0, 3)) el('action-buttons').append(button(`Track radio: ${label}`, () => play(item, 'tracks', mode, true)));
            el('action-buttons').append(button('Add to playlist', async () => { el('playlist-picker').hidden = false; playlistTerm = ''; await playlistPage(); }));
        }
        if (kind === 'queue') el('action-buttons').append(button('Remove from queue', async () => {
            await requests.request({ event: 'remove-from-queue', uniqueId: item.uniqueId }); el('actions').close();
        }));
        if (kind === 'radio' && item.homepageUrl) {
            try {
                const url = new URL(item.homepageUrl);
                if (['http:', 'https:'].includes(url.protocol)) el('action-buttons').append(button('Station homepage', () => window.open(url.href, '_blank', 'noopener,noreferrer')));
            } catch { /* Invalid station links are not navigable. */ }
        }
        el('actions').showModal();
    }
    function connect(data) {
        clearTimeout(retryTimer); retryTimer = undefined;
        el('error').textContent = '';
        const previous = socket; socket = undefined;
        previous?.close(4001); requests.close();
        const connection = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`);
        socket = connection;
        connection.onopen = () => connection.send(JSON.stringify({ event: 'authenticate', header: 'Basic ' + btoa(unescape(encodeURIComponent(`${data.get('username')}:${data.get('password')}`))) }));
        connection.onclose = event => {
            if (socket !== connection) return;
            requests.close(); viewRevision++; listingBusy = false;
            clearTimeout(searchTimer);
            cancelHold(); cancelDrag();
            el('app').hidden = true; el('login').hidden = false;
            error(event.code === 4004 ? 'Authentication failed. Check your credentials.' : 'Connection closed. Connect again to continue.');
            if (![4000, 4001, 4002, 4003, 4004].includes(event.code)) {
                error('Connection lost. Reconnecting...');
                retryTimer = setTimeout(() => {
                    retryTimer = undefined;
                    if (socket === connection) connect(data);
                }, 2000);
            }
        };
        connection.onerror = () => { if (socket === connection) error('Cannot reach the desktop. Check the remote address.'); };
        connection.onmessage = event => {
            if (socket !== connection) return;
            let message;
            try { message = JSON.parse(event.data); } catch { return; }
            const { event: type, data } = message;
            if (requests.receive(type, data)) return;
            if (type === 'state') {
                el('login').hidden = true; el('app').hidden = false;
                renderPlayer(data);
            } else if (type === 'queue-state') { queue = data.items; currentKey = data.currentUniqueId; if (heldRow?.key && !queue.some(item => item.uniqueId === heldRow.key)) cancelHold(); if (drag && !queue.some(item => item.uniqueId === drag.key)) cancelDrag(); if (tab === 'queue') render(); }
            else if (type === 'radio-status') { radioActive = !!data.isActive; if (playerState) renderPlayer(playerState); }
            else if (type === 'accent-color') { accentColors = data; applyTheme(); }
            else if (type === 'confirm-queue-changes-setting') confirmChanges = data;
            else if (type === 'favorite' && song?.id === data.id) { song.userFavorite = data.favorite; el('favorite').setAttribute('aria-pressed', String(data.favorite)); }
            else if (type === 'rating' && song?.id === data.id) { song.userRating = data.rating; el('rating').value = data.rating; }
            else if (type === 'proxy') el('full-art').src = `data:image/jpeg;base64,${data}`;
            else if (type === 'error') error(data);
        };
    }
    el('login').onsubmit = event => {
        event.preventDefault();
        connect(new FormData(event.target));
    };
    document.querySelectorAll('[data-event]').forEach(button => button.onclick = () => send({ event: button.dataset.event }));
    document.querySelectorAll('[data-tab]').forEach(button => button.onclick = () => {
        clearTimeout(searchTimer);
        cancelHold(); cancelDrag(); el('list').scrollTop = 0;
        selectTab(button.dataset.tab); viewRevision++; listingBusy = false;
        load();
    });
    document.querySelectorAll('[data-close]').forEach(button => button.onclick = () => el(button.dataset.close).close());
    el('actions').onclose = () => { actionRevision++; };
    el('search').onsubmit = event => { event.preventDefault(); clearTimeout(searchTimer); cancelHold(); cancelDrag(); term = new FormData(event.target).get('query'); if (tab === 'queue') selectTab('tracks'); viewRevision++; listingBusy = false; load(); };
    el('search').oninput = () => { clearTimeout(searchTimer); searchTimer = setTimeout(() => el('search').onsubmit({ preventDefault() {}, target: el('search') }), 300); };
    el('playlist-search').onsubmit = event => { event.preventDefault(); playlistTerm = new FormData(event.target).get('query'); actionRevision++; playlistPage(); };
    el('playlist-more').onclick = () => playlistPage(true);
    el('more').onclick = () => load(true);
    el('list').onscroll = () => { if (el('list').dataset.virtual === 'true') render(); maybeLoadMore(); };
    window.addEventListener?.('scroll', maybeLoadMore, { passive: true });
    el('list').onpointermove = event => {
        if (!drag || event.pointerId !== drag.pointer) return;
        drag.y = event.clientY; updateDrag();
        if (!dragFrame) dragFrame = requestAnimationFrame(autoScrollDrag);
    };
    el('list').onpointerup = event => {
        if (!drag || event.pointerId !== drag.pointer) return;
        const { key, target, edge } = drag;
        cancelDrag();
        if (target && key !== target && queue.some(item => item.uniqueId === key) && queue.some(item => item.uniqueId === target)) requests.request({ event: 'reorder-queue', uniqueId: key, targetUniqueId: target, edge }).catch(error);
    };
    el('list').onpointercancel = () => cancelDrag();
    el('refresh').onclick = () => { viewRevision++; listingBusy = false; load(); };
    el('clear').onclick = async () => { if (!queue.length || (confirmChanges && !confirm('Clear the current queue?'))) return; el('clear').disabled = true; try { await requests.request({ event: 'clear-queue' }); } catch (value) { error(value); } finally { el('clear').disabled = false; } };
    el('volume').onchange = event => send({ event: 'volume', volume: Number(event.target.value) });
    el('seek').onchange = event => send({ event: 'position', position: Number(event.target.value) / 1000 * duration });
    el('favorite').onclick = () => { if (song) send({ event: 'favorite', id: song.id, serverId: song._serverId, favorite: !song.userFavorite }); };
    el('rating').onchange = event => { if (song) send({ event: 'rating', id: song.id, serverId: song._serverId, rating: Number(event.target.value) }); };
    el('artwork').onclick = () => { if (!song?.imageUrl) return; el('full-art').src = song.imageUrl; el('art-dialog').showModal(); send({ event: 'proxy' }); };
    let theme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    try { const saved = localStorage.getItem('feishin-remote-theme'); if (saved === 'dark' || saved === 'light') theme = saved; } catch { /* Theme preference is optional. */ }
    document.documentElement.dataset.theme = theme;
    el('theme').onclick = () => { const theme = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark'; document.documentElement.dataset.theme = theme; applyTheme(); try { localStorage.setItem('feishin-remote-theme', theme); } catch {} };
})();
