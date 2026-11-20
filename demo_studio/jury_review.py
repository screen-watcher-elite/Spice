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
            prompt = f"""You are a tough 3-judge Google Startup Buildathon Jury Panel (1: Principal Security Architect, 2: Apple/Linear Motion Director, 3: Sequoia/YC Partner) reviewing "{iter_label}" of the AegisMesh product demo video.

Watch the entire video carefully (every cut, transition, camera move, Light-Mode UI readability, unicolour studio background, cursor motion, kinetic typography, voiceover cadence, music, and SFX). Be brutally honest and constructive — do not flatter.

Provide your evaluation in this exact Markdown structure:

## {iter_label} — Google Buildathon Jury Scorecard
| Criterion | Score (1-10) | Crisp Rationale |
| --- | --- | --- |
| Hook & Serious Authority | X.X | ... |
| Pacing & Zero Idle Time | X.X | ... |
| Unicolour Visual Wow Factor | X.X | ... |
| Editing, Cuts & 3D Camera Craft | X.X | ... |
| Audio / VO / SFX Mix | X.X | ... |
| Buildathon Pitch Impact | X.X | ... |
| **Overall Jury Score** | **X.X / 10** | ... |

## Timestamped Jury Critiques & Concrete Fixes Needed
List 5 to 7 specific, timestamped (`MM:SS–MM:SS`) weaknesses ordered from highest impact to lowest impact, each with an exact actionable visual, timing, or audio fix for our motion graphics stage or audio mix."""

            text = None
            for model in ['gemini-3-flash-preview', 'gemini-3.5-flash-lite', 'gemini-3.1-flash-lite']:
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
