# Development Notes

## Best Practices
1. Use atomic operations for thread-safe state management instead of Mutex for boolean flags
2. Implement proper mutex guard dropping by using separate scopes
3. Use crossbeam channels for thread communication instead of tokio channels for better performance
4. Utilize parking_lot::Mutex instead of std::sync::Mutex for better performance

## Optimizations
1. MiMalloc provides better memory allocation performance
2. Core affinity improves thread scheduling
3. Spin sleep provides more accurate sleep timing

## Deprecated/Avoid
1. Avoid using std::thread::sleep in favor of spin_sleep for more accurate timing
2. Don't use tokio::sync::Mutex when parking_lot::Mutex is sufficient
3. Avoid holding mutex locks across await points

## Learnings
1. Atomic operations don't need ? operator as they return Result-like values directly
2. UI state should be managed separately from background task state
3. Core affinity should be set after spawning threads, not before 