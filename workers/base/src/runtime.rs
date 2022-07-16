//! Foca runtime for tokio.

use std::any::Any;
use std::num::NonZeroU32;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

use bincode::DefaultOptions;
use derivative::Derivative;
use foca::{BincodeCodec, Config, Foca, NoCustomBroadcast, Notification, Runtime, Timer};
use futures::StreamExt;
use rand::prelude::StdRng;
use rand::SeedableRng;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

use sg_core::utils::ScopedJoinHandle;

use crate::compression::{compress, decompress};
use crate::ident::ID;
use crate::transport::{GossipSink, GossipStream};

/// Foca type instantiated with crate-specific type parameters.
type ConcreteFoca = Foca<ID, BincodeCodec<DefaultOptions>, StdRng, NoCustomBroadcast>;

/// Runtime events.
#[derive(Derivative)]
#[derivative(Debug)]
enum Input {
    /// Timed event.
    Event(Timer<ID>),
    /// Incoming data.
    Data(Vec<u8>),
    /// Announce to a node.
    Announce(ID),
    /// Execute a closure on foca instance.
    Closure(#[derivative(Debug = "ignore")] Box<dyn FnOnce(&mut ConcreteFoca) + Send + 'static>),
}

/// Wrapper for channel to foca runtime.
#[derive(Debug, Clone)]
struct FocaSender(UnboundedSender<Input>);

impl FocaSender {
    pub fn do_with<F, O>(&self, f: F)
    where
        F: FnOnce(&mut ConcreteFoca) -> O + Send + 'static,
    {
        self.0
            .send(Input::Closure(Box::new(move |foca| {
                // TODO catch unwind
                f(foca);
            })))
            .expect("Foca is dead");
    }
    pub async fn with<F, O>(&self, f: F) -> Box<O>
    where
        F: FnOnce(&mut ConcreteFoca) -> O + Send + 'static,
        O: Any + Send,
    {
        let (tx, rx) = oneshot::channel();
        self.0
            .send(Input::Closure(Box::new(move |foca| {
                // TODO catch unwind
                tx.send(Box::new(f(foca)) as Box<dyn Any + Send>)
                    .expect("Control thread is dead");
            })))
            .expect("Foca is dead");
        rx.await.unwrap().downcast().unwrap()
    }
}

impl Deref for FocaSender {
    type Target = UnboundedSender<Input>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FocaSender {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Tokio-based Foca runtime.
pub struct TokioFocaRuntime<Sink> {
    tx_foca: FocaSender,
    tx_notify: UnboundedSender<Notification<ID>>,
    sink: Sink,
}

impl<Sink> Runtime<ID> for TokioFocaRuntime<Sink>
where
    Sink: GossipSink<ID>,
{
    #[allow(clippy::missing_panics_doc)]
    fn notify(&mut self, notification: Notification<ID>) {
        // Update cluster config if member list changed.
        if matches!(
            notification,
            Notification::MemberUp(_) | Notification::MemberDown(_)
        ) {
            self.tx_foca.do_with(|foca| {
                let size = NonZeroU32::new(foca.num_members() as u32 + 1).unwrap();
                info!("Cluster config updated: {:?}", size);
                #[cfg(not(test))]
                drop(foca.set_config(Config::new_wan(size)));
            });
        }

        // Notify the main task.
        if self.tx_notify.send(notification).is_err() {
            warn!("Failed to send notification to main thread. Maybe ctl has been dropped.");
        }
    }

    fn send_to(&mut self, to: ID, data: &[u8]) {
        let data = match compress(data) {
            Ok(data) => data,
            Err(e) => {
                // Bail out. Don't panic here because gossip is resilient.
                error!("Unable to compress data: {}", e);
                return;
            }
        };

        // Spawn a new task to send data to the target node.
        debug!("Sending data of length {} to {:?}.", data.len(), to);
        let pool = self.sink.clone();
        tokio::task::spawn(async move {
            if let Err(e) = pool.send(to.clone(), data).await {
                warn!("Failed to send to {}: {}", to.addr(), e);
            }
        });
    }

    fn submit_after(&mut self, event: Timer<ID>, after: Duration) {
        let tx_foca = self.tx_foca.clone();
        tokio::task::spawn(async move {
            tokio::time::sleep(after).await;
            if tx_foca.send(Input::Event(event)).is_err() {
                warn!("Failed to send event to foca. Maybe ctl has been dropped.");
            }
        });
    }
}

/// Controller for Tokio-based Foca runtime.
pub struct TokioFocaCtl {
    /// Sender to foca task.
    tx_foca: FocaSender,
    /// Receiver from foca task.
    rx_notify: UnboundedReceiver<Notification<ID>>,
    // TODO change to broadcast or something else
    /// RAII handle for spawned tasks.
    _handle: (ScopedJoinHandle<()>, ScopedJoinHandle<()>),
}

impl TokioFocaCtl {
    /// Announce to a node to join a pre-existing cluster.
    pub fn announce(&self, id: ID) {
        self.tx_foca
            .send(Input::Announce(id))
            .expect("Foca is dead");
    }
    /// Receive notifications from the runtime.
    pub async fn recv(&mut self) -> Option<Notification<ID>> {
        // TODO can be changed to broadcast or something else?
        self.rx_notify.recv().await
    }
    /// Execute a closure on foca instance.
    pub async fn with<F, O>(&self, f: F) -> Box<O>
    where
        F: FnOnce(&mut ConcreteFoca) -> O + Send + 'static,
        O: Any + Send,
    {
        self.tx_foca.with(f).await
    }
}

/// Main entry point for Tokio-based Foca runtime.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn start_foca(
    id: ID,
    mut stream: impl GossipStream,
    sink: impl GossipSink<ID>,
    foca_config: impl Into<Option<Config>>,
) -> TokioFocaCtl {
    let config = foca_config
        .into()
        .unwrap_or_else(|| Config::new_wan(NonZeroU32::new(5).unwrap()));

    // Create foca instance.
    let mut foca = Foca::new(
        id,
        config,
        StdRng::from_entropy(),
        BincodeCodec(DefaultOptions::new()),
    );

    // Channels for inter-task communication.
    let (tx_foca, mut rx_foca) = unbounded_channel();
    let tx_foca = FocaSender(tx_foca);
    let (tx_notify, rx_notify) = unbounded_channel();

    // Instantiate runtime proxy.
    let mut foca_rt = TokioFocaRuntime {
        tx_foca: tx_foca.clone(),
        tx_notify,
        sink,
    };

    // Spawn foca task.
    let foca_handle = ScopedJoinHandle(tokio::spawn(async move {
        while let Some(input) = rx_foca.recv().await {
            if let Err(e) = match input {
                Input::Event(timer) => foca.handle_timer(timer, &mut foca_rt),
                Input::Data(data) => foca.handle_data(&data, &mut foca_rt),
                Input::Announce(id) => foca.announce(id, &mut foca_rt),
                Input::Closure(f) => {
                    f(&mut foca);
                    Ok(())
                }
            } {
                error!("Failed to handle input: {}", e);
            }
        }
    }));

    // Spawn packet receiver task.
    let income_handle = {
        let tx_foca = tx_foca.clone();
        ScopedJoinHandle(tokio::spawn(async move {
            while let Some(income) = stream.next().await {
                // Gossip packets should be small, so no need to spawn a blocking task (?)
                match decompress(&income) {
                    Ok(data) => tx_foca.send(Input::Data(data)).expect("Foca is dead"),
                    Err(e) => error!("Unable to handle packet: {}", e),
                }
            }
        }))
    };

    // Return the controller.
    TokioFocaCtl {
        _handle: (foca_handle, income_handle),
        tx_foca,
        rx_notify,
    }
}
