(() => {
  const csrf = meta("csrf");
  const unread = Number(meta("unread") || "0");
  const native = capacitorPlugins();

  markShell(native);
  if (sendInstalledToHome(native)) {
    return;
  }
  clearFlashParams();
  registerShell();
  paintBadge(unread);
  paintTimes();
  hookInstall(native);
  hookShare(native);
  hookRewrite(csrf);
  hookReview(csrf);
  hookBusySubmit();
  hookTitleMorph();
  hookPickers();
  hookChurchSearch();
  hookPasswordToggle();
  hookHaptics(native);
  hookAlerts(csrf, native);
  if (native) {
    bootNative(native);
    if (csrf) {
      enableNativePush(csrf, native);
    }
  } else if (csrf && notificationState() === "granted") {
    enableWebPush(csrf);
  }
})();

function meta(name) {
  const node = document.querySelector(`meta[name="${name}"]`);
  return node ? node.getAttribute("content") || "" : "";
}

function capacitorPlugins() {
  return window.Capacitor && window.Capacitor.Plugins ? window.Capacitor.Plugins : null;
}

function markShell(native) {
  if (native) {
    document.body.classList.add("native-shell");
  }
  if (isStandalone()) {
    document.body.classList.add("standalone");
  }
}

function registerShell() {
  if (!("serviceWorker" in navigator)) {
    return;
  }
  navigator.serviceWorker.register("/sw.js", { scope: "/" }).catch(() => {});
}

function paintBadge(count) {
  if (!navigator.setAppBadge) {
    return;
  }
  if (count > 0) {
    navigator.setAppBadge(count);
  } else if (navigator.clearAppBadge) {
    navigator.clearAppBadge();
  }
}

function hookInstall(native) {
  const bar = document.querySelector(".install-bar");
  if (!bar || native || isStandalone()) {
    return;
  }
  if (window.localStorage && localStorage.getItem("ecclesia-install-dismissed")) {
    return;
  }
  const install = bar.querySelector("[data-install]");
  const dismiss = bar.querySelector("[data-install-dismiss]");
  let pending = null;

  window.addEventListener("beforeinstallprompt", (event) => {
    event.preventDefault();
    pending = event;
    bar.hidden = false;
  });

  if (needsManualInstall()) {
    const hint = bar.querySelector(".install-bar-ios");
    if (hint) {
      hint.hidden = false;
    }
    install.hidden = true;
    dismiss.textContent = "Got it";
    bar.classList.add("install-bar-manual");
    bar.hidden = false;
    hookInstallHelp(bar);
  }

  install.addEventListener("click", async () => {
    if (!pending) {
      return;
    }
    pending.prompt();
    await pending.userChoice;
    pending = null;
    bar.hidden = true;
  });
  dismiss.addEventListener("click", () => {
    bar.hidden = true;
    if (window.localStorage) {
      localStorage.setItem("ecclesia-install-dismissed", "1");
    }
  });
}

function sendInstalledToHome(native) {
  if (window.location.pathname !== "/") {
    return false;
  }
  if (!native && !isStandalone()) {
    return false;
  }
  window.location.replace("/home" + window.location.search + window.location.hash);
  return true;
}

function isStandalone() {
  return (
    window.matchMedia("(display-mode: standalone)").matches ||
    window.navigator.standalone === true
  );
}

function needsManualInstall() {
  const ua = window.navigator.userAgent;
  if (/iphone|ipad|ipod/i.test(ua)) {
    return true;
  }
  return window.navigator.platform === "MacIntel" && window.navigator.maxTouchPoints > 1;
}

function isIpadInstall() {
  if (/ipad/i.test(window.navigator.userAgent)) {
    return true;
  }
  return window.navigator.platform === "MacIntel" && window.navigator.maxTouchPoints > 1;
}

function hookInstallHelp(bar) {
  const helpBtn = bar.querySelector("[data-install-help]");
  const dialog = document.querySelector(".install-help");
  if (!helpBtn || !dialog || typeof dialog.showModal !== "function") {
    return;
  }
  helpBtn.hidden = false;
  const ipad = isIpadInstall();
  dialog.querySelectorAll("[data-install-device]").forEach((node) => {
    const device = node.getAttribute("data-install-device");
    node.hidden = device === "ipad" ? !ipad : ipad;
  });
  helpBtn.addEventListener("click", () => {
    dialog.showModal();
  });
}

