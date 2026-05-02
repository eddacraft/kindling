//! Claude Code hook handlers.
//!
//! Reads hook context from stdin, dispatches the appropriate observation
//! through `kindling-client`, returns the response JSON on stdout matching
//! the Node.js script contract byte-for-byte. Invoked as `kindling hook
//! <type>` by the umbrella binary.
//!
//! Filled in by PORT-009.
