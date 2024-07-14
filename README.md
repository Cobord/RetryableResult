# Retryable Result

Three way enumeration with type for successful values, recoverable errors and nonrecoverable errors.
The recoverable errors must have a way to translate into nonrecoverable errors for the case of getting so many recoverable errors that is just time to give up.
The recoverable errors must also have a way to determine how long to wait given the information of what previous recoverable errors were received and when.
  For example, the policy might be to see a whole bunch of recoverable error messages that said some process was too busy and it says to wait twice as long
  before trying again in order to give whatever was being a bottleneck time to clear up.

# Try Repeatedly

We have an asynchronous function that besides the good results can return recovarable and nonrecoverable errors.

We have another asynchronous function which repeatedly tries this function until either
  - success
  - there are enough recoverable errors that the wait_time on Retryable says it is time to give up
  - a nonrecoverable error

There are loggers as well. Whenever the final result is an error either through a fatal error on a particular call or just too many recoverable errors, all that error information gets passed to the loggers.
