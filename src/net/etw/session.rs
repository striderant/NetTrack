//! The ETW session.
//!
//! A single real-time session is opened for the
//! `Microsoft-Windows-Kernel-Network` provider. Events arrive on the
//! provider's own thread and are forwarded over a bounded channel; the
//! channel is deliberately lossy, because a stalled UI thread must never
//! block the kernel trace buffer.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crossbeam_channel::{Receiver, TrySendError, bounded};
use ferrisetw::parser::Parser;
use ferrisetw::provider::Provider;
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::UserTrace;
use ferrisetw::EventRecord;

use crate::model::{Endpoint, Proto};
use crate::net::etw::events::{NetEvent, NetEventKind};

/// The `Microsoft-Windows-Kernel-Network` provider.
const KERNEL_NETWORK_PROVIDER: &str = "7dd42a49-5329-4832-8dfd-43d979153a88";

/// A fixed session name, so a new run can stop the session a crashed previous
/// run left behind in the kernel.
const SESSION_NAME: &str = "NetTrack-KernelNetwork";

/// The channel is bounded: if the consumer falls behind, events are dropped
/// rather than growing without limit.
const CHANNEL_CAPACITY: usize = 8192;

/// Whether the ETW layer is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtwStatus {
    /// The session is running.
    Active,
    /// The session could not be started; the reason is user visible.
    Unavailable(String),
}

/// A running ETW session.
pub struct EtwCollector {
    trace: Option<UserTrace>,
    receiver: Receiver<NetEvent>,
    /// Number of events dropped because the consumer fell behind.
    dropped: Arc<AtomicU64>,
    /// Number of events delivered.
    seen: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
}

impl EtwCollector {
    /// Start a session. A failure is not fatal: the monitor falls back to the
    /// socket table alone.
    pub fn start() -> (Self, EtwStatus) {
        let (sender, receiver) = bounded::<NetEvent>(CHANNEL_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        let seen = Arc::new(AtomicU64::new(0));
        let stopping = Arc::new(AtomicBool::new(false));

        let provider = Provider::by_guid(KERNEL_NETWORK_PROVIDER)
            .add_callback({
                let sender = sender.clone();
                let dropped = Arc::clone(&dropped);
                let seen = Arc::clone(&seen);
                move |record, schema_locator| {
                    seen.fetch_add(1, Ordering::Relaxed);
                    let Some(event) = decode(record, schema_locator) else {
                        return;
                    };
                    match sender.try_send(event) {
                        Ok(()) => {}
                        Err(TrySendError::Full(_)) => {
                            dropped.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(TrySendError::Disconnected(_)) => {}
                    }
                }
            })
            .build();

        // A session outlives the process that created it. Stopping any orphan
        // left behind by a crashed run is best effort; a failure just means
        // there was none.
        let _ = stop_orphan_session();

        let trace = match UserTrace::new()
            .named(SESSION_NAME.to_string())
            .enable(provider)
            .start_and_process()
        {
            Ok(trace) => trace,
            Err(e) => {
                return (
                    Self {
                        trace: None,
                        receiver,
                        dropped,
                        seen,
                        stopping,
                    },
                    EtwStatus::Unavailable(format!("{e:?}")),
                );
            }
        };

        (
            Self {
                trace: Some(trace),
                receiver,
                dropped,
                seen,
                stopping,
            },
            EtwStatus::Active,
        )
    }

    /// Take any events that have arrived since the last call.
    pub fn drain(&self) -> Vec<NetEvent> {
        self.receiver.try_iter().collect()
    }

    /// Wait for at least one event, up to `timeout`.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<NetEvent> {
        self.receiver.recv_timeout(timeout).ok()
    }

    /// Number of events dropped because the consumer fell behind.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Number of events seen in total.
    pub fn seen(&self) -> u64 {
        self.seen.load(Ordering::Relaxed)
    }

    /// Whether the session is still running.
    pub fn is_active(&self) -> bool {
        self.trace.is_some() && !self.stopping.load(Ordering::Relaxed)
    }
}

impl Drop for EtwCollector {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(trace) = self.trace.take() {
            let _ = trace.stop();
        }
    }
}

/// Stop a session left behind by a previous run.
fn stop_orphan_session() -> Result<(), ()> {
    ferrisetw::trace::stop_trace_by_name(SESSION_NAME).map_err(|_| ())
}

/// Decode a kernel network event.
///
/// The event id tells us both the protocol and the direction. Receive and
/// accept events are oriented from the peer towards the local host, so their
/// source and destination have to be swapped to get the local endpoint first.
fn decode(record: &EventRecord, schema_locator: &SchemaLocator) -> Option<NetEvent> {
    let event_id = record.event_id();

    let (kind, proto, incoming) = match event_id {
        // TCP IPv4 and IPv6.
        10 | 12 | 13 | 14 | 16 | 26 | 28 | 29 | 30 | 32 => (kind_for(event_id), Proto::Tcp, false),
        11 | 15 | 27 | 31 => (kind_for(event_id), Proto::Tcp, true),
        // UDP IPv4 and IPv6.
        42 | 58 => (kind_for(event_id), Proto::Udp, false),
        43 | 59 => (kind_for(event_id), Proto::Udp, true),
        _ => return None,
    };

    let schema = schema_locator.event_schema(record).ok()?;
    let parser = Parser::create(record, &schema);

    let pid: u32 = parser.try_parse("PID").ok()?;
    let source_ip: std::net::IpAddr = parser.try_parse("saddr").ok()?;
    let destination_ip: std::net::IpAddr = parser.try_parse("daddr").ok()?;
    // The MOF "Port" fields are stored in network byte order.
    let source_port: u16 = parser.try_parse("sport").ok()?;
    let destination_port: u16 = parser.try_parse("dport").ok()?;

    let source_port = u16::from_be(source_port);
    let destination_port = u16::from_be(destination_port);

    // The payload size is reported as "size" for data events; older manifests
    // spell it "datalen".
    let size: u64 = parser
        .try_parse("size")
        .or_else(|_| parser.try_parse("datalen"))
        .unwrap_or(0);

    let (local, remote) = if incoming {
        (
            Endpoint::new(destination_ip, destination_port),
            Endpoint::new(source_ip, source_port),
        )
    } else {
        (
            Endpoint::new(source_ip, source_port),
            Endpoint::new(destination_ip, destination_port),
        )
    };

    Some(NetEvent {
        kind,
        proto,
        pid,
        local,
        remote,
        size,
        is_ipv6: local.addr.is_ipv6(),
    })
}

/// Map a kernel network event id onto a kind.
fn kind_for(event_id: u16) -> NetEventKind {
    match event_id {
        // Connect.
        12 | 28 => NetEventKind::Connect,
        // Accept.
        15 | 31 => NetEventKind::Accept,
        // Disconnect.
        13 | 29 => NetEventKind::Disconnect,
        // Retransmit.
        14 | 30 => NetEventKind::Retransmit,
        // Send.
        10 | 26 | 42 | 58 => NetEventKind::Send,
        // Receive.
        11 | 27 | 43 | 59 => NetEventKind::Receive,
        _ => NetEventKind::Send,
    }
}
