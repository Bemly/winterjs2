---
title: "WinterJS2.versions"
slug: WinterJS2/versions
---

The **`WinterJS2.versions`** property holds the versions table of the running
winterjs2 process (a snapshot of `process.versions` plus `winterjs2`).

## Syntax

```js
WinterJS2.versions
```

### Return value

- An object mapping component names (`node`, `winterjs2`, `mozjs`,
  `openssl`, `sqlite`, …) to their version strings.
