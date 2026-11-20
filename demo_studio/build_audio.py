import base64
import json
import os
import subprocess
import sys
import time
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

def synthesize_tts(api_key: str, text: str, out_wav: str, voice: str = 'Charon'):
    models = ['gemini-2.5-flash-preview-tts', 'gemini-3.8-flash-lite-tts', 'gemini-3.8-flash-tts']
    directed_text = (
        'Speak in a warm, authoritative, crystal-clear studio keynote narrator voice with engaging natural rhythm and zero rush:\n\n'
        + text
    )
    payload = {
        'contents': [{'role': 'user', 'parts': [{'text': directed_text}]}],
        'generationConfig': {
            'responseModalities': ['AUDIO'],
            'speechConfig': {'voiceConfig': {'prebuiltVoiceConfig': {'voiceName': voice}}},
        },
    }
    raw_wav = out_wav.replace('.wav', '_raw.wav')
    for m in models:
        url = f'https://generativelanguage.googleapis.com/v1beta/models/{m}:generateContent'
        req = urllib.request.Request(
            url,
            data=json.dumps(payload).encode('utf-8'),
            headers={'x-goog-api-key': api_key, 'Content-Type': 'application/json'},
            method='POST',
        )
        for attempt in range(2):
            try:
                with urllib.request.urlopen(req, timeout=45) as resp:
                    body = json.loads(resp.read().decode('utf-8'))
                parts = body['candidates'][0]['content']['parts']
                inline = next(p['inlineData'] for p in parts if 'inlineData' in p)
                mime = inline.get('mimeType', 'audio/wav')
                raw_bytes = base64.b64decode(inline['data'])
                if 'L16' in mime or 'pcm' in mime:
                    with wave.open(raw_wav, 'wb') as wf:
                        wf.setnchannels(1)
                        wf.setsampwidth(2)
                        wf.setframerate(24000)
                        wf.writeframes(raw_bytes)
                else:
                    with open(raw_wav, 'wb') as f:
                        f.write(raw_bytes)

                # Native 1.0x broadcast vocal mastering (zero atempo metallic artifacts)
                af_chain = (
                    'silenceremove=start_periods=1:start_silence=0.02:start_threshold=-45dB,'
                    'areverse,silenceremove=start_periods=1:start_silence=0.02:start_threshold=-45dB,areverse,'
                    'highpass=f=75,'
                    'equalizer=f=180:t=q:w=1.2:g=2.0,'
                    'equalizer=f=3200:t=q:w=1.4:g=1.8,'
                    'equalizer=f=7500:t=q:w=1.6:g=-1.5,'
                    'acompressor=threshold=-18dB:ratio=2.8:attack=12:release=140:makeup=1.5,'
                    'loudnorm=I=-14.5:TP=-1.0:LRA=6'
                )
                subprocess.run(
                    ['ffmpeg', '-y', '-i', raw_wav, '-af', af_chain, '-ar', str(SR), '-ac', '2', out_wav],
                    check=True,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                os.remove(raw_wav)
                dur = wav_duration(out_wav)
                print(f'[TTS OK] {os.path.basename(out_wav)} via {m} ({voice}): {dur:.2f}s')
                return dur
            except Exception as e:
                print(f'[TTS RETRY] {m} attempt {attempt+1}: {e}')
                time.sleep(2.5)
    raise RuntimeError(f'All TTS models failed for {out_wav}')

ACTS_SPEC = [
    {
        'id': 'a0_problem',
        'title': '01 // THE FINTECH AI COMPLIANCE CRISIS',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'text': (
            'Indian fintechs want autonomous AI handling refunds in seconds. '
            'Yet one prompt injection drains twenty-five thousand rupees, raw Aadhaar and PAN leak under '
            'two-hundred-fifty-crore DPDP penalties, and agents breach RBI five-thousand-rupee caps. '
            'Banking AI is unsafe to ship.'
        ),
    },
    {
        'id': 'a1_solution',
        'title': '02 // MEET AEGISMESH · SIX LAYERS OF RUST',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'text': (
            'Meet AegisMesh: a six-layer Rust security gateway powered by Google Gemini. '
            'Every prompt clears six defense plates: regex normalization, DPDP PII vaulting, '
            'semantic guardrails, RBI circuit breakers, egress scrubbing, and an async compliance sentinel.'
        ),
    },
    {
        'id': 'a2_live_attack',
        'title': '03 // LIVE DEMO · TYPED PROMPT INJECTION BLOCKED',
        'pre_pad': 0.20,
        'post_pad': 0.50,
        'text': (
            'Watch a live attack fail in real time. The attacker types an override demanding twenty-five thousand rupees '
            'plus a customer PAN. Layer One blocks the jailbreak in thirty-seven microseconds, '
            'Layer Two vaults the PAN, and X-Ray stops Base64 XSS cold.'
        ),
    },
    {
        'id': 'a3_gemini_innovation',
        'title': '04 // DPDP VAULT & GEMINI DECISION ONE',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'text': (
            'For subtle social engineering without obvious keywords, our DPDP vault first laser-scrubs Aadhaar, '
            'PAN, and phone strings. Then zero-token Google Gemini Decision One scores hidden RBI coercion '
            'at zero point nine seven one critical violation.'
        ),
    },
    {
        'id': 'a4_maker_checker',
        'title': '05 // RBI CIRCUIT BREAKER & MAKER-CHECKER',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'text': (
            'Safe banking also governs real disputes. When Kavita Roy requests a genuine twenty-five-thousand-rupee refund, '
            'Layer Four trips the five-thousand-rupee RBI breaker. An officer inspects the trace, '
            'approves settlement in one click, and takes over live.'
        ),
    },
    {
        'id': 'a5_gemini_orchestra',
        'title': '06 // THREE ROLE-TUNED GEMINI MODELS',
        'pre_pad': 0.20,
        'post_pad': 0.45,
        'text': (
            'AegisMesh orchestrates three Google Gemini models: Flash-Lite thinks for customers, '
            'zero-token Decision One guards every turn, and async Flash-Preview audits multi-turn drift '
            'over Tokio channels, giving compliance teams live control with zero delay.'
        ),
    },
    {
        'id': 'a6_impact_brand',
        'title': '07 // PROVEN RUST SPEED & AUDIT TRUST',
        'pre_pad': 0.20,
        'post_pad': 1.85,
        'text': (
            'Compared to four-hundred-millisecond Python proxies, AegisMesh protects prompts in thirty-seven microseconds — '
            'ten thousand times faster at seventy-five thousand ops per second. '
            'Every turn writes an immutable RBI audit trail. AegisMesh: safe AI for Indian fintech.'
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

def main():
    regen_vo = '--regen-vo' in sys.argv
    api_key = load_api_key()
    acts_out = []
    vo_files = []
    cursor_t = 0.0

    for i, spec in enumerate(ACTS_SPEC):
        wav_path = os.path.join(AUDIO_DIR, f'buildathon_vo_{i}_{spec["id"]}.wav')
        if not regen_vo and os.path.exists(wav_path) and os.path.getsize(wav_path) > 1000:
            vo_dur = wav_duration(wav_path)
            print(f'[TTS REUSE] {os.path.basename(wav_path)}: {vo_dur:.2f}s')
        else:
            if i > 0:
                time.sleep(3.5)
            vo_dur = synthesize_tts(api_key, spec['text'], wav_path, voice='Charon')
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
    a4_breach = round(a4['start'] + a4['duration'] * 0.14, 3)
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

    # Act 4: RBI ₹5,000 breaker -> Maker-Checker Approve
    sfx_cues.append({'t': a4['start'] + 0.04, 'kind': 'thud'})
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

    # Background music lowered ~18% (0.72 + warm 8.2kHz lowpass) + deeper sidechain ducking so voice shines cleanly
    master_path = os.path.join(AUDIO_DIR, 'master_audio.wav')
    subprocess.run([
        'ffmpeg', '-y',
        '-i', music_path,
        '-i', vo_mix_path,
        '-i', sfx_path,
        '-filter_complex',
        '[0:a]lowpass=f=8200,volume=0.72[m];[1:a]asplit=2[vo1][vo2];[m][vo1]sidechaincompress=threshold=0.010:ratio=10:attack=20:release=280[ducked];[ducked][vo2][2:a]amix=inputs=3:duration=first:normalize=0,loudnorm=I=-14:TP=-1.0:LRA=7[out]',
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
