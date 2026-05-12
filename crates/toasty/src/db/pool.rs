//! Connection pooling for database connections.

use deadpool::managed::{Object, Timeouts};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use toasty_core::driver::{Capability, Driver};
use tokio::{sync::Notify, task::JoinHandle};

use super::connection_task::{ConnectionHandle, ConnectionOperation};
use crate::engine::Engine;

/// Get the default maximum size of a pool, which is `cpu_core_count * 2`
/// including logical cores (Hyper-Threading).
fn get_default_pool_max_size() -> usize {
    deadpool::managed::PoolConfig::default().max_size
}

/// How long the sweep task waits for a `Ping` to come back before
/// treating the connection as dead. Short enough that one stuck
/// connection cannot stall the sweep loop; long enough to ride out a
/// normal round-trip on a busy database.
const DEFAULT_SWEEP_PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for connection pool behavior.
#[derive(Debug, Clone)]
pub(crate) struct PoolConfig {
    pub(crate) max_size: usize,
    pub(crate) timeouts: Timeouts,
    pub(crate) health_check_interval: Option<Duration>,
    pub(crate) pre_ping: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: get_default_pool_max_size(),
            timeouts: Default::default(),
            health_check_interval: Some(Duration::from_secs(60)),
            pre_ping: false,
        }
    }
}

/// A connection pool that manages database connections with background tasks.
#[derive(Debug)]
pub struct Pool {
    inner: deadpool::managed::Pool<Manager>,
    capability: &'static Capability,
    /// Handle for the background health-check sweep, if one was spawned.
    /// Aborted on `Pool::drop` so the task does not outlive the pool.
    sweep_task: Option<JoinHandle<()>>,
}

impl Drop for Pool {
    fn drop(&mut self) {
        if let Some(handle) = self.sweep_task.take() {
            handle.abort();
        }
    }
}

impl Pool {
    /// Creates a new connection pool from the given driver, engine, and
    /// configuration.
    pub(crate) fn new(
        driver: impl Driver,
        engine: Engine,
        config: PoolConfig,
    ) -> crate::Result<Self> {
        let capability = driver.capability();
        let driver_cap = driver.max_connections();

        let effective_max = match driver_cap {
            Some(cap) if cap < config.max_size => {
                tracing::warn!(
                    requested = config.max_size,
                    cap,
                    "driver caps max pool size below the requested value; using driver cap"
                );
                cap
            }
            _ => config.max_size,
        };

        let sweep_waker = Arc::new(SweepWaker::new());

        let inner = deadpool::managed::Pool::builder(Manager {
            driver: Box::new(driver),
            engine,
            sweep_waker: sweep_waker.clone(),
            pre_ping: config.pre_ping,
        })
        .runtime(deadpool::Runtime::Tokio1)
        .max_size(effective_max)
        .timeouts(config.timeouts)
        .build()
        .map_err(|e| {
            tracing::error!(error = %e, "failed to build connection pool");
            toasty_core::Error::connection_pool(e)
        })?;

        let sweep_task = config.health_check_interval.map(|interval| {
            let task = SweepTask {
                pool: inner.clone(),
                waker: sweep_waker,
                interval,
                last_serviced: 0,
            };
            tokio::spawn(task.run())
        });

        Ok(Self {
            inner,
            capability,
            sweep_task,
        })
    }

    /// Retrieves a connection from the pool.
    pub(crate) async fn get(&self, shared: Arc<super::Shared>) -> crate::Result<super::Connection> {
        let connection = self.inner.get().await.map_err(|e| {
            tracing::error!(error = %e, "failed to acquire connection from pool");
            toasty_core::Error::connection_pool(e)
        })?;
        Ok(super::Connection {
            inner: connection,
            shared,
        })
    }

    /// Returns the database driver this pool uses to create connections.
    pub fn driver(&self) -> &dyn Driver {
        self.inner.manager().driver.as_ref()
    }

