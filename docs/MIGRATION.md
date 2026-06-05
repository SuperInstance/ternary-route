## Migrating from Binary

If you're used to binary routing (connected / disconnected), ternary adds a **congested** state — the $0$ where the path works but is slow.

| Binary | Ternary |
|--------|---------|
| Connected ($1$) | Fast path ($+1$) |
| Disconnected ($0$) | Default path ($0$) |
| | Slow path ($-1$) |

Binary routing treats any connected path as equal. Ternary distinguishes "fast" from "usable-but-slow," enabling traffic shaping, latency-aware failover, and graceful degradation without separate metric systems.

See **[From Binary to Ternary](https://github.com/SuperInstance/ternary-cookbook/blob/master/guides/FROM_BINARY.md)** for the full migration guide.
