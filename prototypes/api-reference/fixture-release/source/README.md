# sema-retry

Retry with exponential backoff, circuit breaker, and rate limiting.

Every `http/` and `llm/` call eventually hits a flaky network, a 429, or a
provider hiccup. `sema-retry` wraps those calls in resilience policies —
modeled on Clojure's [diehard](https://github.com/sunng87/diehard) — as plain
functions over thunks: no macros, no global state, everything testable with
injected clocks.

## Install

```bash
sema pkg add sema-retry
```

## Quick start

```sema
(import "sema-retry")

;; Retry a flaky HTTP call: 5 attempts, exponential backoff with jitter.
(retry/with-retry
  (fn () (http/get "https://api.example.com/data"))
  {:max-attempts 5 :initial-delay 200 :multiplier 2.0})

;; See exactly what the policy would sleep between attempts:
(retry/backoff-delays {:max-attempts 5 :initial-delay 200 :multiplier 2.0})
; => (200 400 800 1600)
```

## Retry with backoff

Wrap any thunk. On error the policy sleeps
`initial-delay * multiplier^(attempt-1)` ms (capped at `:max-delay`,
randomized by ±`:jitter`), then tries again — up to `:max-attempts` total
attempts, after which the last error is re-raised as
`"retry: giving up after N attempts — <original message>"`.

```sema
(retry/with-retry
  (fn () (llm/complete "Summarize this document..." {:model "gpt-5-mini"}))
  {:max-attempts 4
   :initial-delay 500
   :max-delay     8000
   ;; only retry rate-limit / transient errors
   :retry-if (fn (err) (string/contains? (get err :message) "429"))
   ;; log each retry
   :on-retry (fn (attempt err delay)
               (println "attempt " attempt " failed, retrying in " delay "ms"))})
```

## Circuit breaker

Stop hammering a service that is clearly down. A breaker starts `:closed`;
after `:failure-threshold` consecutive failures it trips `:open` and every
call raises `"retry: circuit open"` immediately (no thunk invocation). After
`:reset-timeout` ms it becomes `:half-open` and admits `:half-open-max` probe
calls — a probe success closes the breaker, a probe failure re-opens it.

```sema
(define api-breaker
  (retry/circuit-breaker {:failure-threshold 5 :reset-timeout 30000}))

(define (fetch-user id)
  (retry/with-breaker api-breaker
    (fn () (http/get (format "https://api.example.com/users/~a" id)))))

(retry/breaker-state api-breaker)   ; => :closed / :open / :half-open
```

Share one breaker per upstream service so all call sites see its health.

## Rate limiting

A token bucket allowing `:rate` calls per `:per` milliseconds. The bucket
starts full (bursts up to `:rate`) and refills continuously.

```sema
(define llm-limiter (retry/rate-limiter {:rate 10 :per 60000}))  ; 10/minute

;; Blocking: sleeps until a token is free, then runs.
(retry/with-limit llm-limiter
  (fn () (llm/complete prompt)))

;; Non-blocking: take a token only if one is available right now.
(if (retry/try-acquire! llm-limiter)
    (http/get url)
    (println "over budget, skipping"))
```

## Timeout

```sema
(retry/with-timeout 5000
  (fn () (http/get "https://slow.example.com/report")))
;; raises "async/timeout: operation timed out" after 5s
```

Runs the thunk as an async task via `async/timeout`; on expiry the task is
cancelled and in-flight offloaded I/O (`http/`, `shell`, `llm/` best-effort)
is aborted for real. Caveat: Sema's scheduler is cooperative, so the thunk is
only preemptable at yield points (offloaded I/O, `await`, channel ops,
`async/sleep`) — a pure CPU loop or a blocking `(sleep ...)` runs to
completion regardless of the timeout.

## Composing policies

Policies are just functions over thunks, so they nest:

```sema
(retry/with-limit llm-limiter
  (fn ()
    (retry/with-breaker api-breaker
      (fn ()
        (retry/with-retry
          (fn () (retry/with-timeout 10000 (fn () (http/get url))))
          {:max-attempts 3})))))
```

## API

| Function | Description |
|---|---|
| `(retry/with-retry thunk opts?)` | Run thunk, retrying failures with exponential backoff |
| `(retry/backoff-delays opts)` | Pure: the pre-jitter delay list a policy would produce |
| `(retry/circuit-breaker opts?)` | Create a circuit breaker (closed/open/half-open) |
| `(retry/with-breaker breaker thunk)` | Run thunk through a breaker |
| `(retry/breaker-state breaker)` | `:closed`, `:open`, or `:half-open` |
| `(retry/rate-limiter opts)` | Create a token-bucket rate limiter |
| `(retry/with-limit limiter thunk)` | Sleep until a token is available, then run thunk |
| `(retry/try-acquire! limiter)` | Take a token without sleeping; `#t`/`#f` |
| `(retry/with-timeout ms thunk)` | Run thunk async; raise if not done within ms |

