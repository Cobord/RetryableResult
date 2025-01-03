//! when we have something that returns both recoverable and nonrecoverable errors
//! as it's error values, try repeatedly until
//!     - success
//!     - there are enough recoverable errors that the `wait_time` on `Retryable` says it is time to give up
//!     - a fatal error

use crate::retryable::{Retryable, RetryableResult};
use std::{future::Future, time::Instant};

#[allow(dead_code)]
trait ArgType
where
    Self: Sized + Clone,
{
}

#[allow(clippy::needless_for_each)]
#[allow(dead_code)]
pub async fn repeatedly_try<
    SuccessType,
    RecoverableErr,
    FatalErr,
    ArgType,
    OneTryFun,
    FailLogContext,
    Fut0,
    FatalLoggerType,
    RecoverableLoggerType,
>(
    do_this_function: OneTryFun,
    arg: ArgType,
    loggers: (
        &mut FailLogContext,
        Option<FatalLoggerType>,
        Option<RecoverableLoggerType>,
    ),
) -> Result<SuccessType, FatalErr>
where
    RecoverableErr: Retryable<FatalError = FatalErr>,
    ArgType: Sized + Clone,
    OneTryFun: Fn(ArgType) -> Fut0,
    Fut0: Future<Output = RetryableResult<SuccessType, RecoverableErr, FatalErr>>,
    FatalLoggerType: Fn(&FatalErr, Instant, &mut FailLogContext),
    RecoverableLoggerType: Fn(&RecoverableErr, Instant, &mut FailLogContext),
{
    //! it calls `do_this_function` with the provided argument repeatedly until success or until the wait time is None
    //! when it is None, it means that we have reached our breaking point, there is no more waiting to re-call the function
    //!     that is we should just give up
    //! otherwise we are just repeatedly getting recoverable errors and we wait for some time determined by when
    //!     which recoverable errors we saw and when
    //! when the entire thing results in a fatal error the chain of recoverable errors and final fatal error
    //!     go into the logging functions
    //! # Errors
    //! when there are too many recoverable errors to a level of a breaking point or one of the steps gave a `FatalErr` directly
    let mut my_retriable_failures = Vec::<(RecoverableErr, Instant)>::with_capacity(5);
    loop {
        let cur_trial = do_this_function(arg.clone()).await;
        match cur_trial {
            RetryableResult::GoodResult(z) => {
                return Ok(z);
            }
            RetryableResult::Retryable(r) => {
                let this_time = Instant::now();
                if let Some(how_long_to_wait) = r.wait_time(this_time, &my_retriable_failures) {
                    my_retriable_failures.push((r, this_time));
                    async_std::task::sleep(how_long_to_wait).await;
                } else {
                    let (ctx, fatal_logger, recoverable_logger) = loggers;
                    if let Some(recoverable_logger) = recoverable_logger {
                        my_retriable_failures.iter().for_each(|(a, b)| {
                            recoverable_logger(a, *b, ctx);
                        });
                    }
                    let f = r.to_fatal();
                    if let Some(fatal_logger) = fatal_logger {
                        fatal_logger(&f, this_time, ctx);
                    }
                    return Err(f);
                }
            }
            RetryableResult::Fatal(f) => {
                let this_time = Instant::now();
                let (ctx, fatal_logger, recoverable_logger) = loggers;
                if let Some(recoverable_logger) = recoverable_logger {
                    my_retriable_failures.iter().for_each(|(a, b)| {
                        recoverable_logger(a, *b, ctx);
                    });
                }
                if let Some(fatal_logger) = fatal_logger {
                    fatal_logger(&f, this_time, ctx);
                }
                return Err(f);
            }
        }
    }
}

mod test {
    use crate::retryable::Retryable;
    use http::status::{InvalidStatusCode, StatusCode};

    #[repr(transparent)]
    struct RetryingStatusCode(StatusCode);

    impl RetryingStatusCode {
        #[allow(dead_code)]
        fn from_u16(u: u16) -> Result<RetryingStatusCode, InvalidStatusCode> {
            StatusCode::from_u16(u).map(Self)
        }
    }

    impl Retryable for RetryingStatusCode {
        type FatalError = StatusCode;

        fn to_fatal(self) -> Self::FatalError {
            self.0
        }

        fn wait_time(
            &self,
            my_time: std::time::Instant,
            previous_retriable_failures: &[(Self, std::time::Instant)],
        ) -> Option<std::time::Duration> {
            //! if we saw recoverable error twice, wait twice as long as the gap between the last two times
            //! for the next try
            //! exponential backoff
            //! if this was the first time a recoverable error happened, it waits 1 second for the 2nd try
            //! if this wait time gets to be greater than a minute then give up completely
            let default_duration = std::time::Duration::from_millis(1000);
            if let Some((_, last_time)) = previous_retriable_failures.last() {
                if let Some(last_two_gap) = my_time.checked_duration_since(*last_time) {
                    if last_two_gap > std::time::Duration::from_millis(30000) {
                        dbg!("Give up");
                        None
                    } else {
                        dbg!(last_two_gap * 2);
                        Some(last_two_gap * 2)
                    }
                } else {
                    dbg!(default_duration);
                    Some(default_duration)
                }
            } else {
                dbg!(default_duration);
                Some(default_duration)
            }
        }
    }

    #[allow(dead_code)]
    fn dummy_logger1(_error: &RetryingStatusCode, _time: std::time::Instant, _ctx: &mut ()) {}

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[allow(dead_code)]
    fn dummy_logger2(_error: &StatusCode, _time: std::time::Instant, _ctx: &mut ()) {}

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn first_test() {
        use super::repeatedly_try;
        use crate::retryable::RetryableResult;
        async fn one_try(u: u8) -> RetryableResult<u8, RetryingStatusCode, StatusCode> {
            #[allow(clippy::collapsible_else_if)]
            if u % 2 == 0 {
                RetryableResult::GoodResult(u >> 1)
            } else {
                if rand::random() {
                    RetryableResult::GoodResult(u >> 1)
                } else {
                    RetryableResult::Retryable(
                        RetryingStatusCode::from_u16(200).expect("200 is valid"),
                    )
                }
            }
        }
        let z = repeatedly_try(
            one_try,
            4,
            (&mut (), Some(dummy_logger2), Some(dummy_logger1)),
        )
        .await;
        assert_eq!(z, Ok(2));
        let z = repeatedly_try(
            one_try,
            3,
            (&mut (), Some(dummy_logger2), Some(dummy_logger1)),
        )
        .await;
        if z.is_ok() {
            assert_eq!(z, Ok(1));
        } else {
            assert_eq!(z, Err(StatusCode::from_u16(200).expect("200 is valid")));
        }
    }
}
