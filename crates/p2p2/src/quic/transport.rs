use std::{
	collections::{HashMap, HashSet},
	convert::Infallible,
	net::{Ipv4Addr, Ipv6Addr, SocketAddr},
	sync::{Arc, PoisonError, RwLock},
	time::Duration,
};

use flume::{bounded, Receiver, Sender};
use libp2p::{
	core::muxing::StreamMuxerBox,
	futures::{AsyncReadExt, AsyncWriteExt, StreamExt},
	swarm::SwarmEvent,
	StreamProtocol, Swarm, SwarmBuilder, Transport,
};
use libp2p_stream::Behaviour;
use tokio::{
	net::TcpListener,
	sync::{mpsc, oneshot},
	time::timeout,
};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, warn};

use crate::{
	identity::REMOTE_IDENTITY_LEN,
	quic::utils::{
		identity_to_libp2p_keypair, remote_identity_to_libp2p_peerid, socketaddr_to_quic_multiaddr,
	},
	ConnectionRequest, HookEvent, ListenerId, RemoteIdentity, UnicastStream, P2P,
};

const PROTOCOL: StreamProtocol = StreamProtocol::new("/sdp2p/1");

/// [libp2p::PeerId] for debugging purposes only.
#[derive(Debug)]
pub struct Libp2pPeerId(libp2p::PeerId);

#[derive(Debug)]
enum InternalEvent {
	RegisterListener {
		id: ListenerId,
		ipv4: bool,
		addr: SocketAddr,
		result: oneshot::Sender<Result<(), String>>,
	},
	UnregisterListener {
		id: ListenerId,
		ipv4: bool,
		result: oneshot::Sender<Result<(), String>>,
	},
}

/// Transport using Quic to establish a connection between peers.
/// This uses `libp2p` internally.
#[derive(Debug)]
pub struct QuicTransport {
	id: ListenerId,
	p2p: Arc<P2P>,
	internal_tx: Sender<InternalEvent>,
}

impl QuicTransport {
	/// Spawn the `QuicTransport` and register it with the P2P system.
	/// Be aware spawning this does nothing unless you call `Self::set_ipv4_enabled`/`Self::set_ipv6_enabled` to enable the listeners.
	// TODO: Error type here
	pub fn spawn(p2p: Arc<P2P>) -> Result<(Self, Libp2pPeerId), String> {
		let keypair = identity_to_libp2p_keypair(p2p.identity());
		let libp2p_peer_id = Libp2pPeerId(keypair.public().to_peer_id());

		let (tx, rx) = bounded(15);
		let (internal_tx, internal_rx) = bounded(15);
		let (connect_tx, connect_rx) = mpsc::channel(15);
		let id = p2p.register_listener("libp2p-quic", tx, move |listener_id, peer, _addrs| {
			// TODO: I don't love this always being registered. Really it should only show up if the other device is online (do a ping-type thing)???
			peer.listener_available(listener_id, connect_tx.clone());
		});

		let swarm = ok(ok(SwarmBuilder::with_existing_identity(keypair)
			.with_tokio()
			.with_other_transport(|keypair| {
				libp2p_quic::GenTransport::<libp2p_quic::tokio::Provider>::new(
					libp2p_quic::Config::new(keypair),
				)
				.map(|(p, c), _| (p, StreamMuxerBox::new(c)))
				.boxed()
			}))
		.with_behaviour(|_| Behaviour::new()))
		.with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
		.build();

		tokio::spawn(start(p2p.clone(), id, swarm, rx, internal_rx, connect_rx));

		Ok((
			Self {
				id,
				p2p,
				internal_tx,
			},
			libp2p_peer_id,
		))
	}

	// `None` on the port means disabled. Use `0` for random port.
	pub async fn set_ipv4_enabled(&self, port: Option<u16>) -> Result<(), String> {
		self.setup_listener(
			port.map(|p| SocketAddr::from((Ipv4Addr::UNSPECIFIED, p))),
			true,
		)
		.await
	}

	pub async fn set_ipv6_enabled(&self, port: Option<u16>) -> Result<(), String> {
		self.setup_listener(
			port.map(|p| SocketAddr::from((Ipv6Addr::UNSPECIFIED, p))),
			false,
		)
		.await
	}

