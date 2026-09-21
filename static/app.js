(() => {
  const csrf = meta("csrf");
  const unread = Number(meta("unread") || "0");
  const native = capacitorPlugins();

  markShell(native);
  registerShell();
  paintBadge(unread);
  hookInstall(native);
  hookShare(native);
  hookRewrite(csrf);
  hookReview(csrf);
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
  const install = bar.querySelector("[data-install]");
  const dismiss = bar.querySelector("[data-install-dismiss]");
  let pending = null;

  window.addEventListener("beforeinstallprompt", (event) => {
    event.preventDefault();
    pending = event;
    bar.hidden = false;
  });

  if (isIos()) {
    bar.querySelector("p").textContent = "Share, then Add to Home Screen.";
    install.hidden = true;
    bar.hidden = false;
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
  });
}

function isStandalone() {
  return (
    window.matchMedia("(display-mode: standalone)").matches ||
    window.navigator.standalone === true
  );
}

function isIos() {
  return /iphone|ipad|ipod/i.test(window.navigator.userAgent);
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
        status.textContent = "Couldn't rewrite. Try again.";
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
    status.textContent = "No rewrite model is running. Your words are unchanged.";
    status.classList.remove("is-live");
    return;
  }
  status.textContent = "Rewritten. Edit anything you want.";
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
    form.classList.add("is-reviewing");
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
      form.classList.remove("is-reviewing");
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
    native.StatusBar.setStyle({ style: "DARK" });
    native.StatusBar.setBackgroundColor({ color: "#243126" });
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
