"""Run the Playwright CLI smoke script without Windows shell argument rewriting."""
from pathlib import Path
import os
import shutil
import subprocess
import sys

shim = shutil.which('playwright-cli')
if not shim:
    raise SystemExit('playwright-cli is required; start Vite on port 1426 first')
cli = Path(shim).parent / 'node_modules/@playwright/cli/playwright-cli.js'
command = ['node', str(cli)] if os.name == 'nt' and cli.exists() else [shim]
code = Path(__file__).with_name('posttool-ui-smoke.js').read_text(encoding='utf-8').strip().removesuffix(';')
result = subprocess.run(command + ['-s=posttool', 'run-code', code],
                        encoding='utf-8', errors='replace', stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
sys.stdout.buffer.write(result.stdout.encode('utf-8'))
sys.exit(result.returncode or (1 if '### Error' in result.stdout else 0))
