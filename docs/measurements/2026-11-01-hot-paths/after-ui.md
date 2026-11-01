aiditor GUI benchmark (individual events)

Scenario / metric | Samples | p50 | p95 | p99 | Max
baseline / input-to-paint | 120 | 23.708 ms | 27.958 ms | 28.631 ms | 28.713 ms
baseline / paint-gap | 119 | 73.789 ms | 78.217 ms | 78.667 ms | 78.746 ms
baseline: 0 terminal bytes processed during measurement
yes / input-to-paint | 120 | 8.374 ms | 16.738 ms | 16.920 ms | 17.008 ms
yes / paint-gap | 875 | 8.334 ms | 14.033 ms | 14.421 ms | 15.136 ms
yes: 160641661 terminal bytes processed during measurement
cat / input-to-paint | 120 | 8.396 ms | 16.824 ms | 16.895 ms | 26.516 ms
cat / paint-gap | 889 | 8.334 ms | 14.186 ms | 14.785 ms | 25.083 ms
cat: 176786456 terminal bytes processed during measurement

Synthetic scheduled input to matching frame paint; excludes OS keyboard delivery and display scanout.
Paint gaps include the input cadence and idle time; these are not GPU frame durations.