	// TODO: Proper error type
	async fn setup_listener(&self, addr: Option<SocketAddr>, ipv4: bool) -> Result<(), String> {
		let (tx, rx) = oneshot::channel();
		let event = if let Some(mut addr) = addr {
			if addr.port() == 0 {
				#[allow(clippy::unwrap_used)] // TODO: Error handling
				addr.set_port(
					TcpListener::bind(addr)
						.await
						.unwrap()
						.local_addr()
						.unwrap()
						.port(),
				);
			}

			InternalEvent::RegisterListener {
				id: self.id,
				ipv4,
				addr,
				result: tx,
			}
		} else {
			InternalEvent::UnregisterListener {
				id: self.id,
				ipv4,
				result: tx,
			}
		};

		let Ok(_) = self.internal_tx.send(event) else {
			return Err("internal channel closed".to_string());
		};
		rx.await
			.map_err(|_| "internal response channel closed".to_string())
			.and_then(|r| r)
	}

	pub async fn shutdown(self) {
		self.p2p.unregister_hook(self.id.into()).await;
	}
}

fn ok<T>(v: Result<T, Infallible>) -> T {
	match v {
		Ok(v) => v,
		Err(_) => unreachable!(),
	}
}

async fn start(
	p2p: Arc<P2P>,
	id: ListenerId,
	mut swarm: Swarm<Behaviour>,
	rx: Receiver<HookEvent>,
	internal_rx: Receiver<InternalEvent>,
	mut connect_rx: mpsc::Receiver<ConnectionRequest>,
) {
	let mut ipv4_listener = None;
	let mut ipv6_listener = None;

	let mut control = swarm.behaviour().new_control();
	#[allow(clippy::unwrap_used)] // TODO: Error handling
	let mut incoming = control.accept(PROTOCOL).unwrap();
	let map = Arc::new(RwLock::new(HashMap::new()));

	loop {
		tokio::select! {
			Ok(event) = rx.recv_async() => match event {
				HookEvent::PeerExpiredBy(_, identity) => {
					println!("CHECKING {:?}", identity); // TODO

					let Some(peer) = p2p.peers.read().unwrap_or_else(PoisonError::into_inner).get(&identity).map(Clone::clone) else {
						continue;
					};

					let addrs = {
						let state = peer.state.read().unwrap_or_else(PoisonError::into_inner);

						state
							.discovered
							.values()
							.flatten()
							.cloned()
							.collect::<HashSet<_>>()
					};

					let peer_id = remote_identity_to_libp2p_peerid(&identity);

					let mut control = control.clone();
					tokio::spawn(async move {
						match timeout(Duration::from_secs(5), control.open_stream_with_addrs(
							peer_id,
							PROTOCOL,
							addrs.iter()
								.map(socketaddr_to_quic_multiaddr)
								.collect()
						)).await {
							Ok(Ok(_)) => {}
							Err(_) | Ok(Err(_)) => peer.disconnected_from(id),
						};
					});
				},
				HookEvent::Shutdown { _guard } => {
					let connected_peers = swarm.connected_peers().cloned().collect::<Vec<_>>();
					for peer_id in connected_peers {
						let _ = swarm.disconnect_peer_id(peer_id);
					}

					if let Some((id, _)) = ipv4_listener.take() {
						let _ = swarm.remove_listener(id);
					}
					if let Some((id, _)) = ipv6_listener.take() {
						let _ = swarm.remove_listener(id);
					}

					// TODO: We don't break the event loop so libp2p can be polled to keep cleaning up.
					// break;
				},
				_ => {},
			},
			Some((peer_id, mut stream)) = incoming.next() => {
				let p2p = p2p.clone();
				let map = map.clone();
				tokio::spawn(async move {
					let mut actual = [0; REMOTE_IDENTITY_LEN];
					match stream.read_exact(&mut actual).await {
						Ok(_) => {},
						Err(e) => {
							warn!("Failed to read remote identity with libp2p::PeerId({peer_id:?}): {e:?}");
							return;
						},
					}
					let identity = match RemoteIdentity::from_bytes(&actual) {
						Ok(i) => i,
						Err(e) => {
							warn!("Failed to parse remote identity with libp2p::PeerId({peer_id:?}): {e:?}");
							return;
						},
					};

					// We need to go `PeerId -> RemoteIdentity` but as `PeerId` is a hash that's impossible.
					// So to make this work the connection initiator will send their remote identity.
					// It is however untrusted as they could send anything, so we convert it to a PeerId and check it matches the PeerId for this connection.
					// If it matches, we are certain they own the private key as libp2p takes care of ensuring the PeerId is trusted.
					let remote_identity_peer_id = remote_identity_to_libp2p_peerid(&identity);
					if peer_id != remote_identity_peer_id {
						warn!("Derived remote identity '{remote_identity_peer_id:?}' does not match libp2p::PeerId({peer_id:?})");
						return;
					}
					map.write().unwrap_or_else(PoisonError::into_inner).insert(peer_id, identity);

					// TODO: Sync metadata
					let metadata = HashMap::new();

					let stream = UnicastStream::new(identity, stream.compat());
					let (shutdown_tx, shutdown_rx) = oneshot::channel();
					p2p.connected_to(
						id,
						metadata,
						stream,
						shutdown_tx,
					);

					debug!("established inbound stream with '{}'", identity);

					let _todo = shutdown_rx; // TODO: Handle `shutdown_rx`
				});
			},
			event = swarm.select_next_some() => if let SwarmEvent::ConnectionClosed { peer_id, num_established: 0, .. } = event {
					let Some(identity) = map.write().unwrap_or_else(PoisonError::into_inner).remove(&peer_id) else {
						warn!("Tried to remove a peer that wasn't in the map.");
						continue;
					};

					let peers = p2p.peers.read().unwrap_or_else(PoisonError::into_inner);
					let Some(peer) = peers.get(&identity) else {
						warn!("Tried to remove a peer that wasn't in the P2P system.");
						continue;
					};

					peer.disconnected_from(id);
			},
			Ok(event) = internal_rx.recv_async() => match event {
				InternalEvent::RegisterListener { id, ipv4, addr, result } => {
					match swarm.listen_on(socketaddr_to_quic_multiaddr(&addr)) {
						Ok(libp2p_listener_id) => {
							let this = match ipv4 {
								true => &mut ipv4_listener,
								false => &mut ipv6_listener,
							};
							// TODO: Diff the `addr` & if it's changed actually update it
							if this.is_none() {
								*this =  Some((libp2p_listener_id, addr));
								p2p.register_listener_addr(id, addr);
							}

							let _ = result.send(Ok(()));
						},
						Err(e) => {
							let _ = result.send(Err(e.to_string()));
						},
					}
				},
				InternalEvent::UnregisterListener { id, ipv4, result } => {
					let this = match ipv4 {
						true => &mut ipv4_listener,
						false => &mut ipv6_listener,
					};
					if let Some((addr_id, addr)) = this.take() {
						if swarm.remove_listener(addr_id) {
							p2p.unregister_listener_addr(id, addr);
						}
					}
					let _ = result.send(Ok(()));
				},
			},
			Some(req) = connect_rx.recv() => {
				let mut control = control.clone();
				let self_remote_identity = p2p.identity().to_remote_identity();
				let map = map.clone();
				tokio::spawn(async move {
					let peer_id = remote_identity_to_libp2p_peerid(&req.to);
					match control.open_stream_with_addrs(
						peer_id,
						PROTOCOL,
						req.addrs.iter()
							.map(socketaddr_to_quic_multiaddr)
							.collect()
					).await {
						Ok(mut stream) => {
							map.write().unwrap_or_else(PoisonError::into_inner).insert(peer_id, req.to);

							match stream.write_all(&self_remote_identity.get_bytes()).await {
								Ok(_) => {
									debug!("Established outbound stream with '{}'", req.to);
									let _ = req.tx.send(Ok(UnicastStream::new(req.to, stream.compat())));
								},
								Err(e) => {
									let _ = req.tx.send(Err(e.to_string()));
								},
							}
						},
						Err(e) => {
							let _ = req.tx.send(Err(e.to_string()));
						},
					}
				});
			}
		}
	}
}
