#!/usr/bin/env python3
"""One-shot: generate voice clips for the Field prototype, then never call TTS again.
Reads GEMINI_API_KEY from ../adk-rust/.env. Saves audio/<id>.wav (24kHz PCM wrapped as WAV)."""
import base64, json, os, re, struct, urllib.request, pathlib

KEY = None
for line in pathlib.Path("../adk-rust/.env").read_text().splitlines():
    if line.startswith("GEMINI_API_KEY="):
        KEY = line.split("=", 1)[1].strip().strip('"').strip("'")
assert KEY, "GEMINI_API_KEY not found"

MODEL = "gemini-2.5-flash-preview-tts"
VOICE = "Aoede"
PERSONA = ("You are the voice of a personal operating system: warm, confident, and quietly witty — "
    "like a sharp friend who's genuinely glad to help and a little proud of what they just pulled off. "
    "Delivery: relaxed, natural pacing with a hint of a smile, gentle rises on the good news, "
    "a soft knowing beat before the suggestion. Not robotic, not bubbly — effortlessly cool.\n"
    'Read ONLY this line aloud, in character:\n"{}"')

CLIPS = {
  "greeting": "Good morning. Here's your day — three meetings, two emails that need you, and a free window at noon.",
  "morning":  "Good morning. You have 3 meetings, a free window 12 to 2pm, and 2 emails that actually need you. Everything else I've handled.",
  "lisbon":   "You can be in Lisbon Friday night for $284, staying at a riverside Alfama loft for $96 a night. I drafted a 3-day plan — just say the word and I'll hold both.",
  "week":     "This week you spent $1,240 — mostly travel and groceries — slept 6.1 hours a night, and shipped 14 commits. My one suggestion: protect Tuesday mornings.",
  "deck":     "Your numbers, story and slides are ready. Say combine and I'll merge them into one finished deck.",
  "deck_done":"Done. I combined your numbers and story into one finished deck — ten slides, ready to present.",
  "fuse":     "Done. I've combined those for you.",
  "people":   "Three people are waiting on you — Alex, Priya and the dev team. I've drafted replies and prepped your 3pm one-on-one.",
  "live":     "Here's what's happening — rates paused, chips rallying, your portfolio up 0.8 percent, and a keynote is live now.",
  "proactive":"While you were away I did a few things — researched ABC Corp, caught a 12 percent price drop, and made a couple of things you might like.",
}

def wav(pcm, rate=24000):
    return (b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVE" + b"fmt " +
            struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16) +
            b"data" + struct.pack("<I", len(pcm)) + pcm)

def gen(text):
    body = json.dumps({
        "contents": [{"parts": [{"text": PERSONA.format(text)}]}],
        "generationConfig": {"responseModalities": ["AUDIO"],
            "speechConfig": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": VOICE}}}},
    }).encode()
    url = f"https://generativelanguage.googleapis.com/v1beta/models/{MODEL}:generateContent?key={KEY}"
    req = urllib.request.Request(url, body, {"Content-Type": "application/json"})
    data = json.load(urllib.request.urlopen(req, timeout=60))
    inline = data["candidates"][0]["content"]["parts"][0]["inlineData"]
    rate = int((re.search(r"rate=(\d+)", inline["mimeType"]) or [None, "24000"])[1])
    return wav(base64.b64decode(inline["data"]), rate)

out = pathlib.Path("audio"); out.mkdir(exist_ok=True)
for cid, text in CLIPS.items():
    f = out / f"{cid}.wav"
    if f.exists(): print("· skip", cid); continue
    f.write_bytes(gen(text))
    print("✓", cid)
print("done")
