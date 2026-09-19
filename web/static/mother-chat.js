/**
 * Mother chat panel v0 (S10-T3): a persistent, multi-turn chat with the Mother Agent.
 * POST /api/sessions/{sid}/chat streams the same field events as an intent — cards bloom
 * in the field while the synthesized reply lands in the transcript. GET on the same route
 * hydrates history after a refresh. Live mode only; disable with localStorage zavora_p2=0.
 */
(function () {
  'use strict';

  async function boot() {
    if (window.__ZAVORA_BOOT__) await window.__ZAVORA_BOOT__;
    if (window.__ZAVORA_DEMO__) return;
    if (localStorage.getItem('zavora_p2') === '0') return;

    const openBtn = document.getElementById('chatOpen');
    const panel = document.getElementById('chatPanel');
    const list = document.getElementById('chatList');
    const form = document.getElementById('chatForm');
    const input = document.getElementById('chatText');
    const closeBtn = document.getElementById('chatClose');
    if (!openBtn || !panel || !list || !form || !input) return;

    let hydrated = false;
    let inFlight = false;
    let pendingEl = null;

    function turnEl(role, content) {
      const d = document.createElement('div');
      d.className = 'chat-turn ' + role;
      if (role === 'user') d.textContent = content;
      else d.innerHTML = content; // Mother replies carry the same <b> markup as Suzy summaries
      list.appendChild(d);
      list.scrollTop = list.scrollHeight;
      return d;
    }
    function setEmpty(msg) {
      list.innerHTML = `<div class="p2-empty">${msg}</div>`;
    }
    function clearEmpty() {
      const e = list.querySelector('.p2-empty');
      if (e) e.remove();
    }

    async function hydrate() {
      if (hydrated) return;
      hydrated = true;
      const live = window.__ZAVORA_LIVE__;
      try {
        await live?.ensureSession?.();
        const sid = live?.getSessionId?.();
        if (!sid) return setEmpty('Say hello — Suzy is listening.');
        const res = await fetch(`/api/sessions/${sid}/chat`, { credentials: 'include' });
        if (!res.ok) return setEmpty('Say hello — Suzy is listening.');
        const data = await res.json();
        const turns = data.turns || [];
        if (!turns.length) return setEmpty('Say hello — Suzy is listening.');
        turns.forEach((t) => turnEl(t.role === 'user' ? 'user' : 'mother', t.text));
      } catch (_) {
        setEmpty('Say hello — Suzy is listening.');
      }
    }

    function open() {
      document.getElementById('apprPanel')?.setAttribute('hidden', '');
      panel.removeAttribute('hidden');
      hydrate();
      input.focus();
    }
    function close() {
      panel.setAttribute('hidden', '');
    }
    openBtn.addEventListener('click', () => (panel.hidden ? open() : close()));
    if (closeBtn) closeBtn.addEventListener('click', close);

    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      const text = input.value.trim();
      if (!text || inFlight) return;
      const live = window.__ZAVORA_LIVE__;
      if (!live || !live.chat) {
        clearEmpty();
        turnEl('mother', 'Live orchestration is unavailable — check your connection.');
        return;
      }
      clearEmpty();
      input.value = '';
      turnEl('user', text);
      pendingEl = turnEl('mother', '…');
      pendingEl.classList.add('pending');
      inFlight = true;
      try {
        await live.chat(text);
      } catch (err) {
        if (err && err.name !== 'AbortError' && pendingEl) {
          pendingEl.classList.remove('pending');
          pendingEl.textContent = `Could not reach Suzy — ${err.message || 'try again'}.`;
          pendingEl = null;
        }
      } finally {
        inFlight = false;
        if (pendingEl && pendingEl.classList.contains('pending')) pendingEl.remove();
        pendingEl = null;
      }
    });

    // The reply arrives on the shared event stream while our turn is in flight.
    window.addEventListener('zavora:field-event', (e) => {
      if (!inFlight) return;
      const ev = e.detail || {};
      if (ev.type === 'suzy_summary') {
        if (pendingEl) {
          pendingEl.classList.remove('pending');
          pendingEl.innerHTML = ev.html || '';
          pendingEl = null;
        } else {
          turnEl('mother', ev.html || '');
        }
      } else if (ev.type === 'error' && pendingEl) {
        pendingEl.classList.remove('pending');
        pendingEl.textContent = ev.message || 'Something went wrong.';
        pendingEl = null;
      }
    });

    openBtn.removeAttribute('hidden');
  }

  boot().catch((err) => console.warn('[zavora] mother-chat boot failed', err));
})();
