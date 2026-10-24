# Third-party notices

## multi-claude

Agent Dock's Claude account directory isolation, launch environment construction,
proxy precedence, and macOS keychain identity derivation are adapted from:

- Project: https://github.com/jakoes-wu/multi-claude
- Revision: `e3acd8b20746d31b47e0dc08cd7cb9768960311a`
- Sources: `src/multi_claude/env.py`, `launcher.py`, `identity.py`
- License: MIT

The original Python logic has been adapted to Rust, native Windows/macOS/Linux
processes, and Agent Dock's Vue account manager. No Python runtime or upstream
CLI installation is required.

MIT License

Copyright (c) 2026 multi-claude contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
