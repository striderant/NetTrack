//! Live network event capture via ETW.
//!
//! The `Microsoft-Windows-Kernel-Network` provider reports every packet that
//! crosses the TCP/IP stack together with the process that owns the socket.
//! Unlike the socket table, it also tells us the *peer* of a connectionless
//! UDP socket, which is otherwise impossible to observe.
//!
//! The provider is a modern, manifest-based one, so it can be enabled on any
//! supported Windows release without the kernel logger privilege dance that
//! the classic `TcpIp`/`UdpIp` providers need. Its events are self
//! describing: every field has a name, so there is no packed binary layout to
//! get wrong.

pub mod events;
pub mod session;

pub use events::{NetEvent, NetEventKind};
pub use session::{EtwCollector, EtwStatus};
