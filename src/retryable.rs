use std::time::{Duration, Instant};

#[allow(clippy::module_name_repetitions)]
pub trait Retryable
where
    Self: Sized,
{
    //! put the logic of how to handle recoverable errors into the `wait_time` function
    //! one of the tests shows the pattern of exponential backoff with a hard cutoff
    //! but it does not take into account what the recoverable errors were
    //! another implementation of this trait might look to see if the same recoverable error
    //! was the common cause and decide to give up if it that is the case
    type FatalError;
    fn to_fatal(self) -> Self::FatalError;
    fn wait_time(
        &self,
        my_time: Instant,
        previous_retriable_failures: &[(Self, Instant)],
    ) -> Option<Duration>;
}

#[allow(clippy::module_name_repetitions)]
#[allow(dead_code)]
pub enum RetryableResult<T, R, F>
where
    R: Retryable<FatalError = F> + Sized,
    T: Sized,
    F: Sized,
{
    GoodResult(T),
    Retryable(R),
    Fatal(F),
}
