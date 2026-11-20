import base64
import json
import os
import subprocess
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

def synthesize_tts(api_key: str, text: str, out_wav: str, voice: str = 'Kore'):
    models = ['gemini-3.8-flash-tts', 'gemini-3.8-flash-lite-tts', 'gemini-2.5-flash-preview-tts']
    payload = {
        'contents': [{'role': 'user', 'parts': [{'text': text}]}],
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
                with urllib.request.urlopen(req, timeout=30) as resp:
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
                # Resample & normalize cleanly to 48kHz stereo
                subprocess.run([
                    'ffmpeg', '-y', '-i', raw_wav,
                    '-af', 'silenceremove=start_periods=1:start_silence=0.05:start_threshold=-48dB,areverse,silenceremove=start_periods=1:start_silence=0.05:start_threshold=-48dB,areverse,loudnorm=I=-16:TP=-1.5:LRA=7',
                    '-ar', str(SR), '-ac', '2', out_wav,
                ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                os.remove(raw_wav)
                with wave.open(out_wav, 'rb') as wf:
                    dur = wf.getnframes() / float(wf.getframerate())
                print(f'[TTS OK] {os.path.basename(out_wav)} via {m}: {dur:.2f}s')
                return dur
            except Exception as e:
                print(f'[TTS RETRY] {m} attempt {attempt+1}: {e}')
                time.sleep(1.5)
    raise RuntimeError(f'All TTS models failed for {out_wav}')

ACTS_SPEC = [
    {
        'id': 'act0_hook',
        'title': 'Before vs After // AegisMesh 3D Studio',
        'pre_pad': 0.55,
        'post_pad': 2.0,
        'text': 'Most fintech AI demos look flat, and ship wide open. Watch what happens when AegisMesh puts six layers of Rust security around every customer turn.',
    },
    {
        'id': 'act1_overview',
        'title': '01 // Native Rust & DPDP Telemetry Mesh',
        'pre_pad': 0.6,
        'post_pad': 1.4,
        'text': 'Built in native Rust and Svelte Five, AegisMesh inspects every prompt in thirty-seven microseconds, masking Aadhaar and PAN under India DPDP Act before any model sees a single byte.',
    },
    {
        'id': 'act2_sandbox',
        'title': '02 // Red-Team Attack Sandbox & Gemini Decision 1',
        'pre_pad': 0.6,
        'post_pad': 1.5,
        'text': 'In the live Attack Sandbox, XSS, Base64 smuggling, and Hinglish jailbreaks are stopped cold, while Google Gemini Decision One catches subtle social engineering with zero output tokens.',
    },
    {
        'id': 'act3_chats',
        'title': '03 // Layer 4 RBI Breaker & Maker-Checker Staff Control',
        'pre_pad': 0.6,
        'post_pad': 1.6,
        'text': 'When an agent proposes a twenty-five-thousand-rupee refund, Layer Four RBI Circuit Breaker halts execution automatically. Officers inspect the tool trace, confirm the ledger update, and take over live.',
    },
    {
        'id': 'act4_sentinel',
        'title': '04 // Layer 6 Async Sentinel & 3-Model Gemini Stack',
        'pre_pad': 0.6,
        'post_pad': 1.4,
        'text': 'Off the hot path, an asynchronous Gemini Sentinel scores multi-turn drift and compliance grade AAA, controlled cleanly alongside all three Gemini models and instant Command K shortcuts.',
    },
    {
        'id': 'act5_finale',
        'title': '05 // RBI Compliance Audit Trail & Google Cloud Run',
        'pre_pad': 0.7,
        'post_pad': 3.2,
        'text': 'Every decision prints an immutable RBI and DPDP audit record. Ship AegisMesh as a single Windows executable or deploy straight to Google Cloud Run.',
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
    dur = 0.42
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freq = 92.0 * np.exp(-t * 9.5) + 36.0
    phase = 2.0 * np.pi * np.cumsum(freq) / SR
    sub = np.sin(phase) * np.exp(-t * 8.0) * 0.68
    click = np.sin(2.0 * np.pi * 165.0 * t) * np.exp(-t * 45.0) * 0.35
    mono = sub + click
    return np.column_stack([mono, mono])

def synth_whoosh():
    dur = 0.36
    n = int(dur * SR)
    t = np.linspace(0, dur, n, endpoint=False)
    rng = np.random.default_rng(7)
    noise = rng.standard_normal(n)
    # Soft band-swept envelope
    env = np.sin(np.pi * (t / dur)) ** 1.6
    # Simple moving-average smoothing that varies for whoosh character
    kernel = np.hanning(35)
    kernel /= kernel.sum()
    smooth = np.convolve(noise, kernel, mode='same')
    carrier = np.sin(2 * np.pi * (180 + 420 * (t / dur)) * t) * 0.14
    l = (smooth * 0.35 + carrier) * env
    r = (np.roll(smooth, 90) * 0.35 + carrier) * env
    return np.column_stack([l, r])

def synth_pop_click():
    dur = 0.055
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freq = 860.0 * np.exp(-t * 42.0) + 280.0
    phase = 2.0 * np.pi * np.cumsum(freq) / SR
    mono = np.sin(phase) * np.exp(-t * 68.0) * 0.52
    return np.column_stack([mono, mono])

def synth_chime():
    dur = 0.9
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freqs = [554.37, 659.25, 830.61, 1108.73]
    sig = sum(np.sin(2 * np.pi * f * t) * np.exp(-t * (3.2 + 0.4 * i)) for i, f in enumerate(freqs))
    sig *= (1.0 - np.exp(-t * 140.0)) * 0.16
    return np.column_stack([sig, np.roll(sig, 180)])

def synth_paper_slide():
    dur = 1.15
    n = int(dur * SR)
    t = np.linspace(0, dur, n, endpoint=False)
    rng = np.random.default_rng(42)
    noise = rng.standard_normal(n)
    # High-pass feel via difference + mechanical stepper micro-pulses
    hp = np.diff(noise, prepend=0.0)
    env = np.sin(np.pi * (t / dur)) ** 0.85
    pulses = (0.5 + 0.5 * np.sin(2 * np.pi * 22.0 * t)) * 0.55 + 0.45
    mono = hp * env * pulses * 0.22
    return np.column_stack([mono, np.roll(mono, 48)])

def synth_music(total_dur: float):
    n = int(total_dur * SR)
    t = np.linspace(0, total_dur, n, endpoint=False)
    bpm = 120.0
    beat_dur = 60.0 / bpm
    buf = np.zeros((n, 2), dtype=np.float32)
    rng = np.random.default_rng(101)

    # F#m9 - Dmaj7 - Aadd9 - E6 chord roots & voicings (clean Linear/Apple launch pulse)
    chords = [
        [185.00, 220.00, 277.18, 329.63, 415.30],
        [146.83, 220.00, 277.18, 369.99, 440.00],
        [220.00, 277.18, 329.63, 493.88],
        [164.81, 207.65, 246.94, 329.63, 415.30],
    ]
    basses = [92.50, 73.42, 110.00, 82.41]

    # Ambient studio pad bed
    bar_idx = np.floor(t / (4.0 * beat_dur)).astype(int) % 4
    pad = np.zeros(n, dtype=np.float32)
    for ci, freqs in enumerate(chords):
        mask = (bar_idx == ci)
        local_t = t[mask]
        val = sum(np.sin(2 * np.pi * f * local_t) for f in freqs) / len(freqs)
        pad[mask] = val
    pad_l = pad * 0.14
    pad_r = np.roll(pad, 360) * 0.14
    buf[:, 0] += pad_l
    buf[:, 1] += pad_r

    total_beats = int(total_dur / beat_dur)
    for b in range(total_beats):
        bt = b * beat_dur
        idx = int(bt * SR)
        # Tight minimal kick on every beat
        kd = 0.20
        kt = np.linspace(0, kd, int(kd * SR), endpoint=False)
        kf = 105.0 * np.exp(-kt * 22.0) + 46.0
        kph = 2 * np.pi * np.cumsum(kf) / SR
        kick = np.sin(kph) * np.exp(-kt * 16.0) * 0.24
        kn = min(len(kick), n - idx)
        buf[idx : idx + kn, 0] += kick[:kn]
        buf[idx : idx + kn, 1] += kick[:kn]

        # Plucked bass off-beat groove
        bass_f = basses[(b // 4) % 4]
        for sub in (0.0, 0.5):
            if sub == 0.0 and (b % 2 == 0):
                continue
            s_idx = int((bt + sub * beat_dur) * SR)
            bd = 0.18
            btt = np.linspace(0, bd, int(bd * SR), endpoint=False)
            bs = (np.sin(2 * np.pi * bass_f * btt) + 0.35 * np.sin(4 * np.pi * bass_f * btt)) * np.exp(-btt * 14.0) * 0.16
            bn = min(len(bs), n - s_idx)
            if bn > 0:
                buf[s_idx : s_idx + bn, 0] += bs[:bn]
                buf[s_idx : s_idx + bn, 1] += bs[:bn]

        # Crisp light hi-hat on 8ths
        for eighth in (0.0, 0.5):
            h_idx = int((bt + eighth * beat_dur) * SR)
            hd = 0.032
            hn = min(int(hd * SR), n - h_idx)
            if hn > 2:
                ht = np.linspace(0, hd, hn, endpoint=False)
                nz = rng.standard_normal(hn)
                hat = np.diff(nz, prepend=0.0) * np.exp(-ht * 95.0) * (0.045 if eighth else 0.03)
                buf[h_idx : h_idx + hn, 0] += hat
                buf[h_idx : h_idx + hn, 1] += hat * 0.85

        # Melodic glass pluck arp (clean tech energy)
        chord = chords[(b // 4) % 4]
        for step in range(2):
            p_idx = int((bt + step * 0.5 * beat_dur) * SR)
            freq = chord[(b * 2 + step) % len(chord)] * 2.0
            pd = 0.22
            pn = min(int(pd * SR), n - p_idx)
            if pn > 0:
                pt = np.linspace(0, pd, pn, endpoint=False)
                pluck = np.sin(2 * np.pi * freq * pt) * np.exp(-pt * 15.0) * 0.075
                pan = 0.7 if ((b + step) % 2 == 0) else 0.3
                buf[p_idx : p_idx + pn, 0] += pluck * pan
                buf[p_idx : p_idx + pn, 1] += pluck * (1.0 - pan)

    # Fade in & out gracefully
    fade_in = int(1.0 * SR)
    fade_out = int(2.2 * SR)
    if len(buf) > fade_in:
        buf[:fade_in] *= np.linspace(0.0, 1.0, fade_in)[:, None]
    if len(buf) > fade_out:
        buf[-fade_out:] *= np.linspace(1.0, 0.0, fade_out)[:, None]
    return buf

def main():
    api_key = load_api_key()
    acts_out = []
    vo_files = []
    cursor_t = 0.0

    for i, spec in enumerate(ACTS_SPEC):
        wav_path = os.path.join(AUDIO_DIR, f'vo_{i}_{spec["id"]}.wav')
        vo_dur = synthesize_tts(api_key, spec['text'], wav_path)
        start_t = round(cursor_t, 3)
        vo_start = round(start_t + spec['pre_pad'], 3)
        vo_end = round(vo_start + vo_dur, 3)
        # Ensure minimum breathing visual duration per act
        end_t = round(max(vo_end + spec['post_pad'], start_t + 11.5 if i == 0 else vo_end + spec['post_pad']), 3)
        acts_out.append({
            'id': spec['id'],
            'title': spec['title'],
            'start': start_t,
            'end': end_t,
            'vo_start': vo_start,
            'vo_end': vo_end,
            'duration': round(end_t - start_t, 3),
        })
        vo_files.append((vo_start, wav_path))
        cursor_t = end_t

    total_duration = round(cursor_t, 3)
    print(f'[TIMELINE] Total video duration = {total_duration:.2f}s ({len(acts_out)} acts)')
    assert total_duration < 175.0, 'Video must stay strictly under 3 minutes!'

    # Assemble combined VO track
    n_samples = int(np.ceil(total_duration * SR))
    vo_buf = np.zeros((n_samples, 2), dtype=np.float32)
    for vs, wf_path in vo_files:
        seg = load_wav_stereo(wf_path)
        add_sfx(vo_buf, vs, seg)
    vo_mix_path = os.path.join(AUDIO_DIR, 'vo_track.wav')
    save_wav_stereo(vo_mix_path, vo_buf)

    # Generate music track
    music_buf = synth_music(total_duration)
    music_path = os.path.join(AUDIO_DIR, 'music_track.wav')
    save_wav_stereo(music_path, music_buf)

    # Synchronized SFX cues aligned with stage.html choreography
    sfx_cues = []
    a0, a1, a2, a3, a4, a5 = acts_out
    sfx_cues.append({'t': 1.85, 'kind': 'pop'})
    sfx_cues.append({'t': 3.60, 'kind': 'whoosh'})
    sfx_cues.append({'t': a0['end'] - 0.85, 'kind': 'thud'})
    sfx_cues.append({'t': a1['start'] + 1.4, 'kind': 'whoosh'})
    sfx_cues.append({'t': a1['start'] + 4.2, 'kind': 'pop'})
    sfx_cues.append({'t': a2['start'] + 0.05, 'kind': 'thud'})
    sfx_cues.append({'t': a2['start'] + 1.85, 'kind': 'pop'})
    sfx_cues.append({'t': a2['start'] + 2.35, 'kind': 'whoosh'})
    sfx_cues.append({'t': a2['start'] + 6.40, 'kind': 'pop'})
    sfx_cues.append({'t': a2['start'] + 6.85, 'kind': 'chime'})
    sfx_cues.append({'t': a3['start'] + 0.05, 'kind': 'thud'})
    sfx_cues.append({'t': a3['start'] + 2.15, 'kind': 'pop'})
    sfx_cues.append({'t': a3['start'] + 4.65, 'kind': 'whoosh'})
    sfx_cues.append({'t': a3['start'] + 6.80, 'kind': 'pop'})
    sfx_cues.append({'t': a3['start'] + 7.20, 'kind': 'chime'})
    sfx_cues.append({'t': a4['start'] + 0.05, 'kind': 'thud'})
    sfx_cues.append({'t': a4['start'] + 3.80, 'kind': 'whoosh'})
    sfx_cues.append({'t': a4['start'] + 7.40, 'kind': 'pop'})
    sfx_cues.append({'t': a5['start'] + 0.05, 'kind': 'thud'})
    sfx_cues.append({'t': a5['start'] + 1.35, 'kind': 'paper'})
    sfx_cues.append({'t': a5['start'] + 5.60, 'kind': 'chime'})

    sfx_buf = np.zeros((n_samples, 2), dtype=np.float32)
    bank = {
        'thud': synth_thud(),
        'whoosh': synth_whoosh(),
        'pop': synth_pop_click(),
        'chime': synth_chime(),
        'paper': synth_paper_slide(),
    }
    for c in sfx_cues:
        add_sfx(sfx_buf, c['t'], bank[c['kind']])
    sfx_path = os.path.join(AUDIO_DIR, 'sfx_track.wav')
    save_wav_stereo(sfx_path, sfx_buf)

    master_path = os.path.join(AUDIO_DIR, 'master_audio.wav')
    # Duck music cleanly beneath narration with ffmpeg sidechaincompress & mix SFX
    subprocess.run([
        'ffmpeg', '-y',
        '-i', music_path,
        '-i', vo_mix_path,
        '-i', sfx_path,
        '-filter_complex',
        '[0:a]volume=0.82[m];[1:a]asplit=2[vo1][vo2];[m][vo1]sidechaincompress=threshold=0.02:ratio=6:attack=40:release=320[ducked];[ducked][vo2][2:a]amix=inputs=3:duration=first:normalize=0,alimiter=limit=0.95[out]',
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
        'sfx_cues': sfx_cues,
    }
    with open(TIMELINE_PATH, 'w', encoding='utf-8') as f:
        json.dump(timeline, f, indent=2)
    print(f'[AUDIO OK] Wrote {master_path} & {TIMELINE_PATH}')

if __name__ == '__main__':
    main()