    /// Returns the database driver's capabilities.
    pub fn capability(&self) -> &'static Capability {
        self.capability
    }

    /// Returns the current status of the pool, including the number of
    /// connections, how many are available, and how many waiters are queued.
    pub fn status(&self) -> PoolStatus {
        let s = self.inner.status();
        PoolStatus {
            max_size: s.max_size,
            size: s.size,
            available: s.available,
            waiting: s.waiting,
        }
    }
}

pub(super) struct Manager {
    driver: Box<dyn Driver>,
    engine: Engine,
    sweep_waker: Arc<SweepWaker>,
    pre_ping: bool,
}

impl std::fmt::Debug for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Manager")
            .field("driver", &self.driver)
            .finish()
    }
}

impl deadpool::managed::Manager for Manager {
    type Type = ConnectionHandle;
    type Error = crate::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        tracing::debug!("creating new pooled connection");
        let connection = self.driver.connect().await.inspect_err(|e| {
            tracing::error!(error = %e, "failed to create database connection");
        })?;
        Ok(ConnectionHandle::spawn(
            connection,
            self.engine.clone(),
            self.sweep_waker.clone(),
        ))
    }

    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &deadpool::managed::Metrics,
    ) -> deadpool::managed::RecycleResult<Self::Error> {
        if obj.in_tx.is_closed() || obj.is_finished() {
            tracing::debug!("discarding dead pooled connection");
            return Err(deadpool::managed::RecycleError::message(
                "background task is no longer running",
            ));
        }
        if self.pre_ping {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if obj.in_tx.send(ConnectionOperation::Ping { tx }).is_err() {
                tracing::debug!("pre-ping channel closed; discarding pooled connection");
                return Err(deadpool::managed::RecycleError::message(
                    "background task is no longer running",
                ));
            }
            match rx.await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    tracing::debug!(error = %err, "pre-ping failed; discarding pooled connection");
                    return Err(deadpool::managed::RecycleError::Backend(err));
                }
                Err(_) => {
                    tracing::debug!(
                        "pre-ping response channel dropped; discarding pooled connection"
                    );
                    return Err(deadpool::managed::RecycleError::message(
                        "background task exited during pre-ping",
                    ));
                }
            }
        }
        tracing::trace!("recycling pooled connection");
        Ok(())
    }
}

/// Bundles the sweep `Notify` with a monotonic request counter. A
/// user-task that observes connection-lost calls `wake`, which bumps
/// the counter *before* notifying. The sweep task compares the
/// post-notify counter against the snapshot taken at the start of its
/// most recent escalate to decide whether the notify has already been
/// covered by an in-flight or just-completed sweep.
pub(crate) struct SweepWaker {
    requests: AtomicU64,
    notify: Notify,
}

impl SweepWaker {
    fn new() -> Self {
        Self {
            requests: AtomicU64::new(0),
            notify: Notify::new(),
        }
    }

    /// Called when a caller observes `connection_lost` and wants the
    /// sweep to escalate. The counter bump happens-before the
    /// `notify_one`, so the sweep task is guaranteed to load a value
    /// at least as large as the one this caller produced.
    pub(crate) fn wake(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.notify.notify_one();
    }
}

/// Background task that periodically pings the longest-idle connection
/// and escalates to a full idle-pool sweep on failure (either a failing
/// periodic ping or a notify from a user-observed connection-lost).
struct SweepTask {
    pool: deadpool::managed::Pool<Manager>,
    waker: Arc<SweepWaker>,
    interval: Duration,
    /// Highest `waker.requests` value covered by a completed escalate.
    /// The notify branch skips when this is already ≥ the current
    /// counter, since some earlier escalate has already serviced every
    /// outstanding request.
    last_serviced: u64,
}