function notificationState() {
  return window.Notification ? Notification.permission : "unsupported";
}

function hookShare(native) {
  document.addEventListener("click", async (event) => {
    const btn = event.target.closest("[data-share]");
    if (!btn) {
      return;
    }
    const title = btn.getAttribute("data-share-title") || "Ecclesia";
    const text = btn.getAttribute("data-share-text") || "";
    const url = btn.getAttribute("data-share-url") || window.location.href;
    try {
      if (native && native.Share) {
        await native.Share.share({ title, text, url, dialogTitle: title });
        return;
      }
      if (navigator.share) {
        await navigator.share({ title, text, url });
        return;
      }
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText([text, url].filter(Boolean).join("\n"));
        btn.textContent = "Copied.";
      }
    } catch (_error) {
      // The person cancelled the sheet.
    }
  });
}

function hookRewrite(csrf) {
  document.addEventListener("click", async (event) => {
    const btn = event.target.closest("[data-rewrite]");
    if (!btn || !csrf) {
      return;
    }
    const field = rewriteField(btn);
    if (!field) {
      return;
    }
    const status = btn.parentElement.querySelector(".rewrite-status");
    beginRewrite(btn, status);
    try {
      const body = await requestRewrite(csrf, btn.getAttribute("data-kind") || "", field.value);
      field.value = body.text;
      finishRewrite(status, body.seat);
    } catch (_error) {
      if (status) {
        status.hidden = false;
        status.textContent = "Couldn't rewrite.";
        status.classList.remove("is-live");
      }
    } finally {
      btn.classList.remove("is-busy");
      btn.disabled = false;
    }
  });
}

function rewriteField(btn) {
  const host = btn.closest("label");
  return host ? host.querySelector("textarea, input:not([type=hidden])") : null;
}

function beginRewrite(btn, status) {
  btn.classList.add("is-busy");
  btn.disabled = true;
  if (!status) {
    return;
  }
  status.hidden = false;
  status.textContent = "Rewriting…";
  status.classList.remove("is-live");
}

async function requestRewrite(csrf, kind, text) {
  const response = await fetch("/refine", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ csrf, kind, text }),
  });
  const body = await response.json();
  if (!response.ok || typeof body.text !== "string") {
    throw new Error("refine");
  }
  return body;
}

function finishRewrite(status, seat) {
  if (!status) {
    return;
  }
  if (seat === "echo") {
    status.textContent = "Left as you wrote it.";
    status.classList.remove("is-live");
    return;
  }
  status.textContent = "Rewritten.";
  status.classList.add("is-live");
}

function hookReview(csrf) {
  document.addEventListener("submit", async (event) => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !csrf) {
      return;
    }
    if (form.dataset.reviewed === "1") {
      return;
    }
    const fields = reviewFields(form);
    if (!fields.length) {
      return;
    }
    event.preventDefault();
    setReviewing(form, true);
    try {
      const live = await fillRefined(csrf, fields);
      if (!live) {
        markReviewed(form);
        form.submit();
        return;
      }
      showReview(form);
    } catch (_error) {
      markReviewed(form);
      form.submit();
    } finally {
      setReviewing(form, false);
    }
  });
}

function reviewFields(form) {
  return [...form.querySelectorAll("[data-rewrite]")]
    .map((btn) => ({
      field: rewriteField(btn),
      kind: btn.getAttribute("data-kind") || "",
    }))
    .filter((item) => item.field && item.field.value.trim());
}

async function fillRefined(csrf, fields) {
  let live = false;
  for (const item of fields) {
    const body = await requestRewrite(csrf, item.kind, item.field.value);
    item.field.value = body.text;
    if (body.seat === "live") {
      live = true;
    }
  }
  return live;
}

function setReviewing(form, on) {
  form.classList.toggle("is-reviewing", on);
  form.setAttribute("aria-busy", on ? "true" : "false");
  let note = form.querySelector(".review-wait");
  if (on) {
    if (!note) {
      note = document.createElement("p");
      note.className = "review-wait";
      note.setAttribute("role", "status");
      note.textContent = "Looking it over…";
      form.prepend(note);
    }
    return;
  }
  if (note) {
    note.remove();
  }
}

function markReviewed(form) {
  form.dataset.reviewed = "1";
  const pass = form.querySelector('input[name="pass"]');
  if (pass) {
    pass.value = "publish";
  }
}

