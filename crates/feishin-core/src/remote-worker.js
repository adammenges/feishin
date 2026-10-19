'use strict';
const cacheName = 'feishin-native-remote-v1';
const shell = ['/', '/remote.js', '/manifest.json', '/icon-256.png', '/icon-512.png'];
self.addEventListener('install', event => {
    event.waitUntil(caches.open(cacheName).then(cache => cache.addAll(shell)).then(() => self.skipWaiting()));
});
self.addEventListener('activate', event => {
    event.waitUntil(Promise.all([
        self.clients.claim(),
        caches.keys().then(keys => Promise.all(keys.filter(key => key.startsWith('feishin-native-remote-') && key !== cacheName).map(key => caches.delete(key))))
    ]));
});
self.addEventListener('fetch', event => {
    const url = new URL(event.request.url);
    // Only the public shell can be cached. Artwork, commands and credentials stay live.
    if (event.request.method !== 'GET' || url.origin !== self.location.origin || url.search || !shell.includes(url.pathname)) return;
    event.respondWith((async () => {
        try {
            const response = await fetch(event.request);
            if (response.ok && response.type === 'basic') {
                event.waitUntil(caches.open(cacheName).then(cache => cache.put(event.request, response.clone())).catch(() => {}));
            }
            return response;
        } catch (error) {
            const cached = await caches.match(event.request);
            if (cached) return cached;
            throw error;
        }
    })());
});
