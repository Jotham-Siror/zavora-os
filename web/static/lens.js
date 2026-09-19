/**
 * Domain lens (S10-T2, pulled forward): view Both / Work / Home. Filters cards, agent
 * tiles and approval rows by their domain tag (ADR-002); shared items always show.
 * Persisted in localStorage until preferences move server-side (S10). Each switch is
 * recorded as a content-free ui_lens ledger event. Live mode only.
 */
(function () {
  'use strict';

  const KEY = 'zavora_lens';

  async function boot() {
    if (window.__ZAVORA_BOOT__) await window.__ZAVORA_BOOT__;
    if (window.__ZAVORA_DEMO__) return;
    if (localStorage.getItem('zavora_p2') === '0') return;

    const ctl = document.getElementById('lensCtl');
    if (!ctl) return;
    const btns = [...ctl.querySelectorAll('button')];

    function set(lens, record) {
      if (lens === 'both') delete document.body.dataset.lens;
      else document.body.dataset.lens = lens;
      btns.forEach((b) => b.classList.toggle('on', b.dataset.lens === lens));
      try {
        localStorage.setItem(KEY, lens);
      } catch (_) {
        /* private browsing */
      }
      if (record) {
        window.__ZAVORA_LIVE__?.recordUiEvent?.('ui_lens', {
          domain: lens === 'both' ? 'shared' : lens,
        });
      }
    }

    btns.forEach((b) => b.addEventListener('click', () => set(b.dataset.lens, true)));

    const saved = localStorage.getItem(KEY);
    if (saved === 'work' || saved === 'home') set(saved, false);

    ctl.removeAttribute('hidden');
  }

  boot().catch((err) => console.warn('[zavora] lens boot failed', err));
})();