function showReview(form) {
  markReviewed(form);
  if (!form.querySelector(".review-banner")) {
    const banner = document.createElement("p");
    banner.className = "review-banner";
    banner.textContent = "Read this through. Edit anything you want. Then publish.";
    form.prepend(banner);
  }
  const submit = form.querySelector('button[type="submit"]');
  if (submit) {
    submit.textContent = "Publish";
  }
  const banner = form.querySelector(".review-banner");
  if (banner && banner.scrollIntoView) {
    banner.scrollIntoView({ block: "nearest" });
  }
}

function hookHaptics(native) {
  if (!native || !native.Haptics) {
    return;
  }
  document.addEventListener("submit", () => {
    native.Haptics.impact({ style: "Light" });
  });
}

function hookAlerts(csrf, native) {
  const button = document.querySelector("[data-alerts]");
  const status = document.querySelector("[data-alerts-status]");
  paintAlertStatus(status, button, native);
  if (!button) {
    return;
  }
  button.addEventListener("click", async () => {
    if (!csrf) {
      return;
    }
    if (native && native.PushNotifications) {
      await enableNativePush(csrf, native);
    } else {
      await enableWebPush(csrf);
    }
    paintAlertStatus(status, button, native);
  });
}

function paintAlertStatus(status, button, native) {
  if (!status) {
    return;
  }
  if (native && native.PushNotifications) {
    status.hidden = true;
    return;
  }
  const state = notificationState();
  if (state === "unsupported") {
    status.hidden = false;
    status.textContent = "This browser can't show banners.";
    if (button) {
      button.hidden = true;
    }
    return;
  }
  if (state === "granted") {
    status.hidden = false;
    status.textContent = "Alerts are on.";
    if (button) {
      button.hidden = true;
    }
    return;
  }
  if (state === "denied") {
    status.hidden = false;
    status.textContent = "Alerts are blocked in the browser settings.";
    if (button) {
      button.hidden = true;
    }
  }
}

async function enableWebPush(csrf) {
  if (!("serviceWorker" in navigator) || !("PushManager" in window)) {
    return;
  }
  const permission = await Notification.requestPermission();
  if (permission !== "granted") {
    return;
  }
  const publicKey = await (await fetch("/push/vapid")).text();
  if (!publicKey) {
    return;
  }
  const registration = await navigator.serviceWorker.ready;
  const existing = await registration.pushManager.getSubscription();
  const subscription =
    existing ||
    (await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToBytes(publicKey),
    }));
  const keys = subscription.toJSON().keys || {};
  await fetch("/push/subscribe", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      csrf,
      endpoint: subscription.endpoint,
      p256dh: keys.p256dh || "",
      auth: keys.auth || "",
    }),
  });
}

async function enableNativePush(csrf, native) {
  const push = native.PushNotifications;
  if (!push) {
    return;
  }
  const permission = await push.requestPermissions();
  if (permission.receive !== "granted") {
    return;
  }
  await push.register();
  push.addListener("registration", async (token) => {
    const platform = window.Capacitor.getPlatform ? window.Capacitor.getPlatform() : "native";
    await fetch("/push/device", {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({
        csrf,
        token: token.value,
        platform,
      }),
    });
  });
  push.addListener("pushNotificationActionPerformed", (action) => {
    const url =
      (action.notification && action.notification.data && action.notification.data.url) || "/inbox";
    window.location.href = url;
  });
}

function bootNative(native) {
  if (native.StatusBar) {
    paintStatusBar(native);
    darkScheme().addEventListener("change", () => paintStatusBar(native));
  }
  if (native.SplashScreen) {
    native.SplashScreen.hide();
  }
  if (native.App) {
    native.App.addListener("backButton", ({ canGoBack }) => {
      if (canGoBack) {
        window.history.back();
      } else {
        native.App.exitApp();
      }
    });
    native.App.addListener("appUrlOpen", (event) => {
      const path = pathFromAppUrl(event.url);
      if (path) {
        window.location.href = path;
      }
    });
  }
}

function darkScheme() {
  return window.matchMedia("(prefers-color-scheme: dark)");
}

function paintStatusBar(native) {
  const dark = darkScheme().matches;
  native.StatusBar.setStyle({ style: dark ? "DARK" : "LIGHT" });
  native.StatusBar.setBackgroundColor({ color: dark ? "#0c1411" : "#f4f0e8" });
}