impl SweepTask {
    async fn run(mut self) {
        let mut ticker = tokio::time::interval(self.interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // `interval` fires immediately on first poll; skip that tick so
        // the first real ping happens one interval into the pool's life.
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.periodic_iteration().await;
                }
                _ = self.waker.notify.notified() => {
                    if self.waker.requests.load(Ordering::Relaxed) <= self.last_serviced {
                        // An earlier escalate (periodic-failure path or
                        // a prior notify) already covered every request
                        // outstanding when this notify was issued.
                        tracing::trace!("sweep notify already serviced; skipping");
                        continue;
                    }
                    tracing::debug!("sweep woken by observed connection_lost; escalating");
                    self.escalate().await;
                }
            }
        }
    }

    /// One periodic tick: pop the longest-idle connection, ping it,
    /// return it on success or drop it (and escalate) on failure.
    async fn periodic_iteration(&mut self) {
        if self.pool.status().available == 0 {
            return;
        }
        let Some(conn) = self.try_get_idle().await else {
            return;
        };
        if Self::ping_conn(&conn).await {
            // Healthy — drop returns the connection to the pool. A
            // successful ping has just touched the connection, so the
            // longest-idle selector naturally picks a different slot
            // on the next tick.
            drop(conn);
        } else {
            // Failed — detach the slot from the pool. The task has
            // already exited (respond closed the channel).
            let _ = Object::take(conn);
            self.escalate().await;
        }
    }

    /// Walk every currently-idle connection, ping each, drop the
    /// failures and return the healthy ones. Bounded by the snapshot
    /// of `status().available` at entry so a busy producer cannot keep
    /// the sweep looping indefinitely.
    async fn escalate(&mut self) {
        // Snapshot the request counter *before* any pings start. Any
        // user observation that arrives during the loop below
        // increments past this value and will trigger another escalate
        // on the next iteration; observations from before the snapshot
        // are covered by this pass.
        let snap = self.waker.requests.load(Ordering::Relaxed);

        let budget = self.pool.status().available;
        let mut healthy = Vec::with_capacity(budget);
        for _ in 0..budget {
            let Some(conn) = self.try_get_idle().await else {
                break;
            };
            if Self::ping_conn(&conn).await {
                healthy.push(conn);
            } else {
                let _ = Object::take(conn);
            }
        }
        // Healthy connections return to the pool when `healthy` drops.
        drop(healthy);

        self.last_serviced = snap;
    }

    /// Non-blocking acquire that only returns an existing idle slot —
    /// never creates a new connection. `wait = ZERO` makes the
    /// semaphore acquire non-blocking; if no permit is available
    /// (every slot is checked out by a user), `timeout_get` returns
    /// `Timeout(Wait)` and we skip.
    async fn try_get_idle(&self) -> Option<Object<Manager>> {
        let timeouts = Timeouts {
            wait: Some(Duration::ZERO),
            create: Some(Duration::ZERO),
            recycle: self.pool.timeouts().recycle,
        };
        self.pool.timeout_get(&timeouts).await.ok()
    }

    /// Send a `Ping` operation through the connection task, bounded by
    /// `DEFAULT_SWEEP_PING_TIMEOUT`. Returns `true` iff the ping reported a
    /// healthy connection.
    async fn ping_conn(handle: &ConnectionHandle) -> bool {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if handle.in_tx.send(ConnectionOperation::Ping { tx }).is_err() {
            return false;
        }
        match tokio::time::timeout(DEFAULT_SWEEP_PING_TIMEOUT, rx).await {
            Ok(Ok(Ok(()))) => true,
            Ok(Ok(Err(err))) => {
                tracing::debug!(error = %err, "sweep ping failed");
                false
            }
            Ok(Err(_)) => false, // connection task dropped tx
            Err(_) => {
                tracing::debug!("sweep ping timed out");
                false
            }
        }
    }
}

/// Snapshot of the pool's current state.
#[derive(Clone, Copy, Debug)]
pub struct PoolStatus {
    /// The maximum number of connections the pool will manage.
    pub max_size: usize,

    /// The current number of connections (both in-use and idle).
    pub size: usize,

    /// The number of idle connections ready to be checked out.
    pub available: usize,

    /// The number of tasks waiting for a connection to become available.
    pub waiting: usize,
}
