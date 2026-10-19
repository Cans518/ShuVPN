import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { afterEach, mock, test } from "node:test";

// Exercise the actual TypeScript controllers with only their IPC and DOM
// boundaries replaced. Real DOM/layout and native IPC remain WebView smoke gates.
class Control {
  dataset = {};
  textContent = "";
  value = "";
  checked = false;
  disabled = false;
  hidden = false;
  children = [];
  fields = new Map();
  isConnected = true;
  append(child) { this.children.push(child); }
  replaceChildren(...children) { this.children = children; }
}

const backend = {};
function element(root, selector) {
  if (!root.fields.has(selector)) root.fields.set(selector, new Control());
  return root.fields.get(selector);
}
function text(root, selector, value) { element(root, selector).textContent = value; }
globalThis.__shuvpnPageTests = { backend, element, text };
for (const name of ["HTMLElement", "HTMLButtonElement", "HTMLInputElement",
  "HTMLFormElement", "HTMLSelectElement", "HTMLUListElement", "HTMLImageElement"]) {
  globalThis[name] = Control;
}
globalThis.document = { createElement: () => new Control() };

const stubs = {
  "./api": "export const backend = globalThis.__shuvpnPageTests.backend;",
  "./dom": "export const { element, text } = globalThis.__shuvpnPageTests; export const bytes = String;",
  "./icons": "export function decorateIcons() {}",
};
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (context.parentURL?.includes("/desktop/src/") && specifier in stubs) {
      return { url: `data:text/javascript,${encodeURIComponent(stubs[specifier])}`, shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
});
const { createAccountPage } = await import("../src/account-page.ts");
const { createConnectionPage } = await import("../src/connection-page.ts");

afterEach(() => mock.restoreAll());

function deferred() {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
}

function snapshot() {
  return {
    account: { signedIn: false, busy: false, credentials: [], verification: null },
    connection: { phase: "disconnected", cleanupPending: false, network: null },
    settings: { server: "gateway.invalid", httpProxyEnabled: true, socksProxyEnabled: true,
      httpListen: "127.0.0.1", socksListen: "127.0.0.1", httpPort: 0, socksPort: 0 },
  };
}

test("a late account cancellation cannot dispose the current page or its QR image", async () => {
  const cancellation = deferred();
  const login = deferred();
  const pending = [];
  backend.cancelAccount = () => cancellation.promise;
  backend.signInQr = onEvent => {
    onEvent({ phase: "waiting", image: [1, 2, 3] });
    return login.promise;
  };
  mock.method(URL, "createObjectURL", () => "blob:current-page");
  const revoked = mock.method(URL, "revokeObjectURL", () => {});
  const page = createAccountPage(action => pending.push(action()), error => { throw error; });
  const first = new Control();
  const verifying = snapshot();
  verifying.account.verification = { methods: [], selectedMethod: "", resendAfterSeconds: 0 };
  page.show(first);
  page.update(first, verifying);
  const late = page.cancelWork(verifying);

  // A newer navigation has committed while the earlier cancellation is pending.
  page.dispose();
  first.isConnected = false;
  const current = new Control();
  page.show(current);
  page.update(current, snapshot());
  element(current, "#qr-sign-in").onclick();
  assert.equal(element(current, "#qr-image").src, "blob:current-page");
  assert.equal(element(current, "#sign-in").disabled, true);
  cancellation.resolve();
  await late;
  assert.equal(revoked.mock.callCount(), 0);
  assert.equal(element(current, "#qr-display").hidden, false);

  login.resolve(true);
  await Promise.all(pending);
  assert.equal(revoked.mock.callCount(), 1);
  assert.equal(element(current, "#qr-display").hidden, true);
  page.update(current, snapshot());
  element(current, "#qr-sign-in").onclick();
  assert.equal(element(current, "#sign-in").disabled, true, "new page must retain immediate busy feedback");
  await Promise.all(pending);
  page.dispose();
});

test("connection submission stays exclusive across page visits and still permits cancellation", async () => {
  const startup = deferred();
  let connections = 0;
  let disconnects = 0;
  const pending = [];
  backend.interfaces = async () => [];
  backend.connect = () => { connections++; return startup.promise; };
  backend.disconnect = async () => { disconnects++; return null; };
  const current = snapshot();
  current.account.signedIn = true;
  let root = new Control();
  const page = createConnectionPage({
    run: action => pending.push(action()),
    report: error => { throw error; },
    navigate: () => assert.fail("signed-in connection should not navigate"),
    refreshView: () => page.update(root, current),
  });
  await page.show(root, current);
  element(root, "#connect").onclick();
  element(root, "#connect").onclick();
  assert.equal(connections, 1);
  assert.equal(element(root, "#connect").disabled, true);
  root.isConnected = false;
  root = new Control();
  await page.show(root, current);
  element(root, "#connect").onclick();
  assert.equal(connections, 1);
  current.connection.phase = "connecting";
  page.update(root, current);
  assert.equal(element(root, "#connect").disabled, false);
  element(root, "#connect").onclick();
  assert.equal(disconnects, 1);
  startup.resolve();
  await Promise.all(pending);
});