function clearFlashParams() {
  const url = new URL(window.location.href);
  if (!url.searchParams.has("ok") && !url.searchParams.has("err")) {
    return;
  }
  url.searchParams.delete("ok");
  url.searchParams.delete("err");
  window.history.replaceState(window.history.state, "", url.pathname + url.search + url.hash);
}

function paintTimes() {
  const format = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });
  for (const node of document.querySelectorAll("time[datetime]")) {
    const moment = new Date(node.dateTime);
    if (!Number.isNaN(moment.getTime())) {
      node.textContent = format.format(moment);
    }
  }
}

function hookBusySubmit() {
  document.addEventListener("submit", (event) => {
    if (event.defaultPrevented) {
      return;
    }
    const button = event.submitter;
    if (button && button.classList.contains("btn")) {
      button.classList.add("is-busy");
      event.target.setAttribute("aria-busy", "true");
    }
  });
  window.addEventListener("pageshow", (event) => {
    if (!event.persisted) {
      return;
    }
    for (const button of document.querySelectorAll(".btn.is-busy")) {
      button.classList.remove("is-busy");
    }
    for (const form of document.querySelectorAll("form[aria-busy]")) {
      form.removeAttribute("aria-busy");
    }
  });
}

function hookTitleMorph() {
  let morphHref = "";
  document.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return;
    }
    const card = event.target.closest("a.card-link");
    const title = card ? card.querySelector("h3") : null;
    if (!title) {
      return;
    }
    clearMorph();
    const heading = document.querySelector("main h1");
    if (heading) {
      heading.style.viewTransitionName = "none";
    }
    title.style.viewTransitionName = "page-title";
    morphHref = card.pathname;
  });
  window.addEventListener("pageswap", (event) => {
    if (!event.viewTransition) {
      return;
    }
    const destination = destinationPath(event);
    if (morphHref && destination && morphHref !== destination) {
      clearMorph();
      morphHref = "";
    }
    const heading = document.querySelector("main h1");
    if (!morphHref && heading && !onScreen(heading)) {
      heading.style.viewTransitionName = "none";
    }
  });
  window.addEventListener("pageshow", (event) => {
    if (event.persisted) {
      clearMorph();
      morphHref = "";
    }
  });
}

function destinationPath(event) {
  const entry = event.activation && event.activation.entry;
  if (!entry || !entry.url) {
    return "";
  }
  return new URL(entry.url).pathname;
}

function onScreen(node) {
  const box = node.getBoundingClientRect();
  return box.bottom > 0 && box.top < window.innerHeight;
}

function clearMorph() {
  for (const node of document.querySelectorAll("main h1, a.card-link h3")) {
    node.style.removeProperty("view-transition-name");
  }
}

function pathFromAppUrl(raw) {
  try {
    const url = new URL(raw);
    if (url.protocol === "http:" || url.protocol === "https:") {
      return url.pathname + url.search + url.hash;
    }
    const host = url.hostname || url.host;
    const path = url.pathname && url.pathname !== "/" ? url.pathname : "";
    const joined = `/${host}${path}`.replace(/\/{2,}/g, "/");
    return joined + url.search + url.hash;
  } catch (_error) {
    return "";
  }
}

function urlBase64ToBytes(value) {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(value.length / 4) * 4, "=");
  const raw = atob(padded);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i += 1) {
    bytes[i] = raw.charCodeAt(i);
  }
  return bytes;
}

function hookPasswordToggle() {
  document.addEventListener("click", (event) => {
    const btn = event.target.closest("[data-password-toggle]");
    if (!btn) {
      return;
    }
    const row = btn.closest(".password-row");
    if (!row) {
      return;
    }
    const input = row.querySelector('input[type="password"], input[type="text"]');
    if (!input) {
      return;
    }
    const showIcon = btn.querySelector(".password-icon-show");
    const hideIcon = btn.querySelector(".password-icon-hide");
    const showing = input.type === "text";
    input.type = showing ? "password" : "text";
    btn.setAttribute("aria-pressed", showing ? "false" : "true");
    btn.setAttribute("aria-label", showing ? "Show password" : "Hide password");
    if (showIcon) {
      showIcon.hidden = !showing;
    }
    if (hideIcon) {
      hideIcon.hidden = showing;
    }
  });
}

