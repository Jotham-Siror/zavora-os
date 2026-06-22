/**
 * Gemini Live voice bridge — WS /ws/voice (mia pattern).
 * Falls back to prerecorded clips + SpeechRecognition when unavailable.
 */
(function () {
  'use strict';

  const INPUT_RATE = 16000;
  const OUTPUT_RATE = 24000;

  let enabled = false;
  let active = false;
  let ws = null;
  let mediaStream = null;
  let captureCtx = null;
  let playbackCtx = null;
  let processor = null;
  let sessionId = null;
  let onTranscript = null;

  function wsUrl() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const q = sessionId ? `?session_id=${encodeURIComponent(sessionId)}` : '';
    return `${proto}//${location.host}/ws/voice${q}`;
  }

  function playPcm(buffer) {
    playbackCtx = playbackCtx || new AudioContext({ sampleRate: OUTPUT_RATE });
    if (playbackCtx.state === 'suspended') playbackCtx.resume();
    const pcm16 = new Int16Array(buffer);
    const float32 = new Float32Array(pcm16.length);
    for (let i = 0; i < pcm16.length; i++) float32[i] = pcm16[i] / 32768;
    const audioBuffer = playbackCtx.createBuffer(1, float32.length, OUTPUT_RATE);
    audioBuffer.getChannelData(0).set(float32);
    const source = playbackCtx.createBufferSource();
    source.buffer = audioBuffer;
    source.connect(playbackCtx.destination);
    source.start();
    if (typeof window.drive === 'function' && typeof window.voiceTarget === 'function') {
      try {
        const el = document.createElement('audio');
        window.drive(el);
      } catch (_) {}
    }
  }

  async function startCapture() {
    mediaStream = await navigator.mediaDevices.getUserMedia({
      audio: {
        sampleRate: INPUT_RATE,
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
      },
    });
    captureCtx = new AudioContext({ sampleRate: INPUT_RATE });
    const source = captureCtx.createMediaStreamSource(mediaStream);
    processor = captureCtx.createScriptProcessor(4096, 1, 1);
    processor.onaudioprocess = (e) => {
      if (!active || !ws || ws.readyState !== WebSocket.OPEN) return;
      const input = e.inputBuffer.getChannelData(0);
      const pcm16 = new Int16Array(input.length);
      for (let i = 0; i < input.length; i++) {
        const s = Math.max(-1, Math.min(1, input[i]));
        pcm16[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
      }
      ws.send(pcm16.buffer);
    };
    source.connect(processor);
    processor.connect(captureCtx.destination);
  }

  function stopCapture() {
    if (processor) {
      processor.disconnect();
      processor = null;
    }
    if (captureCtx) {
      captureCtx.close().catch(() => {});
      captureCtx = null;
    }
    if (mediaStream) {
      mediaStream.getTracks().forEach((t) => t.stop());
      mediaStream = null;
    }
  }

  function handleMessage(ev) {
    if (typeof ev.data === 'string') {
      let msg;
      try {
        msg = JSON.parse(ev.data);
      } catch (_) {
        return;
      }
      if (msg.type === 'transcript' && msg.content && onTranscript) {
        onTranscript(msg.content);
      }
      if (msg.type === 'error') {
        console.warn('live voice:', msg.message);
      }
      return;
    }
    if (ev.data instanceof ArrayBuffer) {
      playPcm(ev.data);
    } else if (ev.data instanceof Blob) {
      ev.data.arrayBuffer().then(playPcm);
    }
  }

  async function start(opts) {
    if (!enabled || active) return false;
    sessionId = opts?.sessionId || null;
    onTranscript = opts?.onTranscript || null;

    return new Promise((resolve) => {
      ws = new WebSocket(wsUrl());
      ws.binaryType = 'arraybuffer';

      const fail = () => {
        stop();
        resolve(false);
      };

      ws.onerror = fail;
      ws.onclose = () => {
        if (active) stop();
      };
      ws.onmessage = handleMessage;
      ws.onopen = async () => {
        try {
          await startCapture();
          active = true;
          resolve(true);
        } catch (e) {
          console.warn('live voice capture failed:', e);
          fail();
        }
      };

      setTimeout(() => {
        if (!active) fail();
      }, 8000);
    });
  }

  function stop() {
    active = false;
    stopCapture();
    if (ws) {
      try {
        ws.close();
      } catch (_) {}
      ws = null;
    }
  }

  async function speakText(text) {
    if (!enabled) return false;
    const ok = await start({});
    if (!ok || !ws) return false;
    ws.send(JSON.stringify({ type: 'text', content: text }));
    return true;
  }

  async function probe() {
    try {
      const res = await fetch('/api/voice/status');
      if (!res.ok) return false;
      const data = await res.json();
      enabled = !!data.enabled;
      return enabled;
    } catch (_) {
      enabled = false;
      return false;
    }
  }

  window.ZavoraLiveVoice = {
    probe,
    start,
    stop,
    speakText,
    isEnabled: () => enabled,
    isActive: () => active,
  };

  probe();
})();