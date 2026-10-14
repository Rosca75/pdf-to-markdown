import { mustFind } from "./dom";

/** Handles to the static skeleton; views patch these in place. */
export interface Refs {
  root: HTMLElement;
  dropzone: HTMLElement;
  dropTitle: HTMLElement;
  dropSub: HTMLElement;
  notice: HTMLElement;
  noticeText: HTMLElement;
  noticeDismiss: HTMLButtonElement;
  progress: HTMLElement;
  progressText: HTMLElement;
  progressBar: HTMLElement;
  progressFill: HTMLElement;
  listHead: HTMLElement;
  listCount: HTMLElement;
  clearAll: HTMLButtonElement;
  files: HTMLUListElement;
  pvTitle: HTMLElement;
  pvMeta: HTMLElement;
  pvMode: HTMLButtonElement;
  copy: HTMLButtonElement;
  pvText: HTMLElement;
  pvRendered: HTMLElement;
  pvState: HTMLElement;
  pvStateTitle: HTMLElement;
  pvStateBody: HTMLElement;
  pvRetry: HTMLButtonElement;
  result: HTMLElement;
  resultSummary: HTMLElement;
  resultDismiss: HTMLButtonElement;
  resultFailed: HTMLUListElement;
  resultSaved: HTMLDetailsElement;
  resultSavedSummary: HTMLElement;
  resultSavedList: HTMLUListElement;
  barStatus: HTMLElement;
  convert: HTMLButtonElement;
  live: HTMLElement;
}

const TEMPLATE = `
<div class="app">
  <header class="appbar">
    <svg class="logo" viewBox="0 0 32 32" width="26" height="26" aria-hidden="true" focusable="false">
      <rect x="1" y="1" width="30" height="30" rx="8" fill="currentColor"/>
      <path d="M9 6h9l5 5v15a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1z" fill="var(--paper)"/>
      <path d="M18 6l5 5h-4a1 1 0 0 1-1-1z" fill="var(--accent-wash)"/>
      <path d="M11 24v-7l2.5 3.5L16 17v7M18 17v7h1.2a3.5 3.5 0 0 0 0-7z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
    <h1>PDF2MD</h1>
    <p class="tagline">PDF in, Markdown out. Your files never leave this computer.</p>
  </header>

  <main class="panels">
    <section class="panel panel-input" aria-label="Files">
      <div id="dropzone" class="dropzone" role="button" tabindex="0"
           aria-label="Add PDF files. Press Enter or Space to browse, or drop files here.">
        <svg class="dz-glyph" viewBox="0 0 64 64" width="56" height="56" aria-hidden="true" focusable="false">
          <path d="M16 6h22l12 12v36a3 3 0 0 1-3 3H16a3 3 0 0 1-3-3V9a3 3 0 0 1 3-3z" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round"/>
          <path d="M38 6v12h12" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round"/>
          <path d="M32 29v17m-7-7l7 7 7-7" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <div class="dz-text">
          <div class="dz-title" id="dz-title">Drop PDF files here</div>
          <div class="dz-sub" id="dz-sub">or click to browse</div>
        </div>
      </div>

      <div id="notice" class="notice" role="alert" hidden>
        <p id="notice-text"></p>
        <button id="notice-dismiss" class="icon-btn" type="button" aria-label="Dismiss message">&times;</button>
      </div>

      <div id="progress" class="progress" hidden>
        <p id="progress-text" class="progress-text" aria-hidden="true"></p>
        <div id="progress-bar" class="bar-track" role="progressbar" aria-label="Overall extraction progress"
             aria-valuemin="0" aria-valuemax="100"><div id="progress-fill" class="bar-fill"></div></div>
      </div>

      <div id="list-head" class="list-head" hidden>
        <span id="list-count" class="list-count"></span>
        <button id="clear-all" class="link-btn" type="button">Clear all</button>
      </div>
      <ul id="files" class="files" aria-label="PDF files"></ul>
    </section>

    <section class="panel panel-preview" aria-label="Markdown preview">
      <div class="preview-head">
        <h2 id="pv-title" class="pv-title">Preview</h2>
        <span id="pv-meta" class="pv-meta"></span>
        <button id="pv-mode" class="btn" type="button" disabled>Show raw</button>
        <button id="copy" class="btn" type="button" disabled>Copy</button>
      </div>
      <div class="preview-body">
        <pre id="pv-text" class="pv-text" tabindex="0" aria-label="Extracted Markdown (read-only)" hidden></pre>
        <div id="pv-rendered" class="pv-md" tabindex="0" role="document" aria-label="Rendered Markdown (read-only)" hidden></div>
        <div id="pv-state" class="pv-state">
          <p id="pv-state-title" class="pv-state-title"></p>
          <p id="pv-state-body" class="pv-state-body"></p>
          <button id="pv-retry" class="btn" type="button" hidden>Try again</button>
        </div>
      </div>
    </section>
  </main>

  <footer class="footer">
    <div id="result" class="result" hidden>
      <div class="result-head">
        <p id="result-summary" class="result-summary" role="status"></p>
        <button id="result-dismiss" class="icon-btn" type="button" aria-label="Dismiss summary">&times;</button>
      </div>
      <ul id="result-failed" class="result-failed"></ul>
      <details id="result-saved" class="result-saved">
        <summary id="result-saved-summary"></summary>
        <ul id="result-saved-list"></ul>
      </details>
    </div>
    <div class="bar">
      <p id="bar-status" class="bar-status"></p>
      <button id="convert" class="btn btn-primary" type="button" disabled>Convert to Markdown</button>
    </div>
  </footer>
  <div id="live" class="sr-only" aria-live="polite" aria-atomic="true"></div>
</div>
`;

export function buildShell(root: HTMLElement): Refs {
  root.innerHTML = TEMPLATE;
  const f = <T extends Element>(sel: string) => mustFind<T>(root, sel);
  return {
    root,
    dropzone: f("#dropzone"),
    dropTitle: f("#dz-title"),
    dropSub: f("#dz-sub"),
    notice: f("#notice"),
    noticeText: f("#notice-text"),
    noticeDismiss: f("#notice-dismiss"),
    progress: f("#progress"),
    progressText: f("#progress-text"),
    progressBar: f("#progress-bar"),
    progressFill: f("#progress-fill"),
    listHead: f("#list-head"),
    listCount: f("#list-count"),
    clearAll: f("#clear-all"),
    files: f("#files"),
    pvTitle: f("#pv-title"),
    pvMeta: f("#pv-meta"),
    pvMode: f("#pv-mode"),
    copy: f("#copy"),
    pvText: f("#pv-text"),
    pvRendered: f("#pv-rendered"),
    pvState: f("#pv-state"),
    pvStateTitle: f("#pv-state-title"),
    pvStateBody: f("#pv-state-body"),
    pvRetry: f("#pv-retry"),
    result: f("#result"),
    resultSummary: f("#result-summary"),
    resultDismiss: f("#result-dismiss"),
    resultFailed: f("#result-failed"),
    resultSaved: f("#result-saved"),
    resultSavedSummary: f("#result-saved-summary"),
    resultSavedList: f("#result-saved-list"),
    barStatus: f("#bar-status"),
    convert: f("#convert"),
    live: f("#live"),
  };
}
