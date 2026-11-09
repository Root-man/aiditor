aiditor GUI benchmark (individual events)

Scenario / metric | Samples | p50 | p95 | p99 | Max
baseline / input-to-paint | 120 | 23.808 ms | 27.499 ms | 28.481 ms | 28.977 ms
baseline / paint-gap | 119 | 73.900 ms | 77.739 ms | 78.521 ms | 79.006 ms
baseline: 0 terminal bytes processed during measurement
yes / input-to-paint | 120 | 16.564 ms | 17.056 ms | 17.722 ms | 28.125 ms
yes / paint-gap | 942 | 8.334 ms | 14.665 ms | 15.260 ms | 26.660 ms
yes: 166282611 terminal bytes processed during measurement
cat / input-to-paint | 120 | 16.422 ms | 17.021 ms | 17.215 ms | 17.293 ms
cat / paint-gap | 927 | 8.334 ms | 14.409 ms | 14.992 ms | 16.004 ms
cat: 392400499 terminal bytes processed during measurement

Synthetic scheduled input to matching frame paint; excludes OS keyboard delivery and display scanout.
Paint gaps include the input cadence and idle time; these are not GPU frame durations.
