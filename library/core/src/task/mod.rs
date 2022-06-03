#![stable(feature = "futures_api", since = "1.36.0")]

//! Types and Traits for working with asynchronous tasks.

mod poll;
mod spawn;
use crate::fmt;
use crate::marker::PhantomData;

#[stable(feature = "futures_api", since = "1.36.0")]
pub use self::poll::Poll;

mod wake;
#[stable(feature = "context_spawn", since = "1.63.0")]
pub use self::spawn::{
    JoinHandle, RawJoiner, RawJoinerVTable, RawSpawner, RawSpawnerVTable, Spawner,
};
#[stable(feature = "futures_api", since = "1.36.0")]
pub use self::wake::{RawWaker, RawWakerVTable, Waker};

mod ready;
#[unstable(feature = "ready_macro", issue = "70922")]
pub use ready::ready;
#[unstable(feature = "poll_ready", issue = "89780")]
pub use ready::Ready;

/// The `Context` of an asynchronous task.
///
/// Currently, `Context` only serves to provide access to a `&Waker`
/// which can be used to wake the current task.
#[stable(feature = "futures_api", since = "1.36.0")]
pub struct Context<'a> {
    waker: &'a Waker,
    spawner: Option<&'a Spawner>,
    // Ensure we future-proof against variance changes by forcing
    // the lifetime to be invariant (argument-position lifetimes
    // are contravariant while return-position lifetimes are
    // covariant).
    _marker: PhantomData<fn(&'a ()) -> &'a ()>,
}

impl<'a> Context<'a> {
    /// Create a new `Context` from a `&Waker`.
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[must_use]
    #[inline]
    pub fn from_waker(waker: &'a Waker) -> Self {
        Context { waker, spawner: None, _marker: PhantomData }
    }

    /// adds the spawner to the context
    #[stable(feature = "context_spawn", since = "1.63.0")]
    #[must_use]
    #[inline]
    pub fn with_spawner(mut self, spawner: &'a Spawner) -> Self {
        self.spawner = Some(spawner);
        self
    }

    /// Returns a reference to the `Waker` for the current task.
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[must_use]
    #[inline]
    pub fn waker(&self) -> &'a Waker {
        &self.waker
    }

    /// gets the spawner
    #[stable(feature = "context_spawn", since = "1.63.0")]
    pub fn spawner(&self) -> &'a Spawner {
        self.spawner.expect("This context should have a registered spawner")
    }
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("waker", &self.waker)
            .field("spawner", &self.spawner)
            .finish()
    }
}
