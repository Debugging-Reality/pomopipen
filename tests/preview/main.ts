import { mount } from 'svelte';
import App from './App.svelte';

// Outside Tauri there is no asset protocol: pass picture URLs through as-is.
(window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = { convertFileSrc: (path: string) => path };

mount(App, { target: document.getElementById('app')! });
