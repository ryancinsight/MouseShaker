# Development Notes

## Best Practices

1. Use atomic operations for thread-safe state management instead of Mutex for boolean flags.
2. Implement proper mutex guard dropping by using separate scopes.
3. Use crossbeam channels for thread communication; one bounded channel per thread so each thread receives its own stop signal.
4. Use per-thread `Enigo` instances to avoid lock contention across worker threads.
5. Use `std::thread::spawn` for blocking loops; for periodic worker loops, prefer `recv_timeout` on stop channels so Stop can interrupt the wait.
6. Clamp interval configuration to an explicit bounded range (1..=3600 seconds).
7. Keep `JoinHandle`s for worker threads and join them on stop to reclaim thread resources deterministically.
8. Persist mutable runtime settings (intervals, tray behavior) so restarts preserve operator intent.

## Optimizations

1. `rpmalloc` provides better memory allocation performance and builds via clang on windows-gnu toolchains when `CC=clang` is set in `.cargo/config.toml`.
2. Core affinity improves thread scheduling.
3. `crossbeam_channel::Receiver::recv_timeout` reduces idle CPU usage versus active polling loops and improves stop responsiveness.
4. Removing tokio eliminates async runtime overhead for this workload.

## Deprecated/Avoid

1. Avoid active polling sleep loops when a blocking channel timeout can provide interruptible waits.
2. Avoid `tokio` when all tasks are blocking loops; use `std::thread::spawn`.
3. Avoid `mimalloc` and `snmalloc-rs` 0.7 on windows-gnu/clang: both require gcc or have cmake dependency bugs. Use `rpmalloc` with `CC=clang` in `.cargo/config.toml`.
4. Avoid wildcard `*` versions in Cargo.toml; pin to a minor version for reproducible builds.
5. Avoid sharing one crossbeam channel receiver between two threads via clone; provide a separate `bounded(1)` per thread.

## Learnings

1. Atomic operations don't need the `?` operator; they return the value directly.
2. UI state should be managed separately from background task state.
3. Core affinity should be set after spawning threads, not before.
4. `.cargo/config` is deprecated; use `.cargo/config.toml`.
