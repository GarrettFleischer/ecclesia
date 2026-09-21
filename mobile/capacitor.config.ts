import type { CapacitorConfig } from "@capacitor/cli";

const serverUrl = process.env.ECCLESIA_URL;

const config: CapacitorConfig = {
  appId: "app.ecclesia.mobile",
  appName: "Ecclesia",
  webDir: "www",
  backgroundColor: "#f4eee3",
  plugins: {
    SplashScreen: {
      launchAutoHide: true,
      launchShowDuration: 400,
      backgroundColor: "#f4eee3",
    },
    StatusBar: {
      style: "DARK",
      backgroundColor: "#243126",
    },
    PushNotifications: {
      presentationOptions: ["badge", "sound", "alert"],
    },
    Keyboard: {
      resize: "body",
    },
  },
  ios: {
    contentInset: "automatic",
    scheme: "ecclesia",
    preferredContentMode: "mobile",
  },
  android: {
    allowMixedContent: true,
    captureInput: true,
  },
};

if (serverUrl) {
  const host = hostOf(serverUrl);
  config.server = {
    url: serverUrl,
    cleartext: serverUrl.startsWith("http://"),
    allowNavigation: host ? [host] : [],
  };
}

function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return "";
  }
}

export default config;
