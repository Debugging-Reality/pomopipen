// Stand-ins for the Tauri window and log APIs, so the real stats and tasks
// pages (src/routes/*) render in the review harness without a Tauri runtime.
const noop = async (..._args: unknown[]) => {};
const window = {
  label: 'preview',
  show: noop,
  close: noop,
  minimize: noop,
  toggleMaximize: noop,
  startDragging: noop,
  startResizeDragging: noop,
  setZoom: noop,
  isMaximized: async () => false,
  onResized: async (..._args: unknown[]) => () => {},
};
export const getCurrentWebviewWindow = () => window;
export const getCurrentWebview = () => window;
export const info = noop;
export const warn = noop;
export const error = noop;
export const debug = noop;
export const trace = noop;
/** The real titlebar (?view=jots) opens other windows through this; the harness has none. */
export class WebviewWindow {
  static getByLabel = async (_label: string) => null;
  constructor(_label: string, _options?: unknown) {}
}