function hookChurchSearch() {
  document.addEventListener("input", (event) => {
    const query = event.target.closest("[data-church-search-input]");
    if (!query) {
      return;
    }
    const root = query.closest("[data-church-search]");
    if (!root) {
      return;
    }
    scheduleChurchSearch(root, query);
  });

  document.addEventListener("click", (event) => {
    const pick = event.target.closest("[data-church-pick]");
    if (!pick) {
      return;
    }
    const root = pick.closest("[data-church-search]");
    if (!root) {
      return;
    }
    const hidden = root.querySelector('input[name="church_id"]');
    const query = root.querySelector("[data-church-search-input]");
    if (!hidden || !query) {
      return;
    }
    hidden.value = pick.getAttribute("data-church-id") || "";
    query.value = pick.getAttribute("data-church-name") || "";
    cancelChurchSearch(root);
    clearChurchSuggestions(root);
  });
}

const churchSearchTimers = new WeakMap();
const churchSearchTokens = new WeakMap();
const churchSearchAbort = new WeakMap();
const churchSearchSent = new WeakMap();
const CHURCH_SEARCH_PAUSE_MS = 400;

function cancelChurchSearch(root) {
  const prior = churchSearchTimers.get(root);
  if (prior) {
    window.clearTimeout(prior);
    churchSearchTimers.delete(root);
  }
  const inflight = churchSearchAbort.get(root);
  if (inflight) {
    inflight.abort();
    churchSearchAbort.delete(root);
  }
}

function scheduleChurchSearch(root, query) {
  const hidden = root.querySelector('input[name="church_id"]');
  if (hidden) {
    hidden.value = "";
  }
  const prior = churchSearchTimers.get(root);
  if (prior) {
    window.clearTimeout(prior);
  }
  const timer = window.setTimeout(() => {
    churchSearchTimers.delete(root);
    void runChurchSearch(root, query.value.trim());
  }, CHURCH_SEARCH_PAUSE_MS);
  churchSearchTimers.set(root, timer);
}

async function runChurchSearch(root, text) {
  const list = root.querySelector("[data-church-search-list]");
  const status = root.querySelector("[data-church-search-status]");
  const query = root.querySelector("[data-church-search-input]");
  if (!list || !status) {
    return;
  }
  if (!text) {
    cancelChurchSearch(root);
    churchSearchSent.delete(root);
    clearChurchSuggestions(root);
    return;
  }
  if (churchSearchSent.get(root) === text) {
    return;
  }
  const previous = churchSearchAbort.get(root);
  if (previous) {
    previous.abort();
  }
  const token = (churchSearchTokens.get(root) || 0) + 1;
  churchSearchTokens.set(root, token);
  const controller = new AbortController();
  churchSearchAbort.set(root, controller);
  churchSearchSent.set(root, text);
  if (list.hidden) {
    status.hidden = false;
    status.textContent = "Searching…";
  }
  try {
    const response = await fetch(`/register/churches?q=${encodeURIComponent(text)}`, {
      headers: { Accept: "application/json" },
      signal: controller.signal,
    });
    if (token !== churchSearchTokens.get(root)) {
      return;
    }
    if (response.status === 429) {
      churchSearchSent.delete(root);
      status.hidden = false;
      status.textContent = "Wait a moment.";
      return;
    }
    if (!response.ok) {
      churchSearchSent.delete(root);
      status.hidden = false;
      status.textContent = "Search failed. Try again.";
      return;
    }
    const hits = await response.json();
    if (token !== churchSearchTokens.get(root)) {
      return;
    }
    paintChurchHits(root, hits);
  } catch (error) {
    if (error && error.name === "AbortError") {
      if (churchSearchSent.get(root) === text) {
        churchSearchSent.delete(root);
      }
      return;
    }
    if (token !== churchSearchTokens.get(root)) {
      return;
    }
    status.hidden = false;
    status.textContent = "Search failed. Try again.";
  }
}

function paintChurchHits(root, hits) {
  const list = root.querySelector("[data-church-search-list]");
  const status = root.querySelector("[data-church-search-status]");
  const query = root.querySelector("[data-church-search-input]");
  if (!list || !status) {
    return;
  }
  list.replaceChildren();
  if (!hits.length) {
    status.hidden = false;
    status.textContent = "No churches match that name.";
    list.hidden = true;
    if (query) {
      query.setAttribute("aria-expanded", "false");
    }
    return;
  }
  status.hidden = true;
  status.textContent = "";
  for (const hit of hits) {
    const item = document.createElement("li");
    item.setAttribute("role", "option");
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "church-suggestion";
    btn.setAttribute("data-church-pick", "");
    btn.setAttribute("data-church-id", hit.id);
    btn.setAttribute("data-church-name", hit.name);
    btn.textContent = `${hit.name} · ${hit.city}, ${hit.region}`;
    item.appendChild(btn);
    list.appendChild(item);
  }
  list.hidden = false;
  if (query) {
    query.setAttribute("aria-expanded", "true");
  }
}

