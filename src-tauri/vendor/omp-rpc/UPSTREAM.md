# Vendored OMP RPC client

Source: https://github.com/can1357/oh-my-pi/tree/d6bc0b76eb82ed0dbd3eeffd8de7309f02c22a81/sdk/rust/omp-rpc

MIT license, copied in LICENSE. Generated wire types are unchanged.

Agent Dock adds `ClientOptions.defer_negotiation` so startup extension dialogs
can be answered before ordinary RPC commands are processed, `request_value`
for the desktop's validated command bridge, and shared disconnect/closed access.
All transport framing and response correlation continue to use upstream code.

Negotiated protocol version is atomic so deferred negotiation also updates the
public client state. Transport tests initialize the new option; Unix-only smoke
tests are gated on Unix for Windows builds.

Spawned processes retain a bounded 16 KiB stderr tail. Startup errors include
that diagnostic, and `stderr_tail()` exposes it after a later disconnect.
