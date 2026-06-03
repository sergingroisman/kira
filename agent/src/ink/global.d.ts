import type { DOMElement } from './dom.js';

declare global {
  namespace JSX {
    interface IntrinsicElements {
      'ink-root': any;
      'ink-box': any;
      'ink-text': any;
      'ink-virtual-text': any;
      'ink-link': any;
      'ink-progress-bar': any;
      'ink-raw-ansi': any;
    }
  }
}

export {};
