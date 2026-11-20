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
                # Trim edge silence, tighten cadence slightly (+7% fast keynote delivery), normalize cleanly
                subprocess.run([
                    'ffmpeg', '-y', '-i', raw_wav,
                    '-af', 'silenceremove=start_periods=1:start_silence=0.02:start_threshold=-45dB,areverse,silenceremove=start_periods=1:start_silence=0.02:start_threshold=-45dB,areverse,atempo=1.07,loudnorm=I=-15:TP=-1.2:LRA=6',
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

def wav_duration(wav_path: str) -> float:
    with wave.open(wav_path, 'rb') as wf:
        return wf.getnframes() / float(wf.getframerate())

ACTS_SPEC = [
    {
        'id': 'b0_slam',
        'title': 'AEGISMESH // SIX-LAYER FINTECH AI FIREWALL',
        'pre_pad': 0.14,
        'post_pad': 0.30,
        'text': 'Autonomous banking AI breaks fast under RBI caps and DPDP privacy law. Meet AegisMesh, six layers of native Rust security built for Indian fintech.',
    },
    {
        'id': 'b1_overview',
        'title': '01 // 37µs RUST CORE & DPDP PII VAULT',
        'pre_pad': 0.14,
        'post_pad': 0.28,
        'text': 'Every prompt clears our Rust normalizer in thirty-seven microseconds, vaulting Aadhaar, PAN, and UPI tokens before any model sees plaintext.',
    },
    {
        'id': 'b2_sandbox',
        'title': '02 // LIVE ATTACK SANDBOX & GEMINI DECISION 1',
        'pre_pad': 0.14,
        'post_pad': 0.30,
        'text': 'XSS, Base64 smuggling, and Hinglish jailbreaks stop cold at Layer One, while zero-token Gemini Decision One catches subtle RBI coercion live.',
    },
    {
        'id': 'b3_chats',
        'title': '03 // RBI CIRCUIT BREAKER & MAKER-CHECKER',
        'pre_pad': 0.14,
        'post_pad': 0.32,
        'text': 'When an agent triggers a twenty-five-thousand-rupee refund, Layer Four halts execution immediately. Officers inspect the trace, confirm ledger settlement, or take over live.',
    },
    {
        'id': 'b4_sentinel',
        'title': '04 // ASYNC SENTINEL & 3-MODEL GEMINI STACK',
        'pre_pad': 0.14,
        'post_pad': 0.30,
        'text': 'Off the hot path, a Layer Six Gemini Sentinel audits compliance grade AAA, backed by three role-tuned Gemini models and instant Command K control.',
    },
    {
        'id': 'b5_finale',
        'title': '05 // IMMUTABLE AUDIT & GOOGLE CLOUD RUN',
        'pre_pad': 0.14,
        'post_pad': 2.25,
        'text': 'Every turn writes an immutable RBI audit trail. Run AegisMesh as one portable executable or deploy straight to Google Cloud Run Mumbai.',
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
    sub = np.sin(phase) * np.exp(-t * 9.5) * 0.72
    punch = np.sin(2.0 * np.pi * 195.0 * t) * np.exp(-t * 52.0) * 0.36
    mono = sub + punch
    return np.column_stack([mono, mono])

def synth_stamp_thud():
    dur = 0.48
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freq = 104.0 * np.exp(-t * 15.0) + 32.0
    phase = 2.0 * np.pi * np.cumsum(freq) / SR
    sub = np.sin(phase) * np.exp(-t * 6.8) * 1.15
    rng = np.random.default_rng(19)
    click = np.diff(rng.standard_normal(len(t)), prepend=0.0) * np.exp(-t * 85.0) * 0.44
    seal = np.sin(2.0 * np.pi * 230.0 * t) * np.exp(-t * 32.0) * 0.50
    mono = np.tanh((sub + click + seal) * 1.25) * 0.96
    return np.column_stack([mono, np.roll(mono, 20)])

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
    carrier = np.sin(2 * np.pi * (240 + 520 * (t / dur)) * t) * 0.14
    l = (smooth * 0.34 + carrier) * env
    r = (np.roll(smooth, 72) * 0.34 + carrier) * env
    return np.column_stack([l, r])

def synth_riser():
    dur = 0.45
    n = int(dur * SR)
    t = np.linspace(0, dur, n, endpoint=False)
    freq = 160.0 + 620.0 * (t / dur) ** 2.2
    ph = 2.0 * np.pi * np.cumsum(freq) / SR
    env = (t / dur) ** 1.8 * 0.18
    mono = np.sin(ph) * env
    return np.column_stack([mono, np.roll(mono, 40)])

def synth_pop_click():
    dur = 0.045
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freq = 920.0 * np.exp(-t * 48.0) + 310.0
    phase = 2.0 * np.pi * np.cumsum(freq) / SR
    mono = np.sin(phase) * np.exp(-t * 74.0) * 0.50
    return np.column_stack([mono, mono])

def synth_chime():
    dur = 0.65
    t = np.linspace(0, dur, int(dur * SR), endpoint=False)
    freqs = [554.37, 659.25, 830.61, 1108.73]
    sig = sum(np.sin(2 * np.pi * f * t) * np.exp(-t * (4.0 + 0.4 * i)) for i, f in enumerate(freqs))
    sig *= (1.0 - np.exp(-t * 160.0)) * 0.16
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
    mono = hp * env * pulses * 0.24
    return np.column_stack([mono, np.roll(mono, 42)])

def synth_music(total_dur: float):
    # Driving 128 BPM keynote electronic beat
    n = int(total_dur * SR)
    t = np.linspace(0, total_dur, n, endpoint=False)
    bpm = 128.0
    beat_dur = 60.0 / bpm
    buf = np.zeros((n, 2), dtype=np.float32)
    rng = np.random.default_rng(108)

    chords = [
        [185.00, 220.00, 277.18, 329.63, 415.30],
        [146.83, 220.00, 277.18, 369.99, 440.00],
        [220.00, 277.18, 329.63, 493.88],
        [164.81, 207.65, 246.94, 329.63, 415.30],
    ]
    basses = [92.50, 73.42, 110.00, 82.41]

    bar_idx = np.floor(t / (4.0 * beat_dur)).astype(int) % 4
    pad = np.zeros(n, dtype=np.float32)
    for ci, freqs in enumerate(chords):
        mask = (bar_idx == ci)
        local_t = t[mask]
        val = sum(np.sin(2 * np.pi * f * local_t) for f in freqs) / len(freqs)
        pad[mask] = val
    buf[:, 0] += pad * 0.11
    buf[:, 1] += np.roll(pad, 320) * 0.11

    total_beats = int(total_dur / beat_dur)
    for b in range(total_beats):
        bt = b * beat_dur
        idx = int(bt * SR)
        kd = 0.18
        kt = np.linspace(0, kd, int(kd * SR), endpoint=False)
        kf = 118.0 * np.exp(-kt * 26.0) + 48.0
        kph = 2 * np.pi * np.cumsum(kf) / SR
        kick = np.sin(kph) * np.exp(-kt * 17.5) * 0.28
        kn = min(len(kick), n - idx)
        buf[idx : idx + kn, 0] += kick[:kn]
        buf[idx : idx + kn, 1] += kick[:kn]

        # Driving 16th-beat pulse bass
        bass_f = basses[(b // 4) % 4]
        for sub in (0.25, 0.5, 0.75):
            s_idx = int((bt + sub * beat_dur) * SR)
            bd = 0.11
            btt = np.linspace(0, bd, int(bd * SR), endpoint=False)
            bs = (np.sin(2 * np.pi * bass_f * btt) + 0.4 * np.sin(4 * np.pi * bass_f * btt)) * np.exp(-btt * 19.0) * 0.14
            bn = min(len(bs), n - s_idx)
            if bn > 0:
                buf[s_idx : s_idx + bn, 0] += bs[:bn]
                buf[s_idx : s_idx + bn, 1] += bs[:bn]

        # Crisp off-beat hi-hat & clap snap on beats 2 & 4
        if b % 2 == 1:
            sn_len = min(int(0.08 * SR), n - idx)
            if sn_len > 2:
                st = np.linspace(0, 0.08, sn_len, endpoint=False)
                snap = np.diff(rng.standard_normal(sn_len), prepend=0.0) * np.exp(-st * 48.0) * 0.09
                buf[idx : idx + sn_len, 0] += snap
                buf[idx : idx + sn_len, 1] += snap

        for eighth in (0.0, 0.5):
            h_idx = int((bt + eighth * beat_dur) * SR)
            hd = 0.025
            hn = min(int(hd * SR), n - h_idx)
            if hn > 2:
                ht = np.linspace(0, hd, hn, endpoint=False)
                hat = np.diff(rng.standard_normal(hn), prepend=0.0) * np.exp(-ht * 110.0) * (0.048 if eighth else 0.028)
                buf[h_idx : h_idx + hn, 0] += hat
                buf[h_idx : h_idx + hn, 1] += hat * 0.85

        chord = chords[(b // 4) % 4]
        for step in range(2):
            p_idx = int((bt + step * 0.5 * beat_dur) * SR)
            freq = chord[(b * 2 + step) % len(chord)] * 2.0
            pd = 0.16
            pn = min(int(pd * SR), n - p_idx)
            if pn > 0:
                pt = np.linspace(0, pd, pn, endpoint=False)
                pluck = np.sin(2 * np.pi * freq * pt) * np.exp(-pt * 18.0) * 0.078
                pan = 0.72 if ((b + step) % 2 == 0) else 0.28
                buf[p_idx : p_idx + pn, 0] += pluck * pan
                buf[p_idx : p_idx + pn, 1] += pluck * (1.0 - pan)

    fade_in = int(0.35 * SR)
    fade_out = int(1.2 * SR)
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
        wav_path = os.path.join(AUDIO_DIR, f'fast_vo_{i}_{spec["id"]}.wav')
        if not regen_vo and os.path.exists(wav_path) and os.path.getsize(wav_path) > 1000:
            vo_dur = wav_duration(wav_path)
            print(f'[TTS REUSE] {os.path.basename(wav_path)}: {vo_dur:.2f}s')
        else:
            if i > 0:
                time.sleep(1.0)
            vo_dur = synthesize_tts(api_key, spec['text'], wav_path)
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
    print(f'[TIMELINE] Fast Buildathon Cut = {total_duration:.2f}s ({len(acts_out)} tight beats)')

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
    b0, b1, b2, b3, b4, b5 = acts_out
    # Beat 0: Rogue prompt slam -> 6-layer unfold -> Bento UI reveal
    sfx_cues.append({'t': 0.02, 'kind': 'thud'})
    sfx_cues.append({'t': 2.90, 'kind': 'whoosh'})
    sfx_cues.append({'t': 6.40, 'kind': 'pop'})
    # Beat 1: 36.9 us hero stat -> PII scanner sweep -> PAN/Aadhaar lock chips
    sfx_cues.append({'t': b1['start'] - 0.35, 'kind': 'riser'})
    sfx_cues.append({'t': b1['start'] + 0.02, 'kind': 'thud'})
    sfx_cues.append({'t': b1['start'] + 3.10, 'kind': 'whoosh'})
    sfx_cues.append({'t': b1['start'] + 4.40, 'kind': 'pop'})
    sfx_cues.append({'t': b1['start'] + 5.00, 'kind': 'pop'})
    # Beat 2: Sandbox L1 Blocked spotlight -> gemini-decision-1 gauge lock
    sfx_cues.append({'t': b2['start'] - 0.35, 'kind': 'riser'})
    sfx_cues.append({'t': b2['start'] + 0.02, 'kind': 'thud'})
    sfx_cues.append({'t': b2['start'] + 1.60, 'kind': 'pop'})
    sfx_cues.append({'t': b2['start'] + 4.50, 'kind': 'whoosh'})
    sfx_cues.append({'t': b2['start'] + 6.10, 'kind': 'chime'})
    # Beat 3: RBI Circuit Breaker trip -> Macro Chats deliberate approval -> stamp_thud -> settled chime
    sfx_cues.append({'t': b3['start'] + 0.02, 'kind': 'thud'})
    sfx_cues.append({'t': b3['start'] + 1.20, 'kind': 'stamp_thud'})
    sfx_cues.append({'t': b3['start'] + 4.00, 'kind': 'whoosh'})
    sfx_cues.append({'t': b3['start'] + 6.95, 'kind': 'stamp_thud'})
    sfx_cues.append({'t': b3['start'] + 7.45, 'kind': 'chime'})
    # Beat 4: Sentinel AAA -> 3-Model Gemini Bento stack -> Minimal Settings Ken Burns
    sfx_cues.append({'t': b4['start'] + 0.02, 'kind': 'thud'})
    sfx_cues.append({'t': b4['start'] + 3.40, 'kind': 'whoosh'})
    sfx_cues.append({'t': b4['start'] + 4.20, 'kind': 'pop'})
    sfx_cues.append({'t': b4['start'] + 4.60, 'kind': 'pop'})
    sfx_cues.append({'t': b4['start'] + 5.00, 'kind': 'pop'})
    sfx_cues.append({'t': b4['start'] + 7.40, 'kind': 'whoosh'})
    # Beat 5: Receipt prints -> AAA stamp slams (stamp_thud) + extended Verified hold -> Cloud Run Mumbai CTA
    sfx_cues.append({'t': b5['start'] + 0.02, 'kind': 'thud'})
    sfx_cues.append({'t': b5['start'] + 0.22, 'kind': 'paper'})
    sfx_cues.append({'t': b5['start'] + 1.65, 'kind': 'stamp_thud'})
    sfx_cues.append({'t': b5['start'] + 5.65, 'kind': 'whoosh'})
    sfx_cues.append({'t': b5['start'] + 6.20, 'kind': 'chime'})

    sfx_buf = np.zeros((n_samples, 2), dtype=np.float32)
    bank = {
        'thud': synth_thud(),
        'stamp_thud': synth_stamp_thud(),
        'whoosh': synth_whoosh(),
        'riser': synth_riser(),
        'pop': synth_pop_click(),
        'chime': synth_chime(),
        'paper': synth_paper_slide(),
    }
    for c in sfx_cues:
        add_sfx(sfx_buf, c['t'], bank[c['kind']])
    sfx_path = os.path.join(AUDIO_DIR, 'sfx_track.wav')
    save_wav_stereo(sfx_path, sfx_buf)

    master_path = os.path.join(AUDIO_DIR, 'master_audio.wav')
    subprocess.run([
        'ffmpeg', '-y',
        '-i', music_path,
        '-i', vo_mix_path,
        '-i', sfx_path,
        '-filter_complex',
        '[0:a]volume=0.88[m];[1:a]asplit=2[vo1][vo2];[m][vo1]sidechaincompress=threshold=0.018:ratio=7:attack=25:release=220[ducked];[ducked][vo2][2:a]amix=inputs=3:duration=first:normalize=0,loudnorm=I=-14:TP=-1.0:LRA=7[out]',
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
    print(f'[AUDIO OK] {master_path} & {TIMELINE_PATH}')

if __name__ == '__main__':
    main()
