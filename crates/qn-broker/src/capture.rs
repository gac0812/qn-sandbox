//! Host-only Noise capture transport; consumers authorize each live grant (#96).
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{Instant, timeout, timeout_at},
};
use zeroize::Zeroizing;
const MAX_FRAME: usize = 16 * 1024;
const MAX_WIRE: usize = MAX_FRAME + 16;
const PROTOCOL: &str = "Noise_NNpsk0_25519_ChaChaPoly_SHA256";

/// Typed failures contain no payloads, private keys or peer-supplied text.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    /// Operating-system transport failure.
    #[error("host capture transport failed; obtain a new private binding")]
    Io(#[from] std::io::Error),
    /// Invalid frame, target or timeout.
    #[error("host capture rejected; use a current private binding and bounded request")]
    Rejected,
    /// Accepted exchange exceeded its deadline.
    #[error("host capture timed out; retry only while the session grant remains live")]
    Timeout,
    /// Secure randomness is unavailable.
    #[error("host capture randomness unavailable; restore the host random source")]
    Random,
    /// Noise authentication/encryption failure, without wire data in the source.
    #[error("host capture authentication failed; obtain a new private binding")]
    Crypto(#[from] snow::Error),
}
/// Secret bytes with redacted Debug and owned storage clearing on drop.
pub struct CaptureBytes(Zeroizing<Vec<u8>>);
impl CaptureBytes {
    /// Own a bounded payload; rejected buffers are also cleared on drop.
    /// # Errors
    /// Rejects payloads outside 1–16 KiB.
    pub fn new(bytes: Vec<u8>) -> Result<Self, CaptureError> {
        let bytes = Zeroizing::new(bytes);
        if bytes.is_empty() || bytes.len() > MAX_FRAME {
            return Err(CaptureError::Rejected);
        }
        Ok(Self(bytes))
    }
    /// Borrow only for host parsing or credential delivery; never log these bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for CaptureBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureBytes([REDACTED])")
    }
}
/// Per-listener host authentication; never an Agent config or grant handle.
/// Provision only in private host storage/stdin; never sandbox fs/env/argv or logs.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureBinding {
    address: SocketAddr,
    capability: Zeroizing<[u8; 32]>,
}
impl fmt::Debug for CaptureBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureBinding([REDACTED])")
    }
}
impl CaptureBinding {
    /// Exchange one mutually authenticated, encrypted request; no retries/cache.
    /// # Errors
    /// Rejects non-loopback, invalid binding, frame, IO or deadline failure.
    pub async fn exchange(
        &self,
        request: CaptureBytes,
        lifetime: Duration,
    ) -> Result<CaptureBytes, CaptureError> {
        if self.address.ip() != Ipv4Addr::LOCALHOST {
            return Err(CaptureError::Rejected);
        }
        deadline(lifetime)?;
        timeout(lifetime, async {
            let mut stream = TcpStream::connect(self.address).await?;
            let mut noise = builder(&self.capability)?.build_initiator()?;
            send_handshake(&mut stream, &mut noise).await?;
            receive_handshake(&mut stream, &mut noise).await?;
            let mut channel = noise.into_transport_mode()?;
            send(&mut stream, &mut channel, request).await?;
            receive(&mut stream, &mut channel).await
        })
        .await
        .map_err(|_| CaptureError::Timeout)?
    }
}
/// Owned listener without background tasks; callers impose bounded concurrency.
pub struct CaptureListener {
    listener: TcpListener,
    binding: CaptureBinding,
    lifetime: Duration,
}
impl CaptureListener {
    /// Bind IPv4 loopback with an independent OS-random 256-bit shared capability.
    /// # Errors
    /// Fails on invalid timeout, randomness failure or IO.
    pub async fn bind(lifetime: Duration) -> Result<Self, CaptureError> {
        deadline(lifetime)?;
        let mut capability = Zeroizing::new([0; 32]);
        getrandom::fill(capability.as_mut()).map_err(|_| CaptureError::Random)?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let binding = CaptureBinding {
            address: listener.local_addr()?,
            capability,
        };
        Ok(Self {
            listener,
            binding,
            lifetime,
        })
    }
    /// Private host provisioning only; never disclose to a sandbox or logger.
    #[must_use]
    pub fn binding(&self) -> &CaptureBinding {
        &self.binding
    }
    /// Authenticate before exposing metadata. Idle accept is cancel-safe.
    /// # Errors
    /// Fails on peer authentication, IO, invalid framing or accepted-peer deadline.
    pub async fn accept(&self) -> Result<CaptureRequest, CaptureError> {
        let (mut stream, _) = self.listener.accept().await?;
        let deadline = deadline(self.lifetime)?;
        timeout_at(deadline, async {
            let mut noise = builder(&self.binding.capability)?.build_responder()?;
            receive_handshake(&mut stream, &mut noise).await?;
            send_handshake(&mut stream, &mut noise).await?;
            let mut channel = noise.into_transport_mode()?;
            let payload = receive(&mut stream, &mut channel).await?;
            Ok(CaptureRequest {
                stream,
                channel,
                payload,
                deadline,
            })
        })
        .await
        .map_err(|_| CaptureError::Timeout)?
    }
}
/// Authenticated transport request, not grant authorization; Drop closes it.
pub struct CaptureRequest {
    stream: TcpStream,
    channel: snow::TransportState,
    payload: CaptureBytes,
    deadline: Instant,
}
impl CaptureRequest {
    /// Untrusted metadata: validate the bound session, exact scope and live grant.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        self.payload.as_bytes()
    }
    /// Reply after final live-grant admission; consumes this request.
    /// Admitted responses are in flight, not retroactively revocable.
    /// # Errors
    /// Fails on IO, encryption or expiry of the accepted exchange deadline.
    pub async fn reply(mut self, response: CaptureBytes) -> Result<(), CaptureError> {
        if Instant::now() >= self.deadline {
            return Err(CaptureError::Timeout);
        }
        timeout_at(
            self.deadline,
            send(&mut self.stream, &mut self.channel, response),
        )
        .await
        .map_err(|_| CaptureError::Timeout)?
    }
}
fn deadline(lifetime: Duration) -> Result<Instant, CaptureError> {
    if lifetime.is_zero() {
        return Err(CaptureError::Rejected);
    }
    Instant::now()
        .checked_add(lifetime)
        .ok_or(CaptureError::Rejected)
}
fn builder(secret: &[u8; 32]) -> Result<snow::Builder<'_>, CaptureError> {
    Ok(snow::Builder::new(PROTOCOL.parse()?).psk(0, secret)?)
}
async fn read_wire(stream: &mut TcpStream) -> Result<Zeroizing<Vec<u8>>, CaptureError> {
    let length = stream.read_u32().await? as usize;
    if length == 0 || length > MAX_WIRE {
        return Err(CaptureError::Rejected);
    }
    let mut bytes = Zeroizing::new(vec![0; length]);
    stream.read_exact(&mut bytes).await?;
    Ok(bytes)
}
async fn write_wire(stream: &mut TcpStream, bytes: &[u8]) -> Result<(), CaptureError> {
    stream
        .write_u32(u32::try_from(bytes.len()).map_err(|_| CaptureError::Rejected)?)
        .await?;
    stream.write_all(bytes).await?;
    Ok(())
}
async fn send_handshake(
    stream: &mut TcpStream,
    noise: &mut snow::HandshakeState,
) -> Result<(), CaptureError> {
    let mut wire = [0; 128];
    let length = noise.write_message(&[], &mut wire)?;
    write_wire(stream, &wire[..length]).await
}
async fn receive_handshake(
    stream: &mut TcpStream,
    noise: &mut snow::HandshakeState,
) -> Result<(), CaptureError> {
    let wire = read_wire(stream).await?;
    if noise.read_message(&wire, &mut [])? != 0 {
        return Err(CaptureError::Rejected);
    }
    Ok(())
}
async fn send(
    stream: &mut TcpStream,
    channel: &mut snow::TransportState,
    bytes: CaptureBytes,
) -> Result<(), CaptureError> {
    let mut wire = Zeroizing::new(vec![0; MAX_WIRE]);
    let length = channel.write_message(bytes.as_bytes(), &mut wire)?;
    write_wire(stream, &wire[..length]).await
}
async fn receive(
    stream: &mut TcpStream,
    channel: &mut snow::TransportState,
) -> Result<CaptureBytes, CaptureError> {
    let wire = read_wire(stream).await?;
    let mut plain = Zeroizing::new(vec![0; MAX_WIRE]);
    let length = channel.read_message(&wire, &mut plain)?;
    plain.truncate(length);
    if plain.is_empty() || plain.len() > MAX_FRAME {
        return Err(CaptureError::Rejected);
    }
    Ok(CaptureBytes(plain))
}
#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
