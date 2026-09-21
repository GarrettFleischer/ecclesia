const CACHE = "ecclesia-shell-v1";
const SHELL = [
  "/static/app.css",
  "/static/app.js",
  "/static/icon-192.png",
  "/static/apple-touch-icon.png",
  "/static/offline.html",
];

self.addEventListener("install", (event) => {
  event.waitUntil(cacheShell());
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(dropOldCaches());
  self.clients.claim();
});

self.addEventListener("fetch", (event) => {
  event.respondWith(handleFetch(event.request));
});

self.addEventListener("push", (event) => {
  event.waitUntil(showPush(event));
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const url = (event.notification.data && event.notification.data.url) || "/inbox";
  event.waitUntil(openWindow(url));
});

async function cacheShell() {
  const cache = await caches.open(CACHE);
  await cache.addAll(SHELL);
}

async function dropOldCaches() {
  const keys = await caches.keys();
  await Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key)));
}

async function handleFetch(request) {
  if (request.method !== "GET") {
    return fetch(request);
  }
  const url = new URL(request.url);
  if (url.pathname.startsWith("/static/")) {
    return cacheFirst(request);
  }
  try {
    return await fetch(request);
  } catch (error) {
    const cached = await caches.match(request);
    return cached || caches.match("/static/offline.html");
  }
}

async function cacheFirst(request) {
  const cached = await caches.match(request);
  if (cached) {
    return cached;
  }
  const response = await fetch(request);
  const cache = await caches.open(CACHE);
  cache.put(request, response.clone());
  return response;
}

async function showPush(event) {
  const data = event.data ? event.data.json() : {};
  const title = data.title || "Ecclesia";
  await self.registration.showNotification(title, {
    body: data.body || "",
    icon: "/static/icon-192.png",
    badge: "/static/icon-192.png",
    data: { url: data.url || "/inbox" },
  });
}

async function openWindow(url) {
  const clients = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
  for (const client of clients) {
    if (client.url.includes(url) && "focus" in client) {
      return client.focus();
    }
  }
  if (self.clients.openWindow) {
    return self.clients.openWindow(url);
  }
  return undefined;
}
