# iOS and Android

Ecclesia is one web app. Phones run it in two ways.

## Add to Home Screen

Android Chrome and iOS Safari (16.4+) can install the site as a standalone app.

1. Open the site in Safari or Chrome.
2. Share, then Add to Home Screen. Or tap Install when the bar appears.
3. Open it from the icon.
4. On You, tap **Turn on alerts**.

Notifications use Web Push. iPhone only delivers them after the site is on the home screen.

## Store build

`mobile/` is a Capacitor 6 shell. It embeds the same server in a native WebView and turns on the usual phone features: status bar, splash, keyboard, share sheet, haptics, Android back, `ecclesia://` links, badges, and native push tokens.

On a machine with Xcode and Android Studio:

```bash
cd mobile
npm install
ECCLESIA_URL=https://your-host.example npx cap add ios
ECCLESIA_URL=https://your-host.example npx cap add android
ECCLESIA_URL=https://your-host.example npx cap sync
npx cap open ios
npx cap open android
```

Add your Firebase `GoogleService-Info.plist` (iOS) and `google-services.json` (Android) so Capacitor PushNotifications can mint FCM tokens. The shell stores those tokens at `POST /push/device`.

Sending through Firebase needs `ECCLESIA_FCM_KEY` on the host. Web Push already goes out for installed PWAs without that key.

## Host keys

For a shared host, generate your own VAPID pair and set:

```
ECCLESIA_VAPID_PEM=/path/to/vapid.pem
```

The public key is derived from that PEM. You can also set `ECCLESIA_VAPID_PUBLIC` if you already have the uncompressed point.

Optional, for store builds:

```
ECCLESIA_FCM_KEY=legacy-or-server-key
```

Local development uses a built-in VAPID pair, the same way it uses a built-in cookie secret.