function hookPickers() {
  let openMenu = null;

  document.addEventListener("pointerdown", (event) => {
    if (!openMenu || openMenu.root.contains(event.target) || openMenu.menu.contains(event.target)) {
      return;
    }
    openMenu.close(false);
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && openMenu) {
      event.preventDefault();
      openMenu.close(true);
    }
  });

  window.addEventListener("resize", () => {
    if (openMenu) {
      openMenu.place();
    }
  });

  window.addEventListener("scroll", () => {
    if (openMenu) {
      openMenu.place();
    }
  }, true);

  for (const select of document.querySelectorAll("select")) {
    mountPicker(select, {
      claim(next) {
        if (openMenu && openMenu !== next) {
          openMenu.close(false);
        }
        openMenu = next;
      },
      release(next) {
        if (openMenu === next) {
          openMenu = null;
        }
      },
    });
  }
}

function mountPicker(select, bus) {
  const picker = document.createElement("span");
  picker.className = "picker";
  select.before(picker);
  picker.append(select);
  select.classList.add("picker-native");
  select.tabIndex = -1;
  select.setAttribute("aria-hidden", "true");

  const face = document.createElement("button");
  face.type = "button";
  face.className = "picker-face";
  face.setAttribute("aria-haspopup", "listbox");
  face.setAttribute("aria-expanded", "false");
  const value = document.createElement("span");
  value.className = "picker-value";
  value.id = `picker-value-${Math.random().toString(36).slice(2, 8)}`;
  face.append(value);
  const label = select.closest("label");
  const caption = label ? labelText(label) : "";
  if (caption) {
    const cap = document.createElement("span");
    cap.id = `picker-cap-${Math.random().toString(36).slice(2, 8)}`;
    cap.hidden = true;
    cap.textContent = caption;
    picker.append(cap);
    face.setAttribute("aria-labelledby", `${cap.id} ${value.id}`);
  }

  const menu = document.createElement("div");
  menu.className = "picker-menu";
  menu.hidden = true;
  menu.setAttribute("role", "listbox");
  const listId = `picker-${Math.random().toString(36).slice(2, 8)}`;
  menu.id = listId;
  face.setAttribute("aria-controls", listId);
  picker.append(face);

  const options = [];
  for (const node of select.children) {
    if (node.tagName === "OPTGROUP") {
      const block = document.createElement("div");
      block.className = "picker-group";
      const label = document.createElement("div");
      label.className = "picker-label";
      label.textContent = node.label;
      block.append(label);
      for (const option of node.querySelectorAll("option")) {
        block.append(pickerOption(option, options));
      }
      menu.append(block);
    } else if (node.tagName === "OPTION") {
      menu.append(pickerOption(node, options));
    }
  }
  document.body.append(menu);

  function paint() {
    const current = select.selectedOptions[0];
    value.textContent = current ? current.textContent.trim() : "";
    for (const item of options) {
      const on = item.dataset.value === select.value;
      item.setAttribute("aria-selected", on ? "true" : "false");
      item.classList.toggle("is-selected", on);
    }
  }

  function place() {
    const box = face.getBoundingClientRect();
    const margin = 12;
    const width = Math.min(box.width, window.innerWidth - margin * 2);
    let left = box.left;
    if (left + width > window.innerWidth - margin) {
      left = window.innerWidth - margin - width;
    }
    if (left < margin) {
      left = margin;
    }
    const floor = menuFloor();
    const below = floor - box.bottom - 8;
    const above = box.top - margin;
    const upward = below < 180 && above > below;
    const room = Math.max(96, Math.min(320, upward ? above - 8 : Math.max(96, below)));
    menu.style.width = `${width}px`;
    menu.style.left = `${left}px`;
    menu.style.maxHeight = `${room}px`;
    menu.classList.toggle("is-up", upward);
    if (upward) {
      const bottom = Math.max(window.innerHeight - box.top + 6, window.innerHeight - floor);
      menu.style.top = "auto";
      menu.style.bottom = `${bottom}px`;
    } else {
      menu.style.bottom = "auto";
      menu.style.top = `${box.bottom + 6}px`;
    }
  }

  const api = {
    root: picker,
    menu,
    place,
    close(focusFace) {
      menu.hidden = true;
      face.setAttribute("aria-expanded", "false");
      picker.classList.remove("is-open");
      bus.release(api);
      if (focusFace) {
        face.focus();
      }
    },
  };

  function open() {
    const box = face.getBoundingClientRect();
    const floor = menuFloor();
    if (box.bottom > floor - 8 || box.top < 12) {
      const delta = box.bottom > floor - 8 ? box.bottom - (floor - 16) : box.top - 12;
      window.scrollBy(0, delta);
    }
    bus.claim(api);
    menu.hidden = false;
    face.setAttribute("aria-expanded", "true");
    picker.classList.add("is-open");
    place();
    const selected = options.find((item) => item.classList.contains("is-selected")) || options[0];
    if (selected) {
      selected.focus({ preventScroll: true });
      revealOption(menu, selected);
    }
  }

  face.addEventListener("click", () => {
    if (menu.hidden) {
      open();
    } else {
      api.close(false);
    }
  });

  face.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      open();
    }
  });

  select.addEventListener("mousedown", (event) => event.preventDefault());
  select.addEventListener("focus", () => face.focus());
  select.addEventListener("invalid", () => {
    face.setAttribute("aria-invalid", "true");
    face.focus();
  });

  if (label) {
    label.addEventListener("click", (event) => {
      if (event.target === face || face.contains(event.target)) {
        return;
      }
      event.preventDefault();
      if (menu.hidden) {
        open();
      }
    });
  }

  for (const item of options) {
    item.addEventListener("click", () => {
      select.value = item.dataset.value;
      select.dispatchEvent(new Event("change", { bubbles: true }));
      face.removeAttribute("aria-invalid");
      paint();
      api.close(true);
    });
  }

  menu.addEventListener("keydown", (event) => {
    const index = options.indexOf(document.activeElement);
    if (event.key === "ArrowDown") {
      event.preventDefault();
      options[Math.min(options.length - 1, index + 1)]?.focus();
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      options[Math.max(0, index - 1)]?.focus();
    } else if (event.key === "Home") {
      event.preventDefault();
      options[0]?.focus();
    } else if (event.key === "End") {
      event.preventDefault();
      options[options.length - 1]?.focus();
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      document.activeElement?.click();
    }
  });

  let scrollTimer = 0;
  menu.addEventListener("scroll", () => {
    menu.classList.add("is-scrolling");
    window.clearTimeout(scrollTimer);
    scrollTimer = window.setTimeout(() => menu.classList.remove("is-scrolling"), 700);
  });

  paint();
}

