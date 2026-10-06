(() => {
  let pickerBus = null;
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
  hookShare(csrf, native);
  hookPrintQr();
  hookInviteMail();
  hookRewrite(csrf);
  hookReview(csrf);
  hookBusySubmit();
  hookTitleMorph();
  hookPlaceShare();
  hookPrayToast();
  hookPickers();
  hookServiceTimes();
  hookChurchSearch();
  hookPasswordToggle();
  hookAuthForms();
  hookMarkMet();
  hookNeedReturn();
  hookHaptics(native);
  hookAlerts(csrf, native);
  hookJoinAlerts(csrf, native);
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
  if (!install || !dismiss) {
    return;
  }
  let pending = null;
  let prompting = false;
  const manual = needsManualInstall();

  if (!manual) {
    install.hidden = true;
  }

  window.addEventListener("beforeinstallprompt", (event) => {
    event.preventDefault();
    pending = event;
    if (!manual) {
      install.hidden = false;
    }
    bar.hidden = false;
  });

  window.addEventListener("appinstalled", () => {
    pending = null;
    prompting = false;
    bar.hidden = true;
  });

  if (manual) {
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

  install.addEventListener("click", () => {
    if (!pending || prompting) {
      return;
    }
    const event = pending;
    prompting = true;
    install.disabled = true;
    void (async () => {
      try {
        await event.prompt();
        await event.userChoice;
      } catch {
        /* prompt already used or browser refused */
      } finally {
        pending = null;
        prompting = false;
        install.disabled = false;
        bar.hidden = true;
      }
    })();
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

function hookPrintQr() {
  document.addEventListener("click", (event) => {
    const button = event.target.closest("[data-print-qr]");
    if (!button) {
      return;
    }
    const sheet = button.closest("[data-invite-sheet]");
    const svg = sheet && sheet.querySelector(".invite-qr");
    if (!sheet || !svg) {
      return;
    }
    printInvite(
      sheet.getAttribute("data-church-name") || "Join code",
      sheet.getAttribute("data-join") || "",
      svg
    );
  });
}

function printInvite(name, url, svg) {
  const frame = document.createElement("iframe");
  frame.className = "print-frame";
  frame.setAttribute("aria-hidden", "true");
  document.body.append(frame);
  const doc = frame.contentDocument;
  const win = frame.contentWindow;
  if (!doc || !win) {
    frame.remove();
    return;
  }
  doc.open();
  doc.write("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Join code</title><link rel=\"stylesheet\" href=\"/static/print-code.css?v=1\"></head><body><h1></h1><div class=\"mark\"></div><p></p></body></html>");
  doc.close();
  doc.title = name;
  doc.querySelector("h1").textContent = name;
  doc.querySelector(".mark").append(svg.cloneNode(true));
  doc.querySelector("p").textContent = url;
  const finish = () => frame.remove();
  win.addEventListener("afterprint", finish);
  const link = doc.querySelector("link");
  const start = () => {
    win.focus();
    win.print();
  };
  if (link.sheet) {
    start();
    return;
  }
  link.addEventListener("load", start, { once: true });
  link.addEventListener("error", start, { once: true });
}

function hookInviteMail() {
  document.addEventListener("click", (event) => {
    const opener = event.target.closest("[data-invite-email]");
    if (opener) {
      const dialog = document.querySelector("[data-invite-mail]");
      if (dialog && typeof dialog.showModal === "function") {
        dialog.showModal();
        const field = dialog.querySelector("[data-invite-rows] input");
        if (field) field.focus();
      }
      return;
    }
    const fileButton = event.target.closest("[data-invite-file]");
    if (fileButton) {
      const dialog = fileButton.closest("[data-invite-mail]");
      const input = dialog && dialog.querySelector("[data-invite-csv]");
      if (input) input.click();
      return;
    }
    const close = event.target.closest("[data-invite-close]");
    if (close) {
      const dialog = close.closest("dialog");
      if (dialog) dialog.close();
    }
  });
  document.addEventListener("input", (event) => {
    const field = event.target.closest("[data-invite-rows] input");
    if (!field) return;
    const dialog = field.closest("[data-invite-mail]");
    clearInviteFileNote(dialog);
    settleInviteRows(field.closest("[data-invite-rows]"));
  });
  document.addEventListener("change", (event) => {
    const input = event.target.closest("[data-invite-csv]");
    if (!input || !input.files || !input.files[0]) return;
    readInviteFile(input.files[0], input.closest("[data-invite-mail]"));
    input.value = "";
  });
  document.addEventListener("dragover", (event) => {
    const dialog = openInviteDialog(event);
    if (!dialog) return;
    event.preventDefault();
    const zone = dialog.querySelector("[data-invite-drop]");
    if (zone) zone.classList.add("is-dropping");
  });
  document.addEventListener("dragleave", (event) => {
    const dialog = openInviteDialog(event);
    if (!dialog) return;
    if (dialog.contains(event.relatedTarget)) return;
    const zone = dialog.querySelector("[data-invite-drop]");
    if (zone) zone.classList.remove("is-dropping");
  });
  document.addEventListener("drop", (event) => {
    const dialog = openInviteDialog(event);
    if (!dialog) return;
    event.preventDefault();
    const zone = dialog.querySelector("[data-invite-drop]");
    if (zone) zone.classList.remove("is-dropping");
    const file = event.dataTransfer && event.dataTransfer.files && event.dataTransfer.files[0];
    if (file) readInviteFile(file, dialog);
  });
}

function openInviteDialog(event) {
  const dialog = event.target.closest && event.target.closest("[data-invite-mail]");
  if (!dialog || !dialog.open) return null;
  return dialog;
}

function clearInviteFileNote(dialog) {
  const note = dialog && dialog.querySelector("[data-invite-csv-error]");
  if (!note) return;
  note.hidden = true;
  note.textContent = "";
}

function settleInviteRows(box) {
  if (!box) return;
  let fields = inviteFields(box);
  while (fields.length > 1 && fieldBlank(fields[fields.length - 1]) && fieldBlank(fields[fields.length - 2])) {
    fields[fields.length - 1].closest("[data-invite-row]").remove();
    fields = inviteFields(box);
  }
  const last = fields[fields.length - 1];
  if (last && !fieldBlank(last)) {
    box.append(blankInviteRow(box));
  }
}

function inviteFields(box) {
  return [...box.querySelectorAll("[data-invite-row] input")];
}

function fieldBlank(field) {
  return field.value.trim() === "";
}

function blankInviteRow(box) {
  const sample = box.querySelector("[data-invite-row]");
  const row = sample.cloneNode(true);
  row.querySelector("input").value = "";
  return row;
}

function readInviteFile(file, dialog) {
  if (!dialog) return;
  const reader = new FileReader();
  reader.addEventListener("load", () => {
    applyInviteCsv(dialog, String(reader.result || ""));
  });
  reader.addEventListener("error", () => {
    const note = dialog.querySelector("[data-invite-csv-error]");
    if (!note) return;
    note.hidden = false;
    note.textContent = "That file didn't open. Try again.";
  });
  reader.readAsText(file);
}

function applyInviteCsv(dialog, text) {
  const parsed = emailsFromCsv(text);
  const note = dialog.querySelector("[data-invite-csv-error]");
  const rows = dialog.querySelector("[data-invite-rows]");
  if (!note || !rows) return;
  if (parsed.error) {
    note.hidden = false;
    note.textContent = parsed.error;
    return;
  }
  const limited = parsed.emails.slice(0, 100);
  fillInviteRows(rows, limited);
  if (parsed.emails.length > 100) {
    note.hidden = false;
    note.textContent = "First 100 emails.";
    return;
  }
  note.hidden = true;
  note.textContent = "";
}

function fillInviteRows(box, emails) {
  const sample = box.querySelector("[data-invite-row]");
  if (!sample) return;
  const template = sample.cloneNode(true);
  template.querySelector("input").value = "";
  box.replaceChildren();
  const seen = new Set();
  for (const email of emails) {
    const value = email.trim();
    const key = value.toLowerCase();
    if (!value || seen.has(key)) continue;
    seen.add(key);
    const row = template.cloneNode(true);
    row.querySelector("input").value = value;
    box.append(row);
  }
  box.append(template);
  const last = box.querySelector("[data-invite-row]:last-child input");
  if (last) last.focus();
}

function emailsFromCsv(text) {
  const table = csvTable(text);
  if (!table.length) return { error: "That file has no Email column." };
  const headers = table[0].map((cell) => cell.trim().toLowerCase());
  const index = headers.findIndex(isEmailHeader);
  if (index < 0) return { error: "That file has no Email column." };
  const emails = [];
  for (const row of table.slice(1)) {
    emails.push(row[index] || "");
  }
  return { emails };
}

function isEmailHeader(cell) {
  return cell === "email" || cell === "e-mail" || cell === "email address";
}

function csvTable(text) {
  const rows = [];
  let row = [];
  let cell = "";
  let quoted = false;
  const source = text.replace(/^\uFEFF/, "").replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  for (let i = 0; i < source.length; i += 1) {
    const ch = source[i];
    if (quoted) {
      if (ch === '"') {
        if (source[i + 1] === '"') {
          cell += '"';
          i += 1;
        } else {
          quoted = false;
        }
      } else {
        cell += ch;
      }
      continue;
    }
    if (ch === '"' && cell === "") {
      quoted = true;
      continue;
    }
    if (ch === ",") {
      row.push(cell);
      cell = "";
      continue;
    }
    if (ch === "\n") {
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
      continue;
    }
    cell += ch;
  }
  if (cell.length || row.length) {
    row.push(cell);
    rows.push(row);
  }
  return rows.filter((item) => item.some((value) => value.trim() !== ""));
}

function hookShare(csrf, native) {
  document.addEventListener("click", async (event) => {
    const btn = event.target.closest("[data-share]");
    if (!btn) {
      return;
    }
    const title = btn.getAttribute("data-share-title") || "Ecclesia";
    const text = btn.getAttribute("data-share-text") || "";
    let url = btn.getAttribute("data-share-url") || "";
    const mint = btn.getAttribute("data-share-mint") || "";
    if (!url && mint) {
      url = await mintedShare(csrf, mint);
      if (!url) {
        return;
      }
    }
    if (!url) {
      url = window.location.href;
    }
    if (url.startsWith("/")) {
      url = window.location.origin + url;
    }
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

async function mintedShare(csrf, mint) {
  if (!csrf || !mint) {
    return "";
  }
  const params = new URLSearchParams({ csrf });
  const lat = document.querySelector('input[name="lat"]');
  const lng = document.querySelector('input[name="lng"]');
  if (lat && lng && lat.value && lng.value) {
    params.set("lat", lat.value);
    params.set("lng", lng.value);
  }
  const response = await fetch(mint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "text/plain" },
    body: params,
  });
  if (!response.ok) {
    return "";
  }
  return (await response.text()).trim();
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
    const submitter = event.submitter;
    const fields = reviewFields(form, submitter);
    if (!fields.length) {
      return;
    }
    event.preventDefault();
    setReviewing(form, true);
    try {
      const live = await fillRefined(csrf, fields);
      if (!live) {
        markReviewed(form);
        submitReviewed(form, submitter);
        return;
      }
      showReview(form, submitter);
    } catch (_error) {
      markReviewed(form);
      submitReviewed(form, submitter);
    } finally {
      setReviewing(form, false);
    }
  });
}

function reviewFields(form, submitter) {
  const marking = submitter instanceof HTMLButtonElement && submitter.hasAttribute("data-mark-met");
  return [...form.querySelectorAll("[data-rewrite]")]
    .map((btn) => ({
      field: rewriteField(btn),
      kind: marking ? "praise" : btn.getAttribute("data-kind") || "",
    }))
    .filter((item) => item.field && item.field.value.trim());
}

function submitReviewed(form, submitter) {
  if (submitter instanceof HTMLButtonElement) {
    const action = submitter.getAttribute("formaction");
    if (action) {
      form.setAttribute("action", action);
    }
  }
  form.submit();
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

function showReview(form, submitter) {
  markReviewed(form);
  if (!form.querySelector(".review-banner")) {
    const banner = document.createElement("p");
    banner.className = "review-banner";
    banner.textContent = "Read this through. Edit anything you want. Then publish.";
    form.prepend(banner);
  }
  const submit = submitter instanceof HTMLButtonElement
    ? submitter
    : form.querySelector('button[type="submit"]');
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

/**
 * @brief On the find-church screen, ask to show notifications after location settles.
 * @param {string} csrf Session token for the push subscription.
 * @param {object|null} native Capacitor plugins, when the app is installed.
 * @returns {void}
 */
function hookJoinAlerts(csrf, native) {
  if (!csrf || window.location.pathname !== "/churches/join") {
    return;
  }
  if (native && native.PushNotifications) {
    return;
  }
  let started = false;
  const ask = async () => {
    if (started || notificationState() !== "default") {
      return;
    }
    started = true;
    try {
      await enableWebPush(csrf);
    } finally {
      started = false;
    }
  };
  const begin = () => {
    ask().then(() => {
      if (notificationState() !== "default") {
        return;
      }
      const once = () => {
        document.removeEventListener("pointerdown", once, true);
        document.removeEventListener("keydown", once, true);
        ask();
      };
      document.addEventListener("pointerdown", once, true);
      document.addEventListener("keydown", once, true);
    });
  };
  const beginOnce = () => {
    document.removeEventListener("ecclesia-place", beginOnce);
    begin();
  };
  document.addEventListener("ecclesia-place", beginOnce);
  if (document.documentElement.dataset.placeSettled === "1") {
    beginOnce();
  }
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

function hookPlaceShare() {
  document.addEventListener("click", (event) => {
    const button = event.target.closest("[data-share-place]");
    if (!button) {
      return;
    }
    const status = document.querySelector("[data-place-status]");
    if (!navigator.geolocation) {
      if (status) {
        status.textContent = "This browser has no location.";
      }
      return;
    }
    navigator.geolocation.getCurrentPosition((position) => {
      const path = button.getAttribute("data-share-place") || "nearby";
      const url = new URL(path, window.location.origin);
      url.searchParams.set("lat", String(position.coords.latitude));
      url.searchParams.set("lng", String(position.coords.longitude));
      window.location.assign(url.pathname + url.search);
    }, () => {
      if (status) {
        status.textContent = "Location stayed off.";
      }
    }, { enableHighAccuracy: false, maximumAge: 60000, timeout: 10000 });
  });
}

function hookPrayToast() {
  const toast = document.querySelector("[data-pray-toast]");
  if (!toast || !window.localStorage) {
    return;
  }
  const key = "ecclesia-pray-toast";
  const until = Number(localStorage.getItem(key) || "0");
  if (Date.now() < until) {
    return;
  }
  const week = 7 * 24 * 60 * 60 * 1000;
  toast.hidden = false;
  localStorage.setItem(key, String(Date.now() + week));
  const dismiss = toast.querySelector("[data-pray-toast-dismiss]");
  if (!dismiss) {
    return;
  }
  dismiss.addEventListener("click", () => {
    toast.hidden = true;
    localStorage.setItem(key, String(Date.now() + week * 3));
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
    if (!morphHref && heading) {
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

function hookAuthForms() {
  document.addEventListener(
    "submit",
    (event) => {
      const form = event.target;
      if (!(form instanceof HTMLFormElement) || !form.matches("[data-auth]")) {
        return;
      }
      const problem = authProblem(form);
      if (!problem) {
        clearAuthError(form);
        return;
      }
      event.preventDefault();
      showAuthError(form, problem);
    },
    true,
  );

  document.addEventListener("input", (event) => {
    const field = event.target;
    if (!(field instanceof HTMLInputElement)) {
      return;
    }
    const form = field.closest("[data-auth]");
    if (!form) {
      return;
    }
    field.classList.remove("is-invalid");
    field.removeAttribute("aria-invalid");
    field.removeAttribute("aria-describedby");
    const error = form.querySelector("[data-auth-error]");
    if (error && !form.querySelector(".is-invalid")) {
      error.hidden = true;
      error.textContent = "";
    }
  });
}

function authProblem(form) {
  const kind = form.getAttribute("data-auth");
  const first = fieldValue(form, "first_name");
  const last = fieldValue(form, "last_name");
  const email = fieldValue(form, "email");
  const password = fieldValue(form, "password");
  if (kind === "register") {
    if (!first.trim()) {
      return { name: "first_name", message: "Fill in the required fields." };
    }
    if (!last.trim()) {
      return { name: "last_name", message: "Fill in the required fields." };
    }
  }
  const emailIssue = emailIssueOf(email);
  if (emailIssue === "empty") {
    return { name: "email", message: "Fill in the required fields." };
  }
  if (emailIssue === "bad") {
    return { name: "email", message: "Check the email address." };
  }
  if (!password) {
    return { name: "password", message: "Fill in the required fields." };
  }
  if (kind === "register" && weakPassword(password, first, last, email)) {
    return { name: "password", message: "Pick a stronger password." };
  }
  return null;
}

function fieldValue(form, name) {
  const field = form.querySelector(`[name="${name}"]`);
  return field instanceof HTMLInputElement ? field.value : "";
}

function emailIssueOf(value) {
  const email = value.trim().toLowerCase();
  if (!email) {
    return "empty";
  }
  if ([...email].length > 120 || [...email].some(emailForbidden)) {
    return "bad";
  }
  const at = email.indexOf("@");
  if (at <= 0 || email.indexOf("@", at + 1) !== -1) {
    return "bad";
  }
  const local = email.slice(0, at);
  const domain = email.slice(at + 1);
  if (!localOk(local) || !domainOk(domain)) {
    return "bad";
  }
  return "";
}

function emailForbidden(ch) {
  return /\s/u.test(ch) || ch.charCodeAt(0) < 32;
}

function localOk(local) {
  if (!local || local.startsWith(".") || local.endsWith(".")) {
    return false;
  }
  return [...local].every(
    (ch) => /[a-z0-9]/i.test(ch) || ch === "." || ch === "+" || ch === "-" || ch === "_",
  );
}

function domainOk(domain) {
  if (!domain.includes(".") || domain.includes("..")) {
    return false;
  }
  return domain.split(".").every(labelOk);
}

function labelOk(label) {
  if (!label || label.startsWith("-") || label.endsWith("-")) {
    return false;
  }
  return [...label].every((ch) => /[a-z0-9]/i.test(ch) || ch === "-");
}

const COMMON_PASSWORDS = new Set([
  "password",
  "password1",
  "password123",
  "passw0rd",
  "123456",
  "1234567",
  "12345678",
  "123456789",
  "1234567890",
  "qwerty",
  "qwerty123",
  "qwertyuiop",
  "letmein",
  "welcome",
  "welcome1",
  "admin",
  "admin123",
  "iloveyou",
  "sunshine",
  "princess",
  "football",
  "baseball",
  "dragon",
  "master",
  "login",
  "abc123",
  "abcdef",
  "abcdefg",
  "111111",
  "000000",
  "trustno1",
  "starwars",
  "monkey",
  "shadow",
  "michael",
  "jordan",
  "harley",
  "ranger",
  "hunter",
  "buster",
  "soccer",
  "hockey",
  "killer",
  "george",
  "andrew",
  "charlie",
  "thomas",
  "robert",
  "daniel",
  "jessica",
  "pepper",
  "access",
  "flower",
  "summer",
  "winter",
  "orange",
  "ginger",
  "cheese",
  "computer",
  "internet",
  "freedom",
  "mustang",
  "secret",
  "changeme",
  "asdfghjkl",
  "zxcvbnm",
  "1q2w3e4r",
  "pass",
  "pass123",
  "guest",
  "default",
  "654321",
  "123123",
  "112233",
  "superman",
  "batman",
]);

function weakPassword(password, first, last, email) {
  const chars = [...password];
  if (chars.length === 0 || chars.length > 128 || chars.length < 8) {
    return true;
  }
  const lower = password.toLowerCase();
  if (COMMON_PASSWORDS.has(lower)) {
    return true;
  }
  if (new Set(chars).size < 3) {
    return true;
  }
  if (/^\d+$/.test(password) && chars.length < 12) {
    return true;
  }
  const local = email.trim().toLowerCase().split("@")[0] || "";
  const hints = [first, last, local]
    .map((hint) => hint.trim().toLowerCase())
    .filter((hint) => hint.length >= 3);
  return hints.some((hint) => lower.includes(hint));
}

function showAuthError(form, problem) {
  const field = form.querySelector(`[name="${problem.name}"]`);
  const error = form.querySelector("[data-auth-error]");
  form.querySelectorAll(".is-invalid").forEach((node) => {
    node.classList.remove("is-invalid");
    node.removeAttribute("aria-invalid");
    node.removeAttribute("aria-describedby");
  });
  if (error) {
    error.hidden = false;
    error.textContent = problem.message;
  }
  if (field instanceof HTMLInputElement) {
    field.classList.add("is-invalid");
    field.setAttribute("aria-invalid", "true");
    if (error) {
      field.setAttribute("aria-describedby", error.id);
    }
    field.focus();
  }
}

function clearAuthError(form) {
  const error = form.querySelector("[data-auth-error]");
  if (error) {
    error.hidden = true;
    error.textContent = "";
  }
  form.querySelectorAll(".is-invalid").forEach((node) => {
    node.classList.remove("is-invalid");
    node.removeAttribute("aria-invalid");
    node.removeAttribute("aria-describedby");
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

  const bus = {
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
  };

  pickerBus = bus;
  for (const select of document.querySelectorAll("select")) {
    mountPicker(select, bus);
  }
  for (const input of document.querySelectorAll("input[list]")) {
    mountSuggest(input, bus);
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
    placeMenu(menu, face);
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

  select.addEventListener("change", () => {
    face.removeAttribute("aria-invalid");
    paint();
  });

  select._picker = api;
  paint();
}

const SERVICE_LIMIT = 8;

function hookServiceTimes() {
  const root = document.querySelector("[data-service-times]");
  const template = document.getElementById("service-row");
  if (!root || !template || !pickerBus) {
    return;
  }
  const add = root.querySelector("[data-service-add]");
  add.addEventListener("click", () => {
    if (serviceRows(root).length >= SERVICE_LIMIT) {
      return;
    }
    const row = template.content.firstElementChild.cloneNode(true);
    root.querySelector("[data-service-list]").append(row);
    mountPicker(row.querySelector("select"), pickerBus);
    packServiceNames(root);
    syncServiceRows(root);
    row.querySelector("input[type='time']")?.focus();
  });
  root.addEventListener("click", (event) => {
    const button = event.target.closest("[data-service-remove]");
    if (!button || !root.contains(button)) {
      return;
    }
    const row = button.closest("[data-service-row]");
    const rows = serviceRows(root);
    if (rows.length <= 1) {
      clearServiceRow(row);
      return;
    }
    row.querySelector("select")?._picker?.close(false);
    row.querySelector("select")?._picker?.menu.remove();
    row.remove();
    packServiceNames(root);
    syncServiceRows(root);
  });
}

function packServiceNames(root) {
  serviceRows(root).forEach((row, index) => {
    row.querySelector("select").name = `service_day_${index}`;
    row.querySelector("input[type='time']").name = `service_time_${index}`;
  });
}

function serviceRows(root) {
  return [...root.querySelectorAll("[data-service-list] [data-service-row]")];
}

function syncServiceRows(root) {
  const rows = serviceRows(root);
  const alone = rows.length === 1;
  for (const row of rows) {
    const button = row.querySelector("[data-service-remove]");
    if (alone) {
      button.setAttribute("hidden", "");
    } else {
      button.removeAttribute("hidden");
    }
  }
  root.querySelector("[data-service-add]").disabled = rows.length >= SERVICE_LIMIT;
}

function clearServiceRow(row) {
  const select = row.querySelector("select");
  const time = row.querySelector("input[type='time']");
  if (select) {
    select.value = "";
    select.dispatchEvent(new Event("change"));
  }
  if (time) {
    time.value = "";
  }
}

function labelText(label) {
  return Array.from(label.childNodes)
    .filter((node) => node.nodeType === Node.TEXT_NODE)
    .map((node) => node.textContent.trim())
    .filter(Boolean)
    .join(" ");
}

function placeMenu(menu, anchor) {
  const box = anchor.getBoundingClientRect();
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

function mountSuggest(input, bus) {
  const source = document.getElementById(input.getAttribute("list") || "");
  if (!source) {
    return;
  }
  const names = [...source.querySelectorAll("option")]
    .map((option) => option.value || option.textContent.trim())
    .filter(Boolean);
  if (names.length === 0) {
    return;
  }
  input.removeAttribute("list");
  input.setAttribute("autocomplete", "off");
  input.classList.add("picker-suggest");
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");

  const menu = document.createElement("div");
  menu.className = "picker-menu";
  menu.hidden = true;
  menu.setAttribute("role", "listbox");
  const listId = `picker-${Math.random().toString(36).slice(2, 8)}`;
  menu.id = listId;
  input.setAttribute("aria-controls", listId);
  document.body.append(menu);

  let active = 0;

  function rows() {
    const query = input.value.trim().toLowerCase();
    if (!query) {
      return names;
    }
    return names.filter((name) => name.toLowerCase().includes(query));
  }

  function paint(matches, reset) {
    if (reset) {
      const exact = matches.findIndex(
        (name) => name.toLowerCase() === input.value.trim().toLowerCase(),
      );
      active = exact >= 0 ? exact : 0;
    }
    if (active >= matches.length) {
      active = Math.max(0, matches.length - 1);
    }
    menu.replaceChildren();
    matches.forEach((name, index) => {
      const item = document.createElement("button");
      item.type = "button";
      item.className = "picker-option";
      item.setAttribute("role", "option");
      item.tabIndex = -1;
      item.dataset.value = name;
      item.textContent = name;
      const on = index === active;
      item.setAttribute("aria-selected", on ? "true" : "false");
      item.classList.toggle("is-selected", on);
      if (on) {
        item.id = `${listId}-on`;
      }
      item.addEventListener("mousedown", (event) => event.preventDefault());
      item.addEventListener("click", () => choose(name));
      menu.append(item);
    });
    if (matches.length > 0) {
      input.setAttribute("aria-activedescendant", `${listId}-on`);
    } else {
      input.removeAttribute("aria-activedescendant");
    }
  }

  function choose(name) {
    input.value = name;
    input.removeAttribute("aria-invalid");
    api.close(false);
  }

  const api = {
    root: input.closest("label") || input,
    menu,
    place() {
      placeMenu(menu, input);
    },
    close(focusInput) {
      menu.hidden = true;
      input.setAttribute("aria-expanded", "false");
      input.removeAttribute("aria-activedescendant");
      bus.release(api);
      if (focusInput) {
        input.focus();
      }
    },
  };

  function show(reset) {
    const matches = rows();
    if (matches.length === 0) {
      api.close(false);
      return;
    }
    paint(matches, reset);
    bus.claim(api);
    menu.hidden = false;
    input.setAttribute("aria-expanded", "true");
    api.place();
    const selected = menu.querySelector(".is-selected");
    if (selected) {
      revealOption(menu, selected);
    }
  }

  input.addEventListener("focus", () => show(true));
  input.addEventListener("click", () => {
    if (menu.hidden) {
      show(true);
    }
  });
  input.addEventListener("input", () => show(true));
  input.addEventListener("keydown", (event) => {
    if (menu.hidden) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        show(true);
      }
      return;
    }
    const count = menu.children.length;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      active = Math.min(count - 1, active + 1);
      show(false);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      active = Math.max(0, active - 1);
      show(false);
    } else if (event.key === "Enter") {
      const pick = rows()[active];
      if (input.value.trim() && pick && pick !== input.value) {
        event.preventDefault();
        choose(pick);
      } else {
        api.close(false);
      }
    }
  });
  input.addEventListener("blur", () => {
    window.setTimeout(() => {
      if (document.activeElement !== input && !menu.contains(document.activeElement)) {
        api.close(false);
      }
    }, 0);
  });

  let scrollTimer = 0;
  menu.addEventListener("scroll", () => {
    menu.classList.add("is-scrolling");
    window.clearTimeout(scrollTimer);
    scrollTimer = window.setTimeout(() => menu.classList.remove("is-scrolling"), 700);
  });
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

function hookMarkMet() {
  document.querySelectorAll("[data-reply-form]").forEach((form) => {
    const box = form.querySelector("textarea");
    const mark = form.querySelector("[data-mark-met]");
    if (!box || !mark) {
      return;
    }
    const sync = () => {
      mark.disabled = box.value.trim() === "";
    };
    box.addEventListener("input", sync);
    sync();
  });
}

function hookNeedReturn() {
  restoreNeedReturn();
  document.addEventListener("click", (event) => {
    const link = event.target.closest("a[data-need]");
    if (!link) {
      return;
    }
    const list = link.closest("[data-church-needs]");
    if (!list) {
      return;
    }
    const saved = {
      churchId: list.getAttribute("data-church-needs") || "",
      scrollY: window.scrollY,
      needId: link.getAttribute("data-need") || "",
      flash: false,
    };
    try {
      sessionStorage.setItem("ecclesia.needReturn", JSON.stringify(saved));
    } catch (error) {
      return;
    }
  });
}

function restoreNeedReturn() {
  const link = document.querySelector("[data-need-back]");
  if (link) {
    rememberNeedMark(link);
  }
  const params = new URLSearchParams(window.location.search);
  if (params.get("return") !== "1") {
    return;
  }
  const root = document.querySelector("[data-church-needs]");
  let raw = null;
  try {
    raw = sessionStorage.getItem("ecclesia.needReturn");
  } catch (error) {
    raw = null;
  }
  let saved = null;
  if (raw) {
    try {
      saved = JSON.parse(raw);
    } catch (error) {
      saved = null;
    }
  }
  if (root && saved && saved.churchId === root.getAttribute("data-church-needs")) {
    const needId = typeof saved.needId === "string" && /^[A-Za-z0-9_-]+$/.test(saved.needId) ? saved.needId : "";
    const card = needId ? document.getElementById(`need-${needId}`) : null;
    if (typeof saved.scrollY === "number" && Number.isFinite(saved.scrollY)) {
      window.scrollTo(0, saved.scrollY);
    } else if (card && card.scrollIntoView) {
      card.scrollIntoView({ block: "center", behavior: "instant" });
    }
    if (saved.flash && card) {
      card.classList.add("is-met");
      window.setTimeout(() => {
        card.classList.remove("is-met");
      }, 1600);
    }
    try {
      sessionStorage.removeItem("ecclesia.needReturn");
    } catch (error) {
      return;
    }
  }
  document.documentElement.classList.remove("need-return");
  params.delete("return");
  let next = window.location.pathname;
  const query = params.toString();
  if (query) {
    next += `?${query}`;
  }
  if (window.location.hash) {
    next += window.location.hash;
  }
  window.history.replaceState(window.history.state, "", next);
}

function rememberNeedMark(link) {
  const marking = link.hasAttribute("data-just-met");
  const reopened = link.hasAttribute("data-reopened");
  if (!marking && !reopened) {
    return;
  }
  const churchId = link.getAttribute("data-church") || "";
  const needId = link.getAttribute("data-need") || "";
  const key = "ecclesia.needReturn";
  let saved = null;
  try {
    saved = JSON.parse(sessionStorage.getItem(key) || "null");
  } catch (error) {
    saved = null;
  }
  if (marking) {
    if (!saved || saved.churchId !== churchId) {
      saved = { churchId, scrollY: null, needId, flash: true };
    } else {
      saved.needId = needId;
      saved.flash = true;
    }
    try {
      sessionStorage.setItem(key, JSON.stringify(saved));
    } catch (error) {
      return;
    }
    return;
  }
  if (saved && saved.churchId === churchId) {
    saved.flash = false;
    try {
      sessionStorage.setItem(key, JSON.stringify(saved));
    } catch (error) {
      return;
    }
  }
}