### `retry/with-retry`

```sema
(retry/with-retry thunk)
(retry/with-retry thunk {:max-attempts 3 :initial-delay 200 :max-delay 10000
                         :multiplier 2.0 :jitter 0.2
                         :retry-if (fn (err) #t)
                         :on-retry (fn (attempt err delay) nil)})
```

Returns the thunk's value on the first success. Options (defaults shown):

- `:max-attempts` — total attempts including the first (`3`).
- `:initial-delay` — ms before the first retry (`200`).
- `:max-delay` — cap on any single delay, ms (`10000`).
- `:multiplier` — backoff factor per attempt (`2.0`).
- `:jitter` — each delay is randomized by ±this fraction (`0.2`); use `0` for
  exact delays.
- `:retry-if` — predicate on the caught error map; returning `#f` re-raises
  immediately without retrying (default: retry everything).
- `:on-retry` — called as `(fn (attempt err delay))` before each sleep.
- `:sleep-fn` — override the sleeper (for testing).

After `:max-attempts` failures raises
`"retry: giving up after N attempts — <original message>"`.

### `retry/backoff-delays`

```sema
(retry/backoff-delays {:max-attempts 5 :initial-delay 100 :multiplier 2.0 :max-delay 500})
; => (100 200 400 500)
```

Pure function: the list of pre-jitter delays (ms) `retry/with-retry` would
sleep between attempts — `(- max-attempts 1)` entries. Jitter is ignored here
(it is randomized at runtime). Useful for tests and docs.

### `retry/circuit-breaker`

```sema
(retry/circuit-breaker)
(retry/circuit-breaker {:failure-threshold 5 :reset-timeout 30000 :half-open-max 1})
```

Returns a breaker (a stateful closure). Options: `:failure-threshold`
consecutive failures to trip (`5`), `:reset-timeout` ms before an open breaker
admits a probe (`30000`), `:half-open-max` probes admitted while half-open
(`1`), `:clock-fn` override (for testing).

### `retry/with-breaker`

```sema
(retry/with-breaker breaker (fn () (http/get url)))
```

Runs the thunk if the breaker admits it and returns its value; records
success/failure on the breaker; errors from the thunk are re-raised. Raises
`"retry: circuit open"` without calling the thunk while the breaker is open.

### `retry/breaker-state`

```sema
(retry/breaker-state breaker)   ; => :closed
```

Reports `:half-open` once an open breaker's reset timeout has elapsed.

### `retry/rate-limiter`

```sema
(retry/rate-limiter {:rate 10 :per 1000})   ; 10 calls per second
```

Returns a limiter (a stateful closure). Options: `:rate` tokens per window
(`10`), `:per` window length in ms (`1000`), `:clock-fn` / `:sleep-fn`
overrides (for testing).

### `retry/with-limit`

```sema
(retry/with-limit limiter (fn () (http/get url)))
```

Sleeps until a token is available, takes it, runs the thunk, and returns its
value.

### `retry/try-acquire!`

```sema
(retry/try-acquire! limiter)   ; => #t or #f
```

Takes a token if one is available right now; never sleeps.

### `retry/with-timeout`

```sema
(retry/with-timeout 5000 (fn () (http/get url)))
```

Runs the thunk as an async task and raises if it isn't done within `ms`. The
timed-out task is cancelled (in-flight HTTP/shell I/O aborted). See the
cooperative-scheduling caveat under [Timeout](#timeout).

## Testing hooks

Every time-dependent construct accepts injectable time sources, documented for
testing only:

- `retry/with-retry` — `:sleep-fn` (called with each delay in ms).
- `retry/circuit-breaker` — `:clock-fn` (returns current ms; default `time-ms`).
- `retry/rate-limiter` — `:clock-fn` and `:sleep-fn`.

With a fake clock and a recording sleeper, the whole suite runs instantly and
deterministically — see `tests.sema` for `make-clock` / `make-sleeper`
examples. Note: a fake `:sleep-fn` given to `retry/rate-limiter` should also
advance the fake clock, or `retry/with-limit` would wait forever.

## Testing

```bash
sema pkg add sema-test   # once
sema tests.sema
```

## License

MIT
