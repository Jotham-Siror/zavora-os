/**
 * Approvals inbox v0 (S10-T4): pending actions queued by the permission gate (S2).
 * Lists GET /api/actions?status=pending, offers approve / reject / edit and batch
 * approve, and reacts live to permission_request / action_result SSE events.
 * The actions routes are "known"-level: signed-out sessions see a sign-in hint,
 * never fabricated data. Live mode only; disable with localStorage zavora_p2=0.
 */
(function () {
  'use strict';

  function toast(msg) {
    const host = document.getElementById('p2Toasts');
    if (!host) return;
    const t = document.createElement('div');
    t.className = 'p2-toast';
    t.textContent = msg;
    host.appendChild(t);
    setTimeout(() => t.remove(), 6000);
  }
  window.__ZAVORA_P2_TOAST__ = toast;

  async function boot() {
    if (window.__ZAVORA_BOOT__) await window.__ZAVORA_BOOT__;
    if (window.__ZAVORA_DEMO__) return;
    if (localStorage.getItem('zavora_p2') === '0') return;

    const openBtn = document.getElementById('apprOpen');
    const panel = document.getElementById('apprPanel');
    const list = document.getElementById('apprList');
    const countEl = document.getElementById('apprCount');
    const allBtn = document.getElementById('apprAll');
    const closeBtn = document.getElementById('apprClose');
    if (!openBtn || !panel || !list || !countEl || !allBtn) return;

    let items = [];
    let signedOut = false;

    const sid = () => window.__ZAVORA_LIVE__?.getSessionId?.() || null;
    const qs = () => {
      const s = sid();
      return s ? `&session_id=${encodeURIComponent(s)}` : '';
    };

    function setBadge(n) {
      if (n > 0) {
        countEl.textContent = String(n);
        countEl.removeAttribute('hidden');
      } else {
        countEl.setAttribute('hidden', '');
      }
    }

    function expiresIn(iso) {
      const ms = new Date(iso).getTime() - Date.now();
      if (!isFinite(ms)) return '';
      if (ms <= 0) return 'expired';
      const h = Math.floor(ms / 3600000);
      return h >= 1 ? `expires in ${h}h` : `expires in ${Math.max(1, Math.round(ms / 60000))}m`;
    }

    function render() {
      list.innerHTML = '';
      allBtn.setAttribute('hidden', '');
      if (signedOut) {
        list.innerHTML =
          '<div class="p2-empty">Sign in (top right) to review actions waiting for your approval.</div>';
        return;
      }
      if (!items.length) {
        list.innerHTML = '<div class="p2-empty">Nothing waiting for you — agents will queue actions here.</div>';
        return;
      }
      if (items.length > 1) allBtn.removeAttribute('hidden');
      items.forEach((a) => {
        const row = document.createElement('div');
        row.className = 'appr-row';
        if (a.domain === 'work' || a.domain === 'home') row.classList.add('dom-' + a.domain);
        const meta = document.createElement('div');
        meta.className = 'appr-meta';
        meta.innerHTML = `<span class="agent-chip"></span><span class="effect-chip ${a.effect}"></span><span class="appr-when"></span>`;
        meta.querySelector('.agent-chip').textContent = a.agent_id;
        meta.querySelector('.effect-chip').textContent = a.effect;
        meta.querySelector('.appr-when').textContent = expiresIn(a.expires_at);
        const sum = document.createElement('div');
        sum.className = 'appr-sum';
        sum.textContent = a.summary || `${a.agent_id} wants to run ${a.tool}`;
        const btns = document.createElement('div');
        btns.className = 'appr-btns';
        const mk = (label, cls, fn) => {
          const b = document.createElement('button');
          b.className = 'btn ' + cls;
          b.textContent = label;
          b.addEventListener('click', fn);
          btns.appendChild(b);
          return b;
        };
        mk('Approve', 'primary approve', () => act(a, 'approve'));
        mk('Edit', '', () => edit(a));
        mk('Reject', '', () => act(a, 'reject'));
        row.appendChild(meta);
        row.appendChild(sum);
        row.appendChild(btns);
        list.appendChild(row);
      });
    }

    async function load() {
      try {
        await window.__ZAVORA_LIVE__?.ensureSession?.();
        const res = await fetch(`/api/actions?status=pending${qs()}`, { credentials: 'include' });
        if (res.status === 401 || res.status === 403) {
          signedOut = true;
          items = [];
        } else if (res.ok) {
          signedOut = false;
          const data = await res.json();
          items = data.actions || [];
        } else {
          return; // transient error: keep the previous view
        }
        setBadge(items.length);
        render();
      } catch (_) {
        /* offline: keep previous view */
      }
    }

    async function act(a, verb) {
      try {
        const res = await fetch(`/api/actions/${a.id}/${verb}`, {
          method: 'POST',
          credentials: 'include',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ session_id: sid() }),
        });
        if (!res.ok) {
          toast(`Could not ${verb} — ${res.status}`);
          return;
        }
        const updated = await res.json();
        toast(
          updated.status === 'approved'
            ? `Approved — ${a.summary}`
            : updated.status === 'failed'
              ? `Failed — ${a.summary}`
              : `Rejected — ${a.summary}`
        );
      } catch (err) {
        toast(`Could not ${verb} — ${err.message || 'try again'}`);
      }
      load();
    }

    async function edit(a) {
      const current = JSON.stringify(a.args ?? {}, null, 2);
      const next = window.prompt(`Edit arguments for ${a.tool} (JSON):`, current);
      if (next == null) return;
      let args;
      try {
        args = JSON.parse(next);
      } catch (_) {
        toast('Edit cancelled — that was not valid JSON.');
        return;
      }
      try {
        const res = await fetch(`/api/actions/${a.id}/edit`, {
          method: 'POST',
          credentials: 'include',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ session_id: sid(), args }),
        });
        toast(res.ok ? `Updated — ${a.summary}` : `Could not edit — ${res.status}`);
      } catch (err) {
        toast(`Could not edit — ${err.message || 'try again'}`);
      }
      load();
    }

    async function approveAll() {
      const ids = items.map((a) => a.id);
      if (!ids.length) return;
      try {
        const res = await fetch('/api/actions/approve', {
          method: 'POST',
          credentials: 'include',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ ids, session_id: sid() }),
        });
        if (res.ok) {
          const data = await res.json();
          toast(`Approved ${(data.approved || []).length} action(s)`);
        } else {
          toast(`Batch approve failed — ${res.status}`);
        }
      } catch (err) {
        toast(`Batch approve failed — ${err.message || 'try again'}`);
      }
      load();
    }

    function open() {
      document.getElementById('chatPanel')?.setAttribute('hidden', '');
      panel.removeAttribute('hidden');
      load();
    }
    function close() {
      panel.setAttribute('hidden', '');
    }
    openBtn.addEventListener('click', () => (panel.hidden ? open() : close()));
    if (closeBtn) closeBtn.addEventListener('click', close);
    allBtn.addEventListener('click', approveAll);

    window.addEventListener('zavora:field-event', (e) => {
      const ev = e.detail || {};
      if (ev.type === 'permission_request') {
        toast(`Needs your approval: ${ev.summary}`);
        load();
      } else if (ev.type === 'action_result') {
        load();
      }
    });

    openBtn.removeAttribute('hidden');
    // First load after field-client has had a chance to restore the session.
    setTimeout(load, 1500);
  }

  boot().catch((err) => console.warn('[zavora] approvals boot failed', err));
})();
