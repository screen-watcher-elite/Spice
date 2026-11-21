import base64
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
import wave
import numpy as np

ROOT = r'C:\Users\Ashutosh\Cyberground\spice'
ENV_PATH = os.path.join(ROOT, '.env')
STUDIO_DIR = os.path.join(ROOT, 'demo_studio')
AUDIO_DIR = os.path.join(STUDIO_DIR, 'audio')
TIMELINE_PATH = os.path.join(STUDIO_DIR, 'timeline.json')
os.makedirs(AUDIO_DIR, exist_ok=True)

SR = 48000

def load_api_key():
    with open(ENV_PATH, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if line.startswith('GEMINI_API_KEY='):
                return line.split('=', 1)[1].strip().strip('"').strip("'")
    raise RuntimeError('GEMINI_API_KEY missing from spice/.env')

def wav_duration(wav_path: str) -> float:
    with wave.open(wav_path, 'rb') as wf:
        return wf.getnframes() / float(wf.getframerate())

DEFAULT_VOICE_38 = 'en-us-podcaster-7'
DEFAULT_VOICE_31 = 'Achird'
DEFAULT_VOICE_EDGE = 'en-US-AndrewMultilingualNeural'
EXHAUSTED_MODELS = set()

AF_CONDENSER_CHAIN = (
    'silenceremove=start_periods=1:start_silence=0.06:start_threshold=-52dB,'
    'areverse,silenceremove=start_periods=1:start_silence=0.06:start_threshold=-52dB,areverse,'
    'highpass=f=65,'
    'equalizer=f=150:t=q:w=1.2:g=1.4,'
    'equalizer=f=6500:t=q:w=1.8:g=-1.2,'
    'acompressor=threshold=-20dB:ratio=1.8:attack=18:release=180:makeup=1.0'
)

def master_vocal(in_path: str, out_wav: str):
    """Apply warm condenser mastering to a raw voice track without crushing micro-dynamics."""
    subprocess.run(
        ['ffmpeg', '-y', '-i', in_path, '-af', AF_CONDENSER_CHAIN, '-ar', str(SR), '-ac', '2', out_wav],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

def write_gemini_audio(inline: dict, raw_wav: str):
    """Decode base64 WAV or 24kHz mono L16 PCM from a Gemini TTS payload into raw_wav."""
    mime = (inline.get('mime_type') or inline.get('mimeType') or 'audio/wav').lower()
    raw_bytes = base64.b64decode(inline['data'])
    if 'wav' in mime or raw_bytes[:4] == b'RIFF':
        with open(raw_wav, 'wb') as f:
            f.write(raw_bytes)
    else:
        with wave.open(raw_wav, 'wb') as wf:
            wf.setnchannels(1)
            wf.setsampwidth(2)
            wf.setframerate(24000)
            wf.writeframes(raw_bytes)

def handle_http_429(model_name: str, err_body: str) -> bool:
    """Return True if RPM transient (slept and worth retrying), False if daily quota exhausted."""
    low = err_body.lower()
    if 'per day' in low or 'requests per day' in low:
        EXHAUSTED_MODELS.add(model_name)
        print(f'[QUOTA RPD] {model_name} hit daily limit; skipping for remainder of run.')
        return False
    if 'per minute' in low or 'rpm' in low or 'rate limit' in low:
        print(f'[QUOTA RPM] {model_name} rate-limited; sleeping 22s before retry...')
        time.sleep(22.0)
        return True
    EXHAUSTED_MODELS.add(model_name)
    return False

def format_31_prompt(text: str, style: str) -> str:
    """Format transcript with Audio Profile & Director Notes for Gemini 3.1 TTS."""
    t31 = text.replace('<short pause>', '[short pause]').replace('<long pause>', '[long pause]')
    s = style or 'warm, natural conversational Demo Day tone'
    return (
        '# AUDIO PROFILE: Founder Narrator\n'
        'Warm, friendly, natural tech founder in his early 30s speaking into a close studio condenser '
        'microphone — relaxed human cadence, genuine conversational inflection, subtle breath, never booming or robotic.\n\n'
        '### DIRECTOR\'S NOTES\n'
        f'Style: {s}\n'
        'Pacing: Natural conversational Demo Day rhythm with relaxed pauses at punctuation and [short pause] cues.\n\n'
        '#### TRANSCRIPT\n'
        f'{t31}'
    )

def synthesize_tts_38(api_key: str, text: str, out_wav: str, voice: str = DEFAULT_VOICE_38, style: str = ''):
    style_val = style or 'warm, conversational, natural human startup founder explaining with genuine clarity'
    base_payload = {
        'store': False,
        'input': [{
            'type': 'user_input',
            'content': [{
                'type': 'text',
                'text': text,
                'annotations': [{'type': 'speech_metadata', 'style': style_val}],
            }],
        }],
        'response_format': {'type': 'audio'},
        'generation_config': {'speech_config': [{'voice': voice}]},
    }
    raw_wav = out_wav.replace('.wav', '_raw.wav')
    for m in ('gemini-3.8-flash-tts', 'gemini-3.8-flash-lite-tts'):
        if m in EXHAUSTED_MODELS:
            continue
        req_payload = dict(base_payload, model=m)
        for attempt in range(3):
            req = urllib.request.Request(
                'https://generativelanguage.googleapis.com/v1beta/interactions',
                data=json.dumps(req_payload).encode('utf-8'),
                headers={'x-goog-api-key': api_key, 'Content-Type': 'application/json'},
                method='POST',
            )
            try:
                with urllib.request.urlopen(req, timeout=60) as resp:
                    body = json.loads(resp.read().decode('utf-8'))
                audio_blocks = [
                    c for s in body.get('steps', []) if s.get('type') == 'model_output'
                    for c in s.get('content', []) if c.get('type') == 'audio'
                ]
                if not audio_blocks:
                    raise RuntimeError(f'No audio returned for {m}')
                write_gemini_audio(audio_blocks[-1], raw_wav)
                master_vocal(raw_wav, out_wav)
                os.remove(raw_wav)
                dur = wav_duration(out_wav)
                print(f'[TTS OK] {os.path.basename(out_wav)} via {m} ({voice}): {dur:.2f}s')
                return dur
            except urllib.error.HTTPError as e:
                err_body = e.read().decode('utf-8', errors='replace')[:400]
                print(f'[TTS RETRY] {m} attempt {attempt+1}: HTTP {e.code} {err_body}')
                if e.code == 429 and not handle_http_429(m, err_body):
                    break
                if e.code != 429:
                    time.sleep(2.5)
            except Exception as e:
                print(f'[TTS RETRY] {m} attempt {attempt+1}: {e}')
                time.sleep(2.5)
    raise RuntimeError(f'3.8 TTS exhausted for {out_wav}')

def synthesize_tts_31(api_key: str, text: str, out_wav: str, voice: str = DEFAULT_VOICE_31, style: str = ''):
    m = 'gemini-3.1-flash-tts-preview'
    if m in EXHAUSTED_MODELS:
        raise RuntimeError(f'{m} exhausted')
    payload = {
        'contents': [{'role': 'user', 'parts': [{'text': format_31_prompt(text, style)}]}],
        'generationConfig': {
            'responseModalities': ['AUDIO'],
            'speechConfig': {'voiceConfig': {'prebuiltVoiceConfig': {'voiceName': voice}}},
        },
    }
    raw_wav = out_wav.replace('.wav', '_raw.wav')
    url = f'https://generativelanguage.googleapis.com/v1beta/models/{m}:generateContent'
    for attempt in range(3):
        req = urllib.request.Request(
            url,
            data=json.dumps(payload).encode('utf-8'),
            headers={'x-goog-api-key': api_key, 'Content-Type': 'application/json'},
            method='POST',
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                body = json.loads(resp.read().decode('utf-8'))
            parts = body['candidates'][0]['content']['parts']
            inline = next(p['inlineData'] for p in parts if 'inlineData' in p)
            write_gemini_audio(inline, raw_wav)
            master_vocal(raw_wav, out_wav)
            os.remove(raw_wav)
            dur = wav_duration(out_wav)
            print(f'[TTS OK] {os.path.basename(out_wav)} via {m} ({voice}): {dur:.2f}s')
            return dur
        except urllib.error.HTTPError as e:
            err_body = e.read().decode('utf-8', errors='replace')[:400]
            print(f'[TTS RETRY] {m} attempt {attempt+1}: HTTP {e.code} {err_body}')
            if e.code == 429 and not handle_http_429(m, err_body):
                break
            if e.code != 429:
                time.sleep(2.5)
        except Exception as e:
            print(f'[TTS RETRY] {m} attempt {attempt+1}: {e}')
            time.sleep(2.5)
    raise RuntimeError(f'{m} exhausted for {out_wav}')

def synthesize_tts_edge(text: str, out_wav: str, voice: str = DEFAULT_VOICE_EDGE):
    import asyncio
    try:
        import edge_tts
    except ImportError:
        subprocess.run([sys.executable, '-m', 'pip', 'install', '-q', 'edge-tts'], check=True)
        import edge_tts
    clean = text.replace(' <short pause>', ', ...').replace('<short pause>', '...').replace('<long pause>', '...')
    raw_mp3 = out_wav.replace('.wav', '_raw.mp3')
    asyncio.run(edge_tts.Communicate(clean, voice).save(raw_mp3))
    master_vocal(raw_mp3, out_wav)
    os.remove(raw_mp3)
    dur = wav_duration(out_wav)
    print(f'[TTS OK] {os.path.basename(out_wav)} via edge-tts ({voice}): {dur:.2f}s')
    return dur

ACTS_SPEC = [
    {
        'id': 'a0_problem',
        'title': '01 // THE FINTECH AI COMPLIANCE CRISIS',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'style': 'warm, grounded, slightly urgent conversational documentary tone',
        'text': (
            'Indian fintechs want autonomous AI resolving customer refunds in seconds. <short pause> '
            'The catch? One clever prompt injection drains twenty-five thousand rupees, raw Aadhaar and PAN details '
            'leak under two-hundred-fifty-crore DPDP penalties, <short pause> and bots blow past RBI five-thousand-rupee caps. '
            'Without deterministic rails, banking AI simply isn\'t safe to ship.'
        ),
    },
    {
        'id': 'a1_solution',
        'title': '02 // MEET AEGISMESH · SIX LAYERS OF RUST',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'style': 'confident, clear, inspiring startup founder introducing a product',
        'text': (
            'Meet AegisMesh — a six-layer Rust firewall built specifically for Google Gemini agents. <short pause> '
            'Every prompt passes through six zero-trust defense layers: regex normalization, a DPDP PII vault, '
            'Gemini guardrails, RBI circuit breakers, egress scrubbing, and an asynchronous sentinel.'
        ),
    },
    {
        'id': 'a2_live_attack',
        'title': '03 // LIVE DEMO · TYPED PROMPT INJECTION BLOCKED',
        'pre_pad': 0.20,
        'post_pad': 0.50,
        'style': 'focused, engaging live product walkthrough, crisp energy',
        'text': (
            'Let\'s watch a live attack fail in real time. <short pause> '
            'An attacker types an urgent override demanding twenty-five thousand rupees and slips in a customer PAN. '
            'One click — <short pause> Layer One shuts down the jailbreak in thirty-seven microseconds, '
            'Layer Two vaults the PAN, and our Sandbox X-Ray stops Base64 XSS cold.'
        ),
    },
    {
        'id': 'a3_gemini_innovation',
        'title': '04 // DPDP VAULT & GEMINI DECISION ONE',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'style': 'thoughtful, articulate engineering explanation with natural emphasis',
        'text': (
            'Of course, subtle social engineering doesn\'t use obvious keywords. <short pause> '
            'So first, our DPDP vault laser-scrubs Aadhaar, PAN, and phone numbers — zero plaintext ever reaches upstream LLMs. <short pause> '
            'Then zero-token Google Gemini Decision One evaluates the semantics cleanly, '
            'flagging hidden RBI coercion at zero point nine seven one.'
        ),
    },
    {
        'id': 'a4_maker_checker',
        'title': '05 // RBI CIRCUIT BREAKER & MAKER-CHECKER',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'style': 'calm, reassuring, practical fintech operator storytelling',
        'text': (
            'Safe banking also means handling real customer disputes smoothly. <short pause> '
            'When Kavita Roy asks for a legitimate twenty-five-thousand-rupee chargeback, '
            'Layer Four trips the five-thousand-rupee RBI breaker right on threshold. <short pause> '
            'A compliance officer inspects the six-layer trace, approves settlement in one click, and steps into the live conversation.'
        ),
    },
    {
        'id': 'a5_gemini_orchestra',
        'title': '06 // THREE ROLE-TUNED GEMINI MODELS',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'style': 'energetic, crisp architectural storytelling, conversational',
        'text': (
            'Under the hood, AegisMesh orchestrates three specialized Gemini models: '
            'Flash-Lite reasons quickly for customers, Decision One guards every single turn inline, <short pause> '
            'and Flash-Preview audits multi-turn drift asynchronously over Tokio channels — '
            'giving compliance teams live policy control with zero delay.'
        ),
    },
    {
        'id': 'a6_impact_brand',
        'title': '07 // PROVEN RUST SPEED & AUDIT TRUST',
        'pre_pad': 0.20,
        'post_pad': 1.85,
        'style': 'warm, proud, grounded closing authority with a subtle smile in the voice',
        'text': (
            'Compared to four-hundred-millisecond Python proxies, AegisMesh secures every prompt in thirty-seven microseconds — '
            'over ten thousand times faster at seventy-five thousand operations per second. <short pause> '
            'Every decision stamps an immutable RBI and DPDP audit trail. '
            'AegisMesh: safe autonomous AI for Indian fintech.'
        ),
    },
]

def load_wav_stereo(path):
    with wave.open(path, 'rb') as wf:
        frames = wf.readframes(wf.getnframes())
    arr = np.frombuffer(frames, dtype=np.int16).astype(np.float32) / 32768.0
    return arr.reshape(-1, 2)

def save_wav_stereo(path, arr):
    clipped = np.clip(arr, -0.98, 0.98)
    pcm = (clipped * 32767.0).astype(np.int16)
    with wave.open(path, 'wb') as wf:
        wf.setnchannels(2)
        wf.setsampwidth(2)
        wf.setframerate(SR)
        wf.writeframes(pcm.tobytes())

def add_sfx(buf, t_sec, sig):
    idx = int(t_sec * SR)
    if idx < 0 or idx >= len(buf):
        return
    n = min(len(sig), len(buf) - idx)
    buf[idx : idx + n] += sig[:n]

def synth_thud():
    dur = 0.34
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freq = 110.0 * np.exp(-t * 13.0) + 38.0
    phase = 2.0 * np.pi * np.cumsum(freq) / SR
    sub = np.sin(phase) * np.exp(-t * 9.5) * 0.65
    punch = np.sin(2.0 * np.pi * 195.0 * t) * np.exp(-t * 52.0) * 0.32
    mono = sub + punch
    return np.column_stack([mono, mono])

def synth_stamp_thud():
    dur = 0.48
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freq = 104.0 * np.exp(-t * 15.0) + 32.0
    phase = 2.0 * np.pi * np.cumsum(freq) / SR
    sub = np.sin(phase) * np.exp(-t * 6.5) * 1.15
    rng = np.random.default_rng(19)
    # Sharp mechanical printer latch transient (3.5 kHz bandpass click + 850 Hz thock)
    hp_click = np.diff(rng.standard_normal(len(t)), prepend=0.0) * np.exp(-t * 110.0) * 0.56
    latch = np.sin(2.0 * np.pi * 880.0 * np.exp(-t * 18.0) * t) * np.exp(-t * 48.0) * 0.42
    seal = np.sin(2.0 * np.pi * 260.0 * t) * np.exp(-t * 30.0) * 0.48
    mono = np.tanh((sub + hp_click + latch + seal) * 1.25) * 0.96
    return np.column_stack([mono, np.roll(mono, 18)])

def synth_whoosh():
    dur = 0.28
    n = int(dur * SR)
    t = np.linspace(0, dur, n, endpoint=False)
    rng = np.random.default_rng(7)
    noise = rng.standard_normal(n)
    env = np.sin(np.pi * (t / dur)) ** 1.4
    kernel = np.hanning(29)
    kernel /= kernel.sum()
    smooth = np.convolve(noise, kernel, mode='same')
    carrier = np.sin(2 * np.pi * (240 + 520 * (t / dur)) * t) * 0.12
    l = (smooth * 0.30 + carrier) * env
    r = (np.roll(smooth, 72) * 0.30 + carrier) * env
    return np.column_stack([l, r])

def synth_riser():
    dur = 0.45
    n = int(dur * SR)
    t = np.linspace(0, dur, n, endpoint=False)
    freq = 160.0 + 620.0 * (t / dur) ** 2.2
    ph = 2.0 * np.pi * np.cumsum(freq) / SR
    env = (t / dur) ** 1.8 * 0.16
    mono = np.sin(ph) * env
    return np.column_stack([mono, np.roll(mono, 40)])

def synth_pop_click():
    dur = 0.045
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freq = 920.0 * np.exp(-t * 48.0) + 310.0
    phase = 2.0 * np.pi * np.cumsum(freq) / SR
    mono = np.sin(phase) * np.exp(-t * 74.0) * 0.46
    return np.column_stack([mono, mono])

def synth_key_tap():
    dur = 0.026
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    rng = np.random.default_rng(91)
    transient = np.diff(rng.standard_normal(len(t)), prepend=0.0) * np.exp(-t * 210.0) * 0.24
    thock = np.sin(2 * np.pi * 440.0 * np.exp(-t * 35.0) * t) * np.exp(-t * 140.0) * 0.18
    mono = transient + thock
    return np.column_stack([mono, np.roll(mono, 8)])

def synth_chime():
    dur = 0.65
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freqs = [554.37, 659.25, 830.61, 1108.73]
    sig = sum(np.sin(2 * np.pi * f * t) * np.exp(-t * (4.0 + 0.4 * i)) for i, f in enumerate(freqs))
    sig *= (1.0 - np.exp(-t * 160.0)) * 0.14
    return np.column_stack([sig, np.roll(sig, 140)])

def synth_paper_slide():
    dur = 0.85
    n = int(dur * SR)
    t = np.linspace(0, dur, n, endpoint=False)
    rng = np.random.default_rng(42)
    noise = rng.standard_normal(n)
    hp = np.diff(noise, prepend=0.0)
    env = np.sin(np.pi * (t / dur)) ** 0.85
    pulses = (0.5 + 0.5 * np.sin(2 * np.pi * 26.0 * t)) * 0.55 + 0.45
    mono = hp * env * pulses * 0.22
    return np.column_stack([mono, np.roll(mono, 42)])

def synth_paper_lock():
    dur = 0.16
    n = int(dur * SR)
    t = np.linspace(0, dur, n, endpoint=False)
    rng = np.random.default_rng(77)
    tear = np.diff(rng.standard_normal(n), prepend=0.0) * np.exp(-t * 36.0) * 0.34
    snap = np.sin(2 * np.pi * 320.0 * np.exp(-t * 24.0) * t) * np.exp(-t * 42.0) * 0.25
    return np.column_stack([tear + snap, np.roll(tear + snap, 14)])

def synth_music(total_dur: float):
    # Subtle, warm 122 BPM architectural electronic underscore (supports voice without clashing)
    n = int(total_dur * SR)
    t = np.linspace(0, total_dur, n, endpoint=False)
    bpm = 122.0
    beat_dur = 60.0 / bpm
    buf = np.zeros((n, 2), dtype=np.float32)
    rng = np.random.default_rng(108)

    chords = [
        [185.00, 220.00, 277.18, 329.63],
        [146.83, 220.00, 277.18, 369.99],
        [220.00, 277.18, 329.63, 493.88],
        [164.81, 207.65, 246.94, 329.63],
    ]
    basses = [92.50, 73.42, 110.00, 82.41]

    bar_idx = np.floor(t / (4.0 * beat_dur)).astype(int) % 4
    pad = np.zeros(n, dtype=np.float32)
    for ci, freqs in enumerate(chords):
        mask = (bar_idx == ci)
        local_t = t[mask]
        val = sum(np.sin(2 * np.pi * f * local_t) for f in freqs) / len(freqs)
        pad[mask] = val
    buf[:, 0] += pad * 0.095
    buf[:, 1] += np.roll(pad, 320) * 0.095

    total_beats = int(total_dur / beat_dur)
    for b in range(total_beats):
        bt = b * beat_dur
        idx = int(bt * SR)
        kd = 0.18
        kt = np.linspace(0, kd, int(kd * SR), endpoint=False)
        kf = 112.0 * np.exp(-kt * 25.0) + 46.0
        kph = 2 * np.pi * np.cumsum(kf) / SR
        kick = np.sin(kph) * np.exp(-kt * 18.0) * 0.22
        kn = min(len(kick), n - idx)
        buf[idx : idx + kn, 0] += kick[:kn]
        buf[idx : idx + kn, 1] += kick[:kn]

        bass_f = basses[(b // 4) % 4]
        for sub in (0.25, 0.5, 0.75):
            s_idx = int((bt + sub * beat_dur) * SR)
            bd = 0.11
            btt = np.linspace(0, bd, int(bd * SR), endpoint=False)
            bs = np.sin(2 * np.pi * bass_f * btt) * np.exp(-btt * 19.0) * 0.11
            bn = min(len(bs), n - s_idx)
            if bn > 0:
                buf[s_idx : s_idx + bn, 0] += bs[:bn]
                buf[s_idx : s_idx + bn, 1] += bs[:bn]

        for eighth in (0.0, 0.5):
            h_idx = int((bt + eighth * beat_dur) * SR)
            hd = 0.022
            hn = min(int(hd * SR), n - h_idx)
            if hn > 2:
                ht = np.linspace(0, hd, hn, endpoint=False)
                hat = np.diff(rng.standard_normal(hn), prepend=0.0) * np.exp(-ht * 125.0) * (0.035 if eighth else 0.020)
                buf[h_idx : h_idx + hn, 0] += hat
                buf[h_idx : h_idx + hn, 1] += hat * 0.85

        chord = chords[(b // 4) % 4]
        for step in range(2):
            p_idx = int((bt + step * 0.5 * beat_dur) * SR)
            freq = chord[(b * 2 + step) % len(chord)] * 2.0
            pd = 0.15
            pn = min(int(pd * SR), n - p_idx)
            if pn > 0:
                pt = np.linspace(0, pd, pn, endpoint=False)
                pluck = np.sin(2 * np.pi * freq * pt) * np.exp(-pt * 19.0) * 0.058
                pan = 0.70 if ((b + step) % 2 == 0) else 0.30
                buf[p_idx : p_idx + pn, 0] += pluck * pan
                buf[p_idx : p_idx + pn, 1] += pluck * (1.0 - pan)

    fade_in = int(0.5 * SR)
    fade_out = int(1.6 * SR)
    if len(buf) > fade_in:
        buf[:fade_in] *= np.linspace(0.0, 1.0, fade_in)[:, None]
    if len(buf) > fade_out:
        buf[-fade_out:] *= np.linspace(1.0, 0.0, fade_out)[:, None]
    return buf

def generate_all_vo_uniform(api_key: str, prefer_31: bool = False):
    """Synthesize all 7 acts using a single consistent speaker tier (no mid-video voice mismatch)."""
    tiers = ['3.1', 'edge'] if prefer_31 else ['3.8', '3.1', 'edge']
    last_err = None
    for tier in tiers:
        print(f'[TTS TIER] Generating uniform 7-act narration via tier={tier}...')
        durs = []
        try:
            for i, spec in enumerate(ACTS_SPEC):
                wav_path = os.path.join(AUDIO_DIR, f'buildathon_vo_{i}_{spec["id"]}.wav')
                if i > 0:
                    time.sleep(2.5)
                if tier == '3.8':
                    d = synthesize_tts_38(api_key, spec['text'], wav_path, voice=DEFAULT_VOICE_38, style=spec.get('style', ''))
                elif tier == '3.1':
                    d = synthesize_tts_31(api_key, spec['text'], wav_path, voice=DEFAULT_VOICE_31, style=spec.get('style', ''))
                else:
                    d = synthesize_tts_edge(spec['text'], wav_path, voice=DEFAULT_VOICE_EDGE)
                durs.append(d)
            return durs
        except Exception as e:
            last_err = e
            print(f'[TTS TIER FALLBACK] tier={tier} failed ({e}); restarting all 7 acts on next uniform speaker tier.')
    raise RuntimeError(f'All uniform TTS tiers failed: {last_err}')

def main():
    regen_vo = '--regen-vo' in sys.argv
    prefer_31 = '--prefer-31' in sys.argv
    api_key = load_api_key()

    if regen_vo:
        vo_durs = generate_all_vo_uniform(api_key, prefer_31=prefer_31)
    else:
        vo_durs = []
        for i, spec in enumerate(ACTS_SPEC):
            wav_path = os.path.join(AUDIO_DIR, f'buildathon_vo_{i}_{spec["id"]}.wav')
            if not (os.path.exists(wav_path) and os.path.getsize(wav_path) > 1000):
                vo_durs = generate_all_vo_uniform(api_key, prefer_31=prefer_31)
                break
            d = wav_duration(wav_path)
            print(f'[TTS REUSE] {os.path.basename(wav_path)}: {d:.2f}s')
            vo_durs.append(d)

    acts_out = []
    vo_files = []
    cursor_t = 0.0

    for i, (spec, vo_dur) in enumerate(zip(ACTS_SPEC, vo_durs)):
        wav_path = os.path.join(AUDIO_DIR, f'buildathon_vo_{i}_{spec["id"]}.wav')
        start_t = round(cursor_t, 3)
        vo_start = round(start_t + spec['pre_pad'], 3)
        vo_end = round(vo_start + vo_dur, 3)
        end_t = round(vo_end + spec['post_pad'], 3)
        dur = round(end_t - start_t, 3)
        acts_out.append({
            'id': spec['id'],
            'title': spec['title'],
            'start': start_t,
            'end': end_t,
            'vo_start': vo_start,
            'vo_end': vo_end,
            'duration': dur,
        })
        vo_files.append((vo_start, wav_path))
        cursor_t = end_t

    total_duration = round(cursor_t, 3)
    print(f'[TIMELINE] Buildathon Evaluator Cut = {total_duration:.2f}s ({len(acts_out)} acts)')
    if total_duration >= 178.0:
        raise RuntimeError(f'Duration {total_duration:.2f}s exceeds 178s safety budget under 3:00 cap!')

    n_samples = int(np.ceil(total_duration * SR))
    vo_buf = np.zeros((n_samples, 2), dtype=np.float32)
    for vs, wf_path in vo_files:
        seg = load_wav_stereo(wf_path)
        add_sfx(vo_buf, vs, seg)
    vo_mix_path = os.path.join(AUDIO_DIR, 'vo_track.wav')
    save_wav_stereo(vo_mix_path, vo_buf)

    music_buf = synth_music(total_duration)
    music_path = os.path.join(AUDIO_DIR, 'music_track.wav')
    save_wav_stereo(music_path, music_buf)

    sfx_cues = []
    a0, a1, a2, a3, a4, a5, a6 = acts_out

    # Sync anchors embedded in timeline.json so visuals lock 0ms to SFX
    a2_exec = round(a2['start'] + a2['duration'] * 0.49, 3)
    a2_slam = round(a2['start'] + a2['duration'] * 0.57, 3)
    a2_xray = round(a2['start'] + a2['duration'] * 0.74, 3)
    a3_gauge = round(a3['start'] + a3['duration'] * 0.49, 3)
    a3_lock = round(a3['start'] + a3['duration'] * 0.67, 3)
    a4_cap_cross = round(a4['start'] + 1.35, 3)
    a4_breach = round(a4['start'] + a4['duration'] * 0.16, 3)
    a4_chats = round(a4['start'] + a4['duration'] * 0.36, 3)
    a4_click = round(a4['start'] + a4['duration'] * 0.68, 3)
    a5_ui = round(a5['start'] + a5['duration'] * 0.52, 3)
    a6_receipt = round(a6['start'] + a6['duration'] * 0.34, 3)
    a6_stamp = round(a6['start'] + a6['duration'] * 0.46, 3)
    a6_brand = round(a6['start'] + a6['duration'] * 0.68, 3)

    # Act 0: Problem scale -> 3 regulatory fault cards
    sfx_cues.append({'t': a0['start'] + 0.04, 'kind': 'whoosh'})
    sfx_cues.append({'t': round(a0['start'] + a0['duration'] * 0.32, 3), 'kind': 'thud'})
    sfx_cues.append({'t': round(a0['start'] + a0['duration'] * 0.48, 3), 'kind': 'thud'})
    sfx_cues.append({'t': round(a0['start'] + a0['duration'] * 0.64, 3), 'kind': 'stamp_thud'})

    # Act 1: Solution 3D Isometric 6-layer cascade
    sfx_cues.append({'t': a1['start'] + 0.04, 'kind': 'whoosh'})
    for k in range(6):
        sfx_cues.append({'t': round(a1['start'] + a1['duration'] * (0.28 + k * 0.095), 3), 'kind': 'pop'})
    sfx_cues.append({'t': round(a1['start'] + a1['duration'] * 0.88, 3), 'kind': 'chime'})

    # Act 2: Live typed prompt injection -> Execute click -> Shield slam -> X-Ray
    sfx_cues.append({'t': a2['start'] + 0.04, 'kind': 'whoosh'})
    t_type_start = a2['start'] + a2['duration'] * 0.06
    t_type_end = a2['start'] + a2['duration'] * 0.44
    t_cur = t_type_start
    step_idx = 0
    while t_cur < t_type_end:
        sfx_cues.append({'t': round(t_cur, 3), 'kind': 'key_tap'})
        t_cur += 0.105 if (step_idx % 6 != 0) else 0.21
        step_idx += 1
    sfx_cues.append({'t': a2_exec, 'kind': 'pop'})
    sfx_cues.append({'t': a2_slam, 'kind': 'stamp_thud'})
    sfx_cues.append({'t': a2_xray, 'kind': 'whoosh'})

    # Act 3: PII Vault -> Decision 1 Gauge lock
    sfx_cues.append({'t': a3['start'] + 0.04, 'kind': 'whoosh'})
    sfx_cues.append({'t': round(a3['start'] + a3['duration'] * 0.18, 3), 'kind': 'pop'})
    sfx_cues.append({'t': round(a3['start'] + a3['duration'] * 0.28, 3), 'kind': 'pop'})
    sfx_cues.append({'t': a3_gauge, 'kind': 'whoosh'})
    sfx_cues.append({'t': a3_lock, 'kind': 'stamp_thud'})
    sfx_cues.append({'t': round(a3_lock + 0.25, 3), 'kind': 'chime'})

    # Act 4: RBI ₹5,000 cap-cross -> ₹25k breaker -> Maker-Checker Approve
    sfx_cues.append({'t': a4['start'] + 0.04, 'kind': 'thud'})
    sfx_cues.append({'t': a4_cap_cross, 'kind': 'pop'})
    sfx_cues.append({'t': round(a4_cap_cross + 0.04, 3), 'kind': 'chime'})
    sfx_cues.append({'t': a4_breach, 'kind': 'stamp_thud'})
    sfx_cues.append({'t': a4_chats, 'kind': 'whoosh'})
    sfx_cues.append({'t': a4_click, 'kind': 'stamp_thud'})
    sfx_cues.append({'t': round(a4_click + 0.35, 3), 'kind': 'chime'})

    # Act 5: 3-Model Orchestra -> Live Sentinel & Settings UI
    sfx_cues.append({'t': a5['start'] + 0.04, 'kind': 'whoosh'})
    sfx_cues.append({'t': round(a5['start'] + 0.85, 3), 'kind': 'pop'})
    sfx_cues.append({'t': round(a5['start'] + 1.35, 3), 'kind': 'pop'})
    sfx_cues.append({'t': round(a5['start'] + 1.85, 3), 'kind': 'pop'})
    sfx_cues.append({'t': a5_ui, 'kind': 'whoosh'})

    # Act 6: Impact benchmark -> Receipt + Paper Lock + AAA Seal -> Brand Authority Finale
    sfx_cues.append({'t': a6['start'] + 0.04, 'kind': 'thud'})
    sfx_cues.append({'t': round(a6['start'] + 1.9, 3), 'kind': 'pop'})
    sfx_cues.append({'t': a6_receipt, 'kind': 'paper'})
    sfx_cues.append({'t': round(a6_stamp - 0.16, 3), 'kind': 'paper_lock'})
    sfx_cues.append({'t': a6_stamp, 'kind': 'stamp_thud'})
    sfx_cues.append({'t': a6_brand, 'kind': 'whoosh'})
    sfx_cues.append({'t': round(a6_brand + 0.45, 3), 'kind': 'chime'})

    sfx_buf = np.zeros((n_samples, 2), dtype=np.float32)
    bank = {
        'thud': synth_thud(),
        'stamp_thud': synth_stamp_thud(),
        'whoosh': synth_whoosh(),
        'riser': synth_riser(),
        'pop': synth_pop_click(),
        'key_tap': synth_key_tap(),
        'chime': synth_chime(),
        'paper': synth_paper_slide(),
        'paper_lock': synth_paper_lock(),
    }
    for c in sfx_cues:
        add_sfx(sfx_buf, c['t'], bank[c['kind']])
    sfx_path = os.path.join(AUDIO_DIR, 'sfx_track.wav')
    save_wav_stereo(sfx_path, sfx_buf)

    # Warm 7.6kHz ambient bed at 0.68 + smooth sidechain ducking + single-stage LRA=8 master loudnorm
    master_path = os.path.join(AUDIO_DIR, 'master_audio.wav')
    subprocess.run([
        'ffmpeg', '-y',
        '-i', music_path,
        '-i', vo_mix_path,
        '-i', sfx_path,
        '-filter_complex',
        '[0:a]lowpass=f=7600,volume=0.68[m];[1:a]asplit=2[vo1][vo2];[m][vo1]sidechaincompress=threshold=0.010:ratio=9:attack=22:release=300[ducked];[ducked][vo2][2:a]amix=inputs=3:duration=first:normalize=0,loudnorm=I=-14:TP=-1.0:LRA=8[out]',
        '-map', '[out]',
        '-ar', str(SR), '-ac', '2',
        master_path,
    ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    timeline = {
        'fps': 60,
        'width': 1920,
        'height': 1080,
        'duration': total_duration,
        'acts': acts_out,
        'cues': {
            'a2_exec': a2_exec,
            'a2_slam': a2_slam,
            'a2_xray': a2_xray,
            'a3_gauge': a3_gauge,
            'a3_lock': a3_lock,
            'a4_cap_cross': a4_cap_cross,
            'a4_breach': a4_breach,
            'a4_chats': a4_chats,
            'a4_click': a4_click,
            'a5_ui': a5_ui,
            'a6_receipt': a6_receipt,
            'a6_stamp': a6_stamp,
            'a6_brand': a6_brand,
        },
        'sfx_cues': sfx_cues,
    }
    with open(TIMELINE_PATH, 'w', encoding='utf-8') as f:
        json.dump(timeline, f, indent=2)
    print(f'[AUDIO OK] {master_path} & {TIMELINE_PATH}')

if __name__ == '__main__':
    main()
