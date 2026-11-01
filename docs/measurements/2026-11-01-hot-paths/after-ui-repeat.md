aiditor GUI benchmark (individual events)

Scenario / metric | Samples | p50 | p95 | p99 | Max
baseline / input-to-paint | 120 | 24.332 ms | 27.611 ms | 28.418 ms | 29.569 ms
baseline / paint-gap | 119 | 74.388 ms | 77.901 ms | 78.470 ms | 79.613 ms
baseline: 0 terminal bytes processed during measurement
yes / input-to-paint | 120 | 8.402 ms | 16.767 ms | 16.953 ms | 17.233 ms
yes / paint-gap | 877 | 8.334 ms | 14.114 ms | 14.715 ms | 15.075 ms
yes: 160819643 terminal bytes processed during measurement
cat / input-to-paint | 120 | 9.016 ms | 16.906 ms | 17.436 ms | 22.977 ms
cat / paint-gap | 903 | 8.334 ms | 14.272 ms | 14.653 ms | 21.752 ms
cat: 177928348 terminal bytes processed during measurement

Synthetic scheduled input to matching frame paint; excludes OS keyboard delivery and display scanout.
Paint gaps include the input cadence and idle time; these are not GPU frame durations.