function labelText(label) {
  return Array.from(label.childNodes)
    .filter((node) => node.nodeType === Node.TEXT_NODE)
    .map((node) => node.textContent.trim())
    .filter(Boolean)
    .join(" ");
}

function pickerOption(option, options) {
  const item = document.createElement("button");
  item.type = "button";
  item.className = "picker-option";
  item.setAttribute("role", "option");
  item.tabIndex = -1;
  item.dataset.value = option.value;
  item.textContent = option.textContent.trim();
  options.push(item);
  return item;
}

function revealOption(menu, item) {
  const top = item.offsetTop;
  const bottom = top + item.offsetHeight;
  if (top < menu.scrollTop) {
    menu.scrollTop = top;
  } else if (bottom > menu.scrollTop + menu.clientHeight) {
    menu.scrollTop = bottom - menu.clientHeight;
  }
}

function menuFloor() {
  const dock = document.querySelector(".dock");
  if (!dock) {
    return window.innerHeight - 12;
  }
  const top = dock.getBoundingClientRect().top;
  return Math.min(window.innerHeight, top) - 8;
}

function clearChurchSuggestions(root) {
  const list = root.querySelector("[data-church-search-list]");
  const status = root.querySelector("[data-church-search-status]");
  const query = root.querySelector("[data-church-search-input]");
  if (list) {
    list.hidden = true;
    list.replaceChildren();
  }
  if (status) {
    status.hidden = true;
    status.textContent = "";
  }
  if (query) {
    query.setAttribute("aria-expanded", "false");
  }
}
