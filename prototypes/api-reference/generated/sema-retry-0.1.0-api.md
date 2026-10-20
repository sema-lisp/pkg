# sema-retry 0.1.0 API

Retry with exponential backoff, circuit breaker, and rate limiting

Release tarball SHA-256: `ea1f177349994e1cfc0684dfc796063ac389a583448f6b1a574c9b4595dcf82e`
Entrypoint: `package.sema` (SHA-256 `031c56d38f481a90d742e50053d53300c45fc4f89226f61f6ae0b52dc2b0e2bf`)

## Exported API

### `retry/with-retry`

```sema
(retry/with-retry thunk . opts)
```

Run thunk, retrying failures with exponential backoff; return its value.
Options: :max-attempts (3), :initial-delay ms (200), :max-delay ms (10000),
:multiplier (2.0), :jitter fraction (0.2), :retry-if (fn (err) bool),
:on-retry (fn (attempt err delay)), :sleep-fn (for testing).
After :max-attempts failures re-raises as "retry: giving up after N attempts — <msg>".

Source: [package.sema:73](/packages/sema-retry/0.1.0/source/package.sema#L73)

### `retry/backoff-delays`

```sema
(retry/backoff-delays opts)
```

Return the list of pre-jitter delays (ms) the retry policy would sleep between attempts.
Pure: length is (- :max-attempts 1); jitter is ignored (it is randomized at runtime).

Source: [package.sema:81](/packages/sema-retry/0.1.0/source/package.sema#L81)

### `retry/circuit-breaker`

```sema
(retry/circuit-breaker . opts)
```

Create a circuit breaker (closed → open → half-open state machine).
Options: :failure-threshold (5) consecutive failures to trip, :reset-timeout ms
(30000) before an open breaker admits a probe, :half-open-max (1) concurrent
probes, :clock-fn (for testing; defaults to time-ms). Use with retry/with-breaker.

Source: [package.sema:94](/packages/sema-retry/0.1.0/source/package.sema#L94)

### `retry/with-breaker`

```sema
(retry/with-breaker breaker thunk)
```

Run thunk through the breaker; return its value.
Closed: failures count toward the threshold. Open: raises "retry: circuit open"
immediately until :reset-timeout elapses. Half-open: admits :half-open-max probes;
a probe success closes the breaker, a probe failure re-opens it.

Source: [package.sema:144](/packages/sema-retry/0.1.0/source/package.sema#L144)

### `retry/breaker-state`

```sema
(retry/breaker-state breaker)
```

Return the breaker's current state: :closed, :open, or :half-open.

Source: [package.sema:158](/packages/sema-retry/0.1.0/source/package.sema#L158)

### `retry/rate-limiter`

```sema
(retry/rate-limiter opts)
```

Create a token-bucket rate limiter allowing :rate calls per :per milliseconds.
Options: :rate (10), :per ms (1000), :clock-fn and :sleep-fn (for testing).
The bucket starts full (burst up to :rate) and refills continuously.
Use with retry/with-limit or retry/try-acquire!.

Source: [package.sema:166](/packages/sema-retry/0.1.0/source/package.sema#L166)

### `retry/with-limit`

```sema
(retry/with-limit limiter thunk)
```

Sleep until the limiter grants a token, then run thunk and return its value.

Source: [package.sema:204](/packages/sema-retry/0.1.0/source/package.sema#L204)

### `retry/try-acquire!`

```sema
(retry/try-acquire! limiter)
```

Take a token if one is available, without sleeping. Returns #t or #f.

Source: [package.sema:209](/packages/sema-retry/0.1.0/source/package.sema#L209)

### `retry/with-timeout`

```sema
(retry/with-timeout ms thunk)
```

Run thunk as an async task; raise an error if it isn't done within ms.
On expiry the task is cancelled and in-flight offloaded I/O (http, shell, llm)
is aborted. Sema's scheduler is cooperative: the thunk must yield (offloaded
I/O, await, channel ops, async/sleep) to be preemptable — pure CPU loops or a
blocking (sleep ...) cannot be interrupted.

Source: [package.sema:217](/packages/sema-retry/0.1.0/source/package.sema#L217)

## Provenance

- Extraction: prototype fixture parser; production proposal requires sema-reader AST parsing
- README content is not used as API documentation.
