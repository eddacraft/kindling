# PocketFlow Integration

## Overview

PocketFlow is a lightweight workflow engine that provides explicit node boundaries and execution structure. Kindling uses PocketFlow's node lifecycle events to create high-signal memory capsules.

## Source

- **Repository**: https://github.com/The-Pocket/PocketFlow-Typescript
- **License**: MIT
- **Version**: Vendored (latest at time of copy)

## License Compatibility

PocketFlow uses the MIT license, which is compatible with Kindling's Apache-2.0 license:

- MIT allows commercial use, modification, distribution
- MIT requires license notice preservation
- No copyleft requirements that conflict with Apache-2.0

## Integration Approach

**Decision**: Vendored (copied in)

**Rationale**:

- PocketFlow is designed for vendoring (their intended model)
- No external dependency at runtime
- Full control over version and modifications
- MIT license explicitly permits copying

## Vendored Location

`packages/kindling-adapter-pocketflow/vendor/pocketflow/`

## Update Procedure

1. Check PocketFlow repository for updates
2. Review changelog for breaking changes
3. Copy updated source to vendor directory
4. Update commit hash in this document
5. Run full test suite: `pnpm test`
6. Verify adapter functionality with sample workflow

## Attribution

PocketFlow is used under the MIT license. The license notice is preserved in the vendored copy at `packages/kindling-adapter-pocketflow/vendor/pocketflow/` and documented here for transparency.

```
MIT License

Copyright (c) The Pocket

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
```
