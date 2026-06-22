/**
 * Live mode: consume POST /api/sessions/{id}/intent SSE and drive the field UI.
 * Offline demo uses legacy scenarios{} when ?demo=1 or localStorage zavora_demo=1.
 */
(function () {
  'use strict';

  const ui = window.__ZAVORA_UI__;
  if (!ui || window.__ZAVORA_DEMO__) return;

  let sessionId = null;
  let abortController = null;
  const cards = new Map();

  async function ensureSession() {
    if (sessionId) return sessionId;
    const res = await fetch('/api/sessions', { method: 'POST' });
    if (!res.ok) throw new Error('session create failed');
    const data = await res.json();
    sessionId = data.session_id;
    return sessionId;
  }

  function beginScenario(key, text, totalCards) {
    ui.setCurrentKey(key);
    ui.setResolvedCount(0);
    ui.setTotalCards(totalCards);
    ui.chips.classList.add('hide');
    ui.suzy.classList.remove('show');
    ui.originText.textContent = text;
    ui.origin.classList.add('show');
    ui.exitFlow();
    ui.cardsEl.innerHTML = '';
    ui.resetAgents();
    cards.clear();
  }

  function spawnCard(index, spec) {
    const card = ui.buildCard(spec);
    ui.cardsEl.appendChild(card.el);
    card.el.animate(
      [
        { opacity: 0, transform: 'translateY(46px) scale(.92)' },
        { opacity: 1, transform: 'translateY(0) scale(1)' },
      ],
      {
        duration: 760,
        delay: index * 130,
        easing: 'cubic-bezier(.22,1,.36,1)',
        fill: 'backwards',
      }
    );
    ui.agentActive(spec);
    card.body.innerHTML = '';
    if (spec.surface) ui.buildSurface(card.body, spec.surface);
    const note = document.createElement('div');
    note.className = 'line sub';
    card.body.appendChild(note);
    cards.set(index, { card, spec, note });
    return card;
  }

  function updateStatus(index, status, line) {
    const entry = cards.get(index);
    if (!entry) return;
    if (line) entry.note.textContent = line;
    entry.card.stText.textContent =
      status === 'composing' ? 'composing…' : status === 'working' ? 'working…' : status;
    entry.card.status.className = 'status' + (status === 'working' ? ' working' : '');
  }

  function handleEvent(ev, intentText) {
    switch (ev.type) {
      case 'scenario':
        beginScenario(ev.key, ev.text || intentText, ev.total_cards || 0);
        break;
      case 'card_spawn':
        spawnCard(ev.index, ev.card);
        break;
      case 'card_status':
        updateStatus(ev.index, ev.status, ev.line);
        break;
      case 'card_surface': {
        const entry = cards.get(ev.index);
        if (!entry || ev.surface !== 'slides') break;
        const work = entry.card.body.querySelector('.work.slides');
        if (!work) break;
        const film = work.querySelector('.film');
        if (!film) break;
        const sls = film.querySelectorAll('.sl');
        const idx = Math.max(0, (ev.slide || 1) - 1);
        if (sls[idx]) {
          sls[idx].classList.add('in');
          sls[idx].textContent = String(ev.slide);
        }
        break;
      }
      case 'card_resolve': {
        const entry = cards.get(ev.index);
        if (!entry) break;
        const merged = Object.assign({}, entry.spec, { resolve: ev.resolve });
        ui.resolve(entry.card, merged, null);
        if (ev.resolve?.artifact_url) {
          const openBtn = entry.card.el.querySelector('.btn.primary');
          if (openBtn) {
            openBtn.addEventListener(
              'click',
              (e) => {
                e.stopPropagation();
                window.open(ev.resolve.artifact_url, '_blank');
              },
              { once: true }
            );
          }
        }
        break;
      }
      case 'error':
        console.warn('[zavora] orchestration error', ev.message);
        break;
      case 'suzy_summary':
        ui.showSuzyCustom(ev.html);
        break;
      case 'done':
        break;
      default:
        break;
    }
  }

  async function submitLive(text) {
    const trimmed = text.trim();
    if (!trimmed) return;

    ui.clearSuggestion();
    ui.input.value = '';

    const hasCards = !!document.querySelector('.card');
    const isAction =
      /\b(do it|combine|merge|fold|book|hold|reply|draft|reserve|apply|arrange|organi[sz]e|sort it|handle it|take care|read aloud|show me|reply to them)\b/i.test(
        trimmed
      );
    if (hasCards && isAction) {
      ui.conductAction();
      return;
    }

    await ensureSession();
    if (abortController) abortController.abort();
    abortController = new AbortController();

    const res = await fetch(`/api/sessions/${sessionId}/intent`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'text/event-stream',
      },
      body: JSON.stringify({ text: trimmed }),
      signal: abortController.signal,
    });
    if (!res.ok) throw new Error(`intent failed: ${res.status}`);

    const reader = res.body.getReader();
    const dec = new TextDecoder();
    let buf = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      const parts = buf.split('\n\n');
      buf = parts.pop() || '';
      for (const part of parts) {
        const line = part.split('\n').find((l) => l.startsWith('data: '));
        if (!line) continue;
        try {
          handleEvent(JSON.parse(line.slice(6)), trimmed);
        } catch (e) {
          console.warn('SSE parse error', e);
        }
      }
    }
  }

  window.__ZAVORA_LIVE__ = {
    submit(text) {
      submitLive(text).catch((err) => {
        if (err.name === 'AbortError') return;
        console.warn('live intent failed, falling back to demo', err);
        ui.legacySubmit(text);
      });
    },
  };

  console.info('[zavora] live mode — SSE orchestration enabled');
})();