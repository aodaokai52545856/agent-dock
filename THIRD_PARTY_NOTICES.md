# Third-party notices

## highlight.js

OMP's conversation and tool output use highlight.js for syntax highlighting.

- Project: https://github.com/highlightjs/highlight.js
- Version: `11.12.0`
- License: BSD-3-Clause

BSD 3-Clause License

Copyright (c) 2006, Ivan Sagalaev.
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

* Redistributions of source code must retain the above copyright notice, this
  list of conditions and the following disclaimer.
* Redistributions in binary form must reproduce the above copyright notice,
  this list of conditions and the following disclaimer in the documentation
  and/or other materials provided with the distribution.
* Neither the name of the copyright holder nor the names of its
  contributors may be used to endorse or promote products derived from
  this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## CC Switch

Agent Dock's usage dashboard layout, metric strip, expandable token breakdown,
53-week calendar and quantile heatmap shading are adapted from CC Switch:

- Project: https://github.com/farion1231/cc-switch
- Version: `v4.0.21` (`c4db63c`)
- Sources: `src/components/usage/UsageDashboard.tsx`, `UsageHero.tsx`,
  `UsageHeatmap.tsx`, `UsageTrendChart.tsx`
- License: MIT

The interface is implemented in Vue and SVG against Agent Dock's local usage
API. CC Switch's React components and dependencies are not required.

MIT License

Copyright (c) 2025 Jason Young

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

## oh-my-pi Rust RPC client

- Project: https://github.com/can1357/oh-my-pi
- Revision: `d6bc0b76eb82ed0dbd3eeffd8de7309f02c22a81`
- Source: `sdk/rust/omp-rpc`, vendored at `src-tauri/vendor/omp-rpc`
- License: MIT; full license is included at `src-tauri/vendor/omp-rpc/LICENSE`.

The wire types and transport are reused with documented desktop-host additions
in `src-tauri/vendor/omp-rpc/UPSTREAM.md`.

## oh-my-pi icon

- Source: https://github.com/can1357/oh-my-pi/blob/d6bc0b76eb82ed0dbd3eeffd8de7309f02c22a81/assets/icon.svg
- License: MIT; full license is included at `src-tauri/vendor/omp-rpc/LICENSE`.

The official icon geometry and orange accents are retained. Its white foreground
uses the Dock theme's icon color so it remains visible in light and dark themes.

## postTool script runtime and protocol libraries

- QuickJS / quickjs-emscripten: https://github.com/justjake/quickjs-emscripten
  (`quickjs-emscripten-core` and `@jitl/quickjs-wasmfile-release-sync`).
  The runtime's complete copyright and MIT license notices are included below
  and in `docs/licenses/posttool-quickjs.txt`.
- Native protocol libraries: reqwest, Tokio, tokio-util, tokio-tungstenite,
  rumqttc, tonic, tonic-reflection, prost, prost-types, prost-reflect, protox,
  tokio-stream, rustls, cookie_store and futures-util. Exact versions are recorded
  in `src-tauri/Cargo.lock`; upstream license metadata is available on crates.io.

```text
quickjs-emscripten:
The MIT License

quickjs-emscripten copyright (c) 2019-2024 Jake Teton-Landis

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


quickjs:
QuickJS Javascript Engine

Copyright (c) 2017-2021 Fabrice Bellard
Copyright (c) 2017-2021 Charlie Gordon

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.

```
