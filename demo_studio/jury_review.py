import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

ROOT = r'C:\Users\Ashutosh\Cyberground\spice'
ENV_PATH = os.path.join(ROOT, '.env')

def load_api_key():
    with open(ENV_PATH, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if line.startswith('GEMINI_API_KEY='):
                return line.split('=', 1)[1].strip().strip('"').strip("'")
    raise RuntimeError('GEMINI_API_KEY missing from spice/.env')

def upload_file_resumable(api_key: str, file_path: str, mime: str) -> str:
    size = os.path.getsize(file_path)
    start_req = urllib.request.Request(
        'https://generativelanguage.googleapis.com/upload/v1beta/files',
        data=json.dumps({'file': {'display_name': os.path.basename(file_path)}}).encode('utf-8'),
        headers={
            'x-goog-api-key': api_key,
            'X-Goog-Upload-Protocol': 'resumable',
            'X-Goog-Upload-Command': 'start',
            'X-Goog-Upload-Header-Content-Length': str(size),
            'X-Goog-Upload-Header-Content-Type': mime,
            'Content-Type': 'application/json',
        },
        method='POST',
    )
    with urllib.request.urlopen(start_req, timeout=30) as resp:
        upload_url = resp.headers.get('X-Goog-Upload-URL')
        if not upload_url:
            raise RuntimeError('Missing X-Goog-Upload-URL header from Gemini Files API')

    with open(file_path, 'rb') as f:
        blob = f.read()

    fin_req = urllib.request.Request(
        upload_url,
        data=blob,
        headers={
            'Content-Length': str(size),
            'X-Goog-Upload-Offset': '0',
            'X-Goog-Upload-Command': 'upload, finalize',
        },
        method='PUT',
    )
    with urllib.request.urlopen(fin_req, timeout=120) as resp:
        meta = json.loads(resp.read().decode('utf-8'))

    fobj = meta.get('file', meta)
    name = fobj['name']
    uri = fobj['uri']

    for _ in range(30):
        st_req = urllib.request.Request(
            f'https://generativelanguage.googleapis.com/v1beta/{name}',
            headers={'x-goog-api-key': api_key},
        )
        with urllib.request.urlopen(st_req, timeout=20) as resp:
            info = json.loads(resp.read().decode('utf-8'))
        state = info.get('state', '')
        if state == 'ACTIVE':
            return name, uri
        if state == 'FAILED':
            raise RuntimeError(f'Gemini file processing failed: {info}')
        time.sleep(1.5)
    raise RuntimeError('Timed out waiting for uploaded video to become ACTIVE')

def delete_uploaded(api_key: str, file_name: str):
    if not file_name:
        return
    try:
        req = urllib.request.Request(
            f'https://generativelanguage.googleapis.com/v1beta/{file_name}',
            headers={'x-goog-api-key': api_key},
            method='DELETE',
        )
        urllib.request.urlopen(req, timeout=15).close()
    except Exception:
        pass

def main():
    if len(sys.argv) < 4:
        print('Usage: python jury_review.py <mp4_path> <iteration_label> <out_md_path>')
        sys.exit(2)

    mp4_path = os.path.abspath(sys.argv[1])
    iter_label = sys.argv[2]
    out_md = os.path.abspath(sys.argv[3])
    api_key = load_api_key()

    with tempfile.TemporaryDirectory() as tmpdir:
        proxy_mp4 = os.path.join(tmpdir, 'jury_proxy_720p.mp4')
        subprocess.run([
            'ffmpeg', '-y', '-i', mp4_path,
            '-vf', 'scale=1280:720', '-r', '30',
            '-c:v', 'libx264', '-preset', 'veryfast', '-crf', '24', '-pix_fmt', 'yuv420p',
            '-c:a', 'aac', '-b:a', '128k',
            proxy_mp4,
        ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print(f'[JURY] Created 720p proxy ({os.path.getsize(proxy_mp4)} B), uploading...')

        file_name = None
        try:
            file_name, file_uri = upload_file_resumable(api_key, proxy_mp4, 'video/mp4')
            prompt = f"""You are a tough 3-judge Google Startup Buildathon Evaluator Panel (1: Google AI / Gemini Principal Architect, 2: Indian Fintech Risk & Compliance Lead, 3: Sequoia / Surge Demo Partner) evaluating "{iter_label}" of the AegisMesh product demo video (which must stay under 3 minutes).

Watch and listen carefully to the entire video — evaluate it strictly against the official Google Startup Buildathon rubric AND verify these specific creator requirements:
- Problem Statement & Market Urgency (20%): Does it clearly open with the painful problem (prompt injections draining balances, raw Aadhaar/PAN leaks under DPDP penalties up to ₹250 Cr, RBI ₹5,000 cap breaches) before presenting AegisMesh as the solution?
- Innovation & Google Gemini Integration (30%): Are Gemini Decision One (0-token inline guard), Gemini 3.5 Flash-Lite, and Async Gemini 3 Flash-Preview deeply integral rather than a thin wrapper?
- Live Product Demo & Technical Proof (25%): Does it clearly show a live typed prompt-injection attack getting blocked alongside real product UI screens, Maker-Checker human override, and verifiable Rust microsecond performance?
- Visual Craft, Immersion & Unicolour Readability (15%): Cohesive theme, multi-method 3D/canvas/Bento visual variety, crystal legibility, and ZERO deployment pitch (.exe / Cloud Run / Mumbai) at the ending?
- Voice Quality, Audio Mix & Pacing (10%): Warm, natural human-sounding studio narration clearly sitting above quiet non-distracting background music, with synchronized tactile SFX and zero dead air?

Output this exact Markdown structure:

## {iter_label} — Google Buildathon Evaluator Scorecard
| Evaluator Pillar | Weight | Score (1-10) | Crisp Rationale |
| --- | --- | --- | --- |
| Problem Statement & Urgency | 20% | X.X | ... |
| Innovation & Gemini Integration | 30% | X.X | ... |
| Live Demo & Technical Proof | 25% | X.X | ... |
| Visual Craft & Immersion | 15% | X.X | ... |
| Voice, Audio Mix & Pacing | 10% | X.X | ... |
| **Weighted Buildathon Score** | **100%** | **X.X / 10** | ... |

**Requirement Compliance Check:** Explicitly confirm `<3m runtime`, `Zero deployment pitch in ending`, `Clear live typed attack demo`, and `Voice clarity vs background music balance`.

## Timestamped Evaluator Critiques & Concrete Improvements
List 5 specific, timestamped (`MM:SS–MM:SS`) weaknesses ordered from highest impact to lowest impact, each with an exact actionable visual, animation, timing, or audio fix."""

            text = None
            for model in ['gemini-3.5-flash-lite', 'gemini-3-flash-preview', 'gemini-3.1-flash-lite']:
                url = f'https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent'
                payload = {
                    'contents': [{
                        'role': 'user',
                        'parts': [
                            {'fileData': {'fileUri': file_uri, 'mimeType': 'video/mp4'}},
                            {'text': prompt},
                        ],
                    }],
                    'generationConfig': {'temperature': 0.25, 'maxOutputTokens': 3072},
                }
                req = urllib.request.Request(
                    url,
                    data=json.dumps(payload).encode('utf-8'),
                    headers={'x-goog-api-key': api_key, 'Content-Type': 'application/json'},
                    method='POST',
                )
                try:
                    with urllib.request.urlopen(req, timeout=90) as resp:
                        body = json.loads(resp.read().decode('utf-8'))
                    parts = body['candidates'][0]['content']['parts']
                    text = ''.join(p.get('text', '') for p in parts if not p.get('thought'))
                    print(f'[JURY] Evaluated by {model}')
                    break
                except urllib.error.HTTPError as e:
                    print(f'[JURY RETRY] {model} HTTP {e.code}: {e.read().decode("utf-8", errors="replace")[:200]}')
                except Exception as e:
                    print(f'[JURY RETRY] {model}: {e}')

            if not text:
                raise RuntimeError('All Gemini models failed during Jury review')

            with open(out_md, 'w', encoding='utf-8') as f:
                f.write(text.strip() + '\n')
            print(text.strip())
        finally:
            if file_name:
                delete_uploaded(api_key, file_name)

if __name__ == '__main__':
    main()
