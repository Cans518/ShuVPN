import { Channel, invoke } from "@tauri-apps/api/core";

export interface QrEvent {
  phase: "waiting" | "redeeming" | "credentials" | "completed" | "unavailable" | "failed";
  image: number[] | null;
}

export interface Settings {
  revision: number;
  themeMode: string;
  server: string;
  timeoutSeconds: number;
  httpPort: number;
  httpListen: string;
  httpProxyEnabled: boolean;
  socksPort: number;
  socksListen: string;
  socksProxyEnabled: boolean;
  systemProxyEnabled: boolean;
  vpnEnabled: boolean;
  vpnMtu: number;
  vpnCaptureMode: string;
}

export interface DesktopSnapshot {
  account: {
    signedIn: boolean;
    accountId: string | null;
    profile: { name?: string | null } | null;
    busy: boolean;
    progressLabel: string | null;
    errorMessage: string | null;
    needsReverification: boolean;
    sessionExpired: boolean;
    credentials: { systemId: string; state: string; errorMessage: string | null }[];
    verification: {
      methods: { method: string; target: string }[];
      selectedMethod: string;
      resendAfterSeconds: number;
    } | null;
  };
  settings: Settings;
  connection: {
    phase: "disconnected" | "connecting" | "connected" | "disconnecting" | "failed";
    runId: string | null;
    cleanupPending: boolean;
    errorMessage: string | null;
    network: {
      virtualAddress: string | null;
      uploadBytes: number;
      downloadBytes: number;
      connections: number;
      warnings: string[];
      httpEndpoint: string | null;
      socksEndpoint: string | null;
    } | null;
  };
}

export interface RoutingConfiguration {
  version: number;
  profile_document: string;
  overrides_document: string;
  notices: string[];
}

export interface RoutingPreview {
  previewId: string;
  configuration: RoutingConfiguration;
}

export interface NetworkInterface {
  name: string;
  label: string;
  recommended: boolean;
}

export const backend = {
  appearance: () => invoke<Appearance>("appearance_current"),
  saveAppearance: (appearance: Appearance) => invoke<Appearance>("appearance_save", { appearance }),
  autostart: () => invoke<boolean>("startup_enabled"),
  setAutostart: (enabled: boolean) => invoke<void>("startup_set", { enabled }),
  exit: () => invoke<void>("exit_application"),
  hideWindow: () => invoke<void>("hide_window"),
  snapshot: () => invoke<DesktopSnapshot>("desktop_snapshot"),
  saveSettings: (edits: { key: string; value: string | boolean | number }[]) =>
    invoke<Settings>("save_settings", { edits }),
  signIn: (username: string, password: string) =>
    invoke<string>("sign_in", { username, password }),
  signInQr: (onEvent: (event: QrEvent) => void) => {
    const events = new Channel<QrEvent>(onEvent);
    return invoke<boolean>("sign_in_qr", { events });
  },
  sendCode: (method: string) => invoke<void>("send_code", { method }),
  verifyCode: (method: string, code: string) =>
    invoke<void>("verify_code", { method, code }),
  cancelAccount: () => invoke<void>("cancel_account_work"),
  signOut: () => invoke<string | null>("sign_out"),
  connect: (physicalInterface: string, physicalDns: string | null) =>
    invoke<void>("connect", { physicalInterface, physicalDns }),
  disconnect: () => invoke<string | null>("disconnect"),
  interfaces: () => invoke<NetworkInterface[]>("network_interfaces"),
  routing: () => invoke<RoutingConfiguration>("routing_current"),
  template: (name: string) => invoke<string>("routing_template", { name }),
  prepare: (profile: string, overrides: string, expectedVersion: number) =>
    invoke<RoutingPreview>("routing_prepare", { profile, overrides, expectedVersion }),
  commit: (previewId: string) => invoke<RoutingConfiguration>("routing_commit", { previewId }),
  discard: (previewId: string) => invoke<void>("routing_discard", { previewId }),
  rollback: (expectedVersion: number) =>
    invoke<RoutingConfiguration>("routing_rollback", { expectedVersion }),
  records: () => invoke<unknown[]>("connection_records"),
};

export interface ThemeColors {
  accent: string;
  canvas: string;
  surface: string;
  ink: string;
  muted: string;
  border: string;
}

export interface CustomTheme {
  name: string;
  light: ThemeColors;
  dark: ThemeColors;
}

export interface Appearance {
  palette: string;
  customTheme: CustomTheme;
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null && "message" in error
      && typeof error.message === "string") return error.message;
  return typeof error === "string" ? error : "操作失败，请重试";
}
