//! Module defining `JsExecutor` - an executor that tries running until stalled
//! on each animation frame callback call.

use crate::prelude::*;

use basegl::control::callback::CallbackHandle;
use basegl::control::EventLoopCallback;
use basegl::control::EventLoop;
use futures::task::LocalSpawn;
use futures::task::LocalFutureObj;
use futures::task::SpawnError;
use futures::executor::LocalPool;
use futures::executor::LocalSpawner;

/// Executor. Uses a single-threaded `LocalPool` underneath, relying on basegl's
/// `EventLoop` to do as much progress as possible on every animation frame.
#[derive(Debug)]
pub struct JSExecutor {
    /// Underlying executor. Shared internally with the event loop callback.
    executor    : Rc<RefCell<LocalPool>>,
    /// Executor's spawner handle.
    pub spawner : LocalSpawner,
    /// Event loop that calls us on each frame.
    event_loop  : Option<EventLoop>,
    /// Handle to the callback - if dropped, loop would have stopped calling us.
    /// Also owns a shared handle to the `executor`.
    cb_handle   : Option<CallbackHandle>,
}

impl JSExecutor {
    /// Creates a new JS Executor. It is not yet running, use `schedule_running`
    /// method to schedule it in an event loop.
    pub fn new() -> JSExecutor {
        let executor  = LocalPool::default();
        let spawner   = executor.spawner();
        let executor  = Rc::new(RefCell::new(executor));
        JSExecutor {
            executor,
            spawner,
            event_loop : None,
            cb_handle  : None,
        }
    }

    /// Creates a new JS executor with an event loop of its own. The event loop
    /// will live as long as this executor.
    pub fn new_running() -> JSExecutor {
        let mut executor   = JSExecutor::new();
        executor.schedule_running(EventLoop::new());
        executor
    }

    /// Returns a callback compatible with `EventLoop` that once called shall
    /// attempt achieving as much progress on this executor's tasks as possible
    /// without stalling.
    pub fn runner_callback(&self) -> impl EventLoopCallback {
        let executor = self.executor.clone();
        move |_| {
            // Safe, because this is the only place borrowing executor and loop
            // callback shall never be re-entrant.
            let mut executor = executor.borrow_mut();
            executor.run_until_stalled();
        }
    }

    /// Registers this executor to the given event's loop. From now on, event
    /// loop shall trigger this executor on each animation frame. To stop call
    /// `stop_running`.
    ///
    /// The executor will keep copy of this loop handle, so caller is not
    /// required to keep it alive.
    pub fn schedule_running(&mut self, event_loop:EventLoop) {
        let cb = self.runner_callback();

        self.cb_handle  = Some(event_loop.add_callback(cb));
        self.event_loop = Some(event_loop);
    }

    /// Stops event loop (previously assigned by `run` method) from calling this
    /// executor anymore. Does nothing if no loop was assigned. To resume call
    /// `schedule_running`.
    ///
    /// Drops the stored handle to the loop.
    pub fn stop_running(&mut self) {
        self.cb_handle  = None;
        self.event_loop = None;
    }
}

impl Default for JSExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalSpawn for JSExecutor {
    fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
        self.spawner.spawn_local_obj(future)
    }

    fn status_local(&self) -> Result<(), SpawnError> {
        self.spawner.status_local()
    }
}