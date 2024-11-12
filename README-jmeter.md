test_service
============

## Introduction

A test server providing various blocking functions.

We use `require 'async_grpc_patch'` to enable/disable the async event loop patch.

### Server configuration

The server is configured to use 5 threads for the thread based server and 5 async tasks per thread for the async server.

| Config   | Threads | Async Tasks |
|----------|---------|-------------|
| Threaded | 5       | 0           |
| Async    | 5       | 5           |

NB: "Threads" refers to GRPC server threads. "Async Tasks" refers to the max number of async tasks allowed per thread.

### Testing methodology

We use JMeter to test the server. The JMeter test plan is in `test_service.jmx`. We shall use the same thread group
configuration for each test:

| Number of threads | Ramp-up period | Loop count |
|-------------------|----------------|------------|
| 20                | 2              | 50         |

The number of users is greater than both threads and async tasks to ensure that the server is fully loaded.

Since GRPC does not block requests when busy (rather it immediately returns a "BUSY" error), we need to record
the error rate as well as throughput.

This will give us some idea about the performance of the server under load.

## CPU

The first test explores performance of a CPU bound API. The API calculates fib(30) on each request.

### JMeter Test Results

| Th/Async | Samples | Average | Min | Max  | Std. Dev. | Error % | Throughput | Received KB/sec | Sent KB/sec | Avg. Bytes |
|----------|---------|---------|-----|------|-----------|---------|------------|-----------------|-------------|------------|
| Thread   | 1000    | 1355    | 73  | 1519 | 224.68    | 0.0     | 13.91      | 0.26            | 0.0         | 19.0       |
| Async    | 1000    | 681     | 0   | 2584 | 514.75    | 0.271   | 19.46      | 0.39            | 0.0         | 20.391     |

### memory_profiler results

```shell
bundler exec ruby-memory-profiler --no-detailed --pretty run  -- ruby ./test_service.rb
```

| Th/Async | Total allocated           | Total retained          |
|----------|---------------------------|-------------------------|
| Thread   | 72.82 MB (577854 objects) | 9.06 MB (55551 objects) |
| Async    | 74.83 MB (589268 objects) | 9.15 MB (56234 objects) | 

### Results

IMO the results for async and threaded are similar. The async server has a 40% higher throughput but also a 27% higher
error rate (due to backpressure), which might be expected as the Fiber in a CPU bound task is not yielding to the event loop.

## IO (Database)

Inserts some rows, selects a COUNT(*) with a delay introduced using `pg_sleep()`

### JMeter Test Results

| Th/Async | Samples | Average | Min | Max | Std. Dev. | Error % | Throughput | Received KB/sec | Sent KB/sec | Avg. Bytes |
|----------|---------|---------|-----|-----|-----------|---------|------------|-----------------|-------------|------------|
| Thread   | 1000    | 64      | 0   | 362 | 119.5     | 0.774   | 67.35      | 1.57            | 0.0         | 23.87      |    
| Async    | 1000    | 163     | 0   | 564 | 158.48    | 0.475   | 52.94      | 1.22            | 0.0         | 23.553     |

### memory_profiler results

| Th/Async | Total allocated             | Total retained           |
|----------|-----------------------------|--------------------------|
| Thread   | 99.60 MB (897146 objects)   | 9.56 MB (59588 objects)  |
| Async    | 174.39 MB (1721271 objects) | 10.48 MB (66735 objects) | 

### Results

- The threaded server has a higher number of backpressure responses ("Error %") than the async server.
- The async server has a lower overall throughput than the threaded server.

JMeter is counting backpressure responses as errors, which makes it difficult to calculate
the exact throughput.


