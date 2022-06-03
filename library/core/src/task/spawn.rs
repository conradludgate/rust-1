#![stable(feature = "context_spawn", since = "1.63.0")]

use super::Context;
use super::Poll;
use crate::fmt;
use crate::future::Future;
use crate::marker::PhantomData;
use crate::marker::Unpin;
use crate::mem::ManuallyDrop;
use crate::pin::Pin;

/// A `RawJoiner` allows the implementor of a task executor to create a [`Spawner`]
/// which provides customized spawnup behavior.
///
/// [vtable]: https://en.wikipedia.org/wiki/Virtual_method_table
///
/// It consists of a data pointer and a [virtual function pointer table (vtable)][vtable]
/// that customizes the behavior of the `RawJoiner`.
#[derive(PartialEq, Debug)]
#[stable(feature = "context_spawn", since = "1.63.0")]
pub struct RawJoiner {
    /// A data pointer, which can be used to store arbitrary data as required
    /// by the executor. This could be e.g. a type-erased pointer to an `Arc`
    /// that is associated with the task.
    /// The value of this field gets passed to all functions that are part of
    /// the vtable as the first parameter.
    data: *const (),
    /// Virtual function pointer table that customizes the behavior of this spawner.
    vtable: &'static RawJoinerVTable,
}

impl RawJoiner {
    /// Creates a new `RawJoiner` from the provided `data` pointer and `vtable`.
    ///
    /// The `data` pointer can be used to store arbitrary data as required
    /// by the executor. This could be e.g. a type-erased pointer to an `Arc`
    /// that is associated with the task.
    /// The value of this pointer will get passed to all functions that are part
    /// of the `vtable` as the first parameter.
    ///
    /// The `vtable` customizes the behavior of a `Spawner` which gets created
    /// from a `RawJoiner`. For each operation on the `Spawner`, the associated
    /// function in the `vtable` of the underlying `RawJoiner` will be called.
    #[inline]
    #[rustc_promotable]
    #[stable(feature = "context_spawn", since = "1.63.0")]
    #[rustc_const_stable(feature = "context_spawn", since = "1.63.0")]
    #[must_use]
    pub const fn new(data: *const (), vtable: &'static RawJoinerVTable) -> RawJoiner {
        RawJoiner { data, vtable }
    }

    /// Get the `data` pointer used to create this `RawJoiner`.
    #[inline]
    #[must_use]
    #[unstable(feature = "spawner_getters", issue = "87021")]
    pub fn data(&self) -> *const () {
        self.data
    }

    /// Get the `vtable` pointer used to create this `RawJoiner`.
    #[inline]
    #[must_use]
    #[unstable(feature = "spawner_getters", issue = "87021")]
    pub fn vtable(&self) -> &'static RawJoinerVTable {
        self.vtable
    }
}

/// A virtual function pointer table (vtable) that specifies the behavior
/// of a [`RawJoiner`].
///
/// The pointer passed to all functions inside the vtable is the `data` pointer
/// from the enclosing [`RawJoiner`] object.
///
/// The functions inside this struct are only intended to be called on the `data`
/// pointer of a properly constructed [`RawJoiner`] object from inside the
/// [`RawJoiner`] implementation. Calling one of the contained functions using
/// any other `data` pointer will cause undefined behavior.
#[stable(feature = "context_spawn", since = "1.63.0")]
#[derive(PartialEq, Copy, Clone, Debug)]
pub struct RawJoinerVTable {
    /// This function will be called when `spawn_by_ref` is called on the [`Spawner`].
    /// It must spawn up the task associated with this [`RawJoiner`].
    ///
    /// This function is similar to `spawn`, but must not consume the provided data
    /// pointer.
    poll: unsafe fn(*const (), *mut (), *mut ()),

