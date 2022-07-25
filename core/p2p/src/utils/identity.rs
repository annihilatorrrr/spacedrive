use std::net::Ipv4Addr;

use rcgen::{CertificateParams, DistinguishedName, DnType, RcgenError, SanType};

/// The common name of the identity certificate generated by sd-p2p.
const CERTIFICATE_COMMON_NAME: &'static str = "sd-p2p-identity";

/// Is the identity which respresents the current peer. An Identity is made from a public key and a private key combo. [crate::PeerId]'s are derived from the public key portion of a peer's [Identity].
/// The public key is safe to share while the private key must remain private to ensure the connections between peers are secure.
#[derive(Clone)]
pub struct Identity {
	cert: Vec<u8>,
	key: Vec<u8>,
}

impl Identity {
	/// Create a new Identity for the current peer.
	pub fn new() -> Result<Self, RcgenError> {
		let mut params: CertificateParams = Default::default();
		params.distinguished_name = DistinguishedName::new();
		params
			.distinguished_name
			.push(DnType::CommonName, CERTIFICATE_COMMON_NAME);
		params.subject_alt_names = vec![SanType::IpAddress(Ipv4Addr::LOCALHOST.into())];
		let cert = rcgen::Certificate::from_params(params)?;

		Ok(Self {
			cert: cert.serialize_der()?,
			key: cert.serialize_private_key_der(),
		})
	}

	/// Load the current identity from it's raw form.
	pub fn from_raw(cert: Vec<u8>, key: Vec<u8>) -> Result<Self, RcgenError> {
		Ok(Self { cert, key })
	}

	/// Convert this identity into it's raw form so it can be saved.
	pub fn to_raw(&self) -> (Vec<u8>, Vec<u8>) {
		(self.cert.clone(), self.key.clone())
	}

	/// Convert this identity into rustls compatible form so it can be used for the QUIC TLS handshake.
	pub fn into_rustls(self) -> (rustls::Certificate, rustls::PrivateKey) {
		(rustls::Certificate(self.cert), rustls::PrivateKey(self.key))
	}
}