    /// This function gets called when a [`RawJoiner`] gets dropped.
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawJoiner`] and
    /// associated task.
    drop: unsafe fn(*const ()),
}

impl RawJoinerVTable {
    /// Creates a new `RawJoinerVTable` from the provided `clone`, `spawn`,
    /// `spawn_by_ref`, and `drop` functions.
    ///
    /// # `clone`
    ///
    /// This function will be called when the [`RawJoiner`] gets cloned, e.g. when
    /// the [`Spawner`] in which the [`RawJoiner`] is stored gets cloned.
    ///
    /// The implementation of this function must retain all resources that are
    /// required for this additional instance of a [`RawJoiner`] and associated
    /// task. Calling `spawn` on the resulting [`RawJoiner`] should result in a spawnup
    /// of the same task that would have been awoken by the original [`RawJoiner`].
    ///
    /// # `spawn`
    ///
    /// This function will be called when `spawn` is called on the [`Spawner`].
    /// It must spawn up the task associated with this [`RawJoiner`].
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawJoiner`] and
    /// associated task.
    ///
    /// # `spawn_by_ref`
    ///
    /// This function will be called when `spawn_by_ref` is called on the [`Spawner`].
    /// It must spawn up the task associated with this [`RawJoiner`].
    ///
    /// This function is similar to `spawn`, but must not consume the provided data
    /// pointer.
    ///
    /// # `drop`
    ///
    /// This function gets called when a [`RawJoiner`] gets dropped.
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawJoiner`] and
    /// associated task.
    #[rustc_promotable]
    #[stable(feature = "context_spawn", since = "1.63.0")]
    #[rustc_const_stable(feature = "context_spawn", since = "1.63.0")]
    pub const fn new(
        poll: unsafe fn(*const (), *mut (), *mut ()),
        drop: unsafe fn(*const ()),
    ) -> Self {
        Self { poll, drop }
    }
}

/// join handle
#[stable(feature = "context_spawn", since = "1.63.0")]
#[derive(Debug)]
pub struct JoinHandle<R> {
    joiner: RawJoiner,
    _marker: PhantomData<R>,
}

impl<R> JoinHandle<R> {
    /// Creates a new `Waker` from [`RawWaker`].
    ///
    /// The behavior of the returned `Waker` is undefined if the contract defined
    /// in [`RawWaker`]'s and [`RawWakerVTable`]'s documentation is not upheld.
    /// Therefore this method is unsafe.
    #[inline]
    #[must_use]
    #[stable(feature = "context_spawn", since = "1.63.0")]
    pub unsafe fn from_raw(joiner: RawJoiner) -> JoinHandle<R> {
        JoinHandle { joiner, _marker: PhantomData }
    }
}

#[stable(feature = "context_spawn", since = "1.63.0")]
impl<R> Future for JoinHandle<R> {
    type Output = R;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<R> {
        let mut poll = Poll::Pending;
        unsafe {
            (self.joiner.vtable.poll)(
                self.joiner.data,
                &mut poll as *mut Poll<R> as *mut (),
                cx as *mut Context<'_> as *mut (),
            )
        };
        poll
    }
}

/// A `RawSpawner` allows the implementor of a task executor to create a [`Spawner`]
/// which provides customized spawnup behavior.
///
/// [vtable]: https://en.wikipedia.org/wiki/Virtual_method_table
///
/// It consists of a data pointer and a [virtual function pointer table (vtable)][vtable]
/// that customizes the behavior of the `RawSpawner`.
#[derive(PartialEq, Debug)]
#[stable(feature = "context_spawn", since = "1.63.0")]
pub struct RawSpawner {
    /// A data pointer, which can be used to store arbitrary data as required
    /// by the executor. This could be e.g. a type-erased pointer to an `Arc`
    /// that is associated with the task.
    /// The value of this field gets passed to all functions that are part of
    /// the vtable as the first parameter.
    data: *const (),
    /// Virtual function pointer table that customizes the behavior of this spawner.
    vtable: &'static RawSpawnerVTable,
}

impl RawSpawner {
    /// Creates a new `RawSpawner` from the provided `data` pointer and `vtable`.
    ///
    /// The `data` pointer can be used to store arbitrary data as required
    /// by the executor. This could be e.g. a type-erased pointer to an `Arc`
    /// that is associated with the task.
    /// The value of this pointer will get passed to all functions that are part
    /// of the `vtable` as the first parameter.
    ///
    /// The `vtable` customizes the behavior of a `Spawner` which gets created
    /// from a `RawSpawner`. For each operation on the `Spawner`, the associated
    /// function in the `vtable` of the underlying `RawSpawner` will be called.
    #[inline]
    #[rustc_promotable]
    #[stable(feature = "context_spawn", since = "1.63.0")]
    #[rustc_const_stable(feature = "context_spawn", since = "1.63.0")]
    #[must_use]
    pub const fn new(data: *const (), vtable: &'static RawSpawnerVTable) -> RawSpawner {
        RawSpawner { data, vtable }
    }

    /// Get the `data` pointer used to create this `RawSpawner`.
    #[inline]
    #[must_use]
    #[unstable(feature = "spawner_getters", issue = "87021")]
    pub fn data(&self) -> *const () {
        self.data
    }

    /// Get the `vtable` pointer used to create this `RawSpawner`.
    #[inline]
    #[must_use]
    #[unstable(feature = "spawner_getters", issue = "87021")]
    pub fn vtable(&self) -> &'static RawSpawnerVTable {
        self.vtable
    }
}

/// A virtual function pointer table (vtable) that specifies the behavior
/// of a [`RawSpawner`].
///
/// The pointer passed to all functions inside the vtable is the `data` pointer
/// from the enclosing [`RawSpawner`] object.
///
/// The functions inside this struct are only intended to be called on the `data`
/// pointer of a properly constructed [`RawSpawner`] object from inside the
/// [`RawSpawner`] implementation. Calling one of the contained functions using
/// any other `data` pointer will cause undefined behavior.
#[stable(feature = "context_spawn", since = "1.63.0")]
#[derive(PartialEq, Copy, Clone, Debug)]
pub struct RawSpawnerVTable {
    /// This function will be called when the [`RawSpawner`] gets cloned, e.g. when
    /// the [`Spawner`] in which the [`RawSpawner`] is stored gets cloned.
    ///
    /// The implementation of this function must retain all resources that are
    /// required for this additional instance of a [`RawSpawner`] and associated
    /// task. Calling `spawn` on the resulting [`RawSpawner`] should result in a spawnup
    /// of the same task that would have been awoken by the original [`RawSpawner`].
    clone: unsafe fn(*const ()) -> RawSpawner,

    /// This function will be called when `spawn` is called on the [`Spawner`].
    /// It must spawn up the task associated with this [`RawSpawner`].
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawSpawner`] and
    /// associated task.
    spawn: unsafe fn(*const (), *const ()) -> RawJoiner,

    /// This function will be called when `spawn_by_ref` is called on the [`Spawner`].
    /// It must spawn up the task associated with this [`RawSpawner`].
    ///
    /// This function is similar to `spawn`, but must not consume the provided data
    /// pointer.
    spawn_by_ref: unsafe fn(*const (), *const ()) -> RawJoiner,

    /// This function gets called when a [`RawSpawner`] gets dropped.
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawSpawner`] and
    /// associated task.
    drop: unsafe fn(*const ()),
}

impl RawSpawnerVTable {
    /// Creates a new `RawSpawnerVTable` from the provided `clone`, `spawn`,
    /// `spawn_by_ref`, and `drop` functions.
    ///
    /// # `clone`
    ///
    /// This function will be called when the [`RawSpawner`] gets cloned, e.g. when
    /// the [`Spawner`] in which the [`RawSpawner`] is stored gets cloned.
    ///
    /// The implementation of this function must retain all resources that are
    /// required for this additional instance of a [`RawSpawner`] and associated
    /// task. Calling `spawn` on the resulting [`RawSpawner`] should result in a spawnup
    /// of the same task that would have been awoken by the original [`RawSpawner`].
    ///
    /// # `spawn`
    ///
    /// This function will be called when `spawn` is called on the [`Spawner`].
    /// It must spawn up the task associated with this [`RawSpawner`].
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawSpawner`] and
    /// associated task.
    ///
    /// # `spawn_by_ref`
    ///
    /// This function will be called when `spawn_by_ref` is called on the [`Spawner`].
    /// It must spawn up the task associated with this [`RawSpawner`].
    ///
    /// This function is similar to `spawn`, but must not consume the provided data
    /// pointer.
    ///
    /// # `drop`
    ///
    /// This function gets called when a [`RawSpawner`] gets dropped.
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawSpawner`] and
    /// associated task.
    #[rustc_promotable]
    #[stable(feature = "context_spawn", since = "1.63.0")]
    #[rustc_const_stable(feature = "context_spawn", since = "1.63.0")]
    pub const fn new(
        clone: unsafe fn(*const ()) -> RawSpawner,
        spawn: unsafe fn(*const (), *const ()) -> RawJoiner,
        spawn_by_ref: unsafe fn(*const (), *const ()) -> RawJoiner,
        drop: unsafe fn(*const ()),
    ) -> Self {
        Self { clone, spawn, spawn_by_ref, drop }
    }
}

/// A `Spawner` is a handle for waking up a task by notifying its executor that it
/// is ready to be run.
///
/// This handle encapsulates a [`RawSpawner`] instance, which defines the
/// executor-specific spawnup behavior.
///
/// Implements [`Clone`], [`Send`], and [`Sync`].
#[repr(transparent)]
#[stable(feature = "context_spawn", since = "1.63.0")]
pub struct Spawner {
    spawner: RawSpawner,
}

#[stable(feature = "context_spawn", since = "1.63.0")]
impl Unpin for Spawner {}
#[stable(feature = "context_spawn", since = "1.63.0")]
unsafe impl Send for Spawner {}
#[stable(feature = "context_spawn", since = "1.63.0")]
unsafe impl Sync for Spawner {}

impl Spawner {
    /// spawns stuff
    #[inline]
    #[stable(feature = "context_spawn", since = "1.63.0")]
    pub fn spawn<F: Future>(&self, task: F) -> JoinHandle<F::Output> {
        // The actual spawnup call is delegated through a virtual function call
        // to the implementation which is defined by the executor.
        let spawn = self.spawner.vtable.spawn;
        let data = self.spawner.data;

        // Don't call `drop` -- the spawner will be consumed by `spawn`.
        crate::mem::forget(self);

        let task = ManuallyDrop::new(task);

        // SAFETY: This is safe because `Spawner::from_raw` is the only way
        // to initialize `spawn` and `data` requiring the user to acknowledge
        // that the contract of `RawSpawner` is upheld.
        unsafe {
            let joiner = (spawn)(data, &task as &dyn Future as *const _ as *const ());
            JoinHandle::from_raw(joiner)
        }
    }
}

#[stable(feature = "context_spawn", since = "1.63.0")]
impl Clone for Spawner {
    #[inline]
    fn clone(&self) -> Self {
        Spawner {
            // SAFETY: This is safe because `Spawner::from_raw` is the only way
            // to initialize `clone` and `data` requiring the user to acknowledge
            // that the contract of [`RawSpawner`] is upheld.
            spawner: unsafe { (self.spawner.vtable.clone)(self.spawner.data) },
        }
    }
}

#[stable(feature = "context_spawn", since = "1.63.0")]
impl Drop for Spawner {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: This is safe because `Spawner::from_raw` is the only way
        // to initialize `drop` and `data` requiring the user to acknowledge
        // that the contract of `RawSpawner` is upheld.
        unsafe { (self.spawner.vtable.drop)(self.spawner.data) }
    }
}

#[stable(feature = "context_spawn", since = "1.63.0")]
impl fmt::Debug for Spawner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vtable_ptr = self.spawner.vtable as *const RawSpawnerVTable;
        f.debug_struct("Spawner")
            .field("data", &self.spawner.data)
            .field("vtable", &vtable_ptr)
            .finish()
    }
}
