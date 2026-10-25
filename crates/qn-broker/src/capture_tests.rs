// AGENTS.md permits unwrap in tests only.
#![allow(clippy::unwrap_used)]
use super::*;
fn bytes(value: &[u8]) -> CaptureBytes {
    CaptureBytes::new(value.to_vec()).unwrap()
}
fn binding(listener: &CaptureListener) -> CaptureBinding {
    serde_json::from_slice(&serde_json::to_vec(listener.binding()).unwrap()).unwrap()
}
async fn connect(listener: &CaptureListener) -> (TcpStream, snow::TransportState) {
    let mut peer = TcpStream::connect(listener.binding.address).await.unwrap();
    let mut noise = builder(&listener.binding.capability)
        .unwrap()
        .build_initiator()
        .unwrap();
    send_handshake(&mut peer, &mut noise).await.unwrap();
    receive_handshake(&mut peer, &mut noise).await.unwrap();
    (peer, noise.into_transport_mode().unwrap())
}
#[tokio::test]
async fn it_exchanges_and_redacts_sensitive_values() {
    let listener = CaptureListener::bind(Duration::from_secs(2)).await.unwrap();
    let client = binding(&listener);
    let (served, received) = tokio::join!(
        async {
            let request = listener.accept().await.unwrap();
            assert_eq!(request.payload(), b"metadata");
            request.reply(bytes(b"synthetic-secret")).await
        },
        client.exchange(bytes(b"metadata"), Duration::from_secs(2))
    );
    served.unwrap();
    let response = received.unwrap();
    assert_eq!(response.as_bytes(), b"synthetic-secret");
    assert!(!format!("{response:?} {client:?}").contains("synthetic-secret"));
}
#[tokio::test]
async fn it_rejects_wrong_identities() {
    let listener = CaptureListener::bind(Duration::from_secs(2)).await.unwrap();
    let other = CaptureListener::bind(Duration::from_secs(2)).await.unwrap();
    let mut client = binding(&other);
    client.address = listener.binding.address;
    let (server, relay) = tokio::join!(
        listener.accept(),
        client.exchange(bytes(b"x"), Duration::from_secs(2))
    );
    assert!(server.is_err(), "reject foreign Noise identity");
    assert!(relay.is_err(), "reject foreign Noise identity");
}
#[tokio::test]
async fn it_rejects_invalid_frames_and_nonlocal_bindings() {
    for length in [0, 16_401, u32::MAX, 10, 16] {
        let listener = CaptureListener::bind(Duration::from_secs(2)).await.unwrap();
        let (result, ()) = tokio::join!(listener.accept(), async {
            let (mut peer, _channel) = connect(&listener).await;
            peer.write_u32(length).await.unwrap();
            if length == 16 {
                peer.write_all(&[0; 16]).await.unwrap();
            }
            peer.shutdown().await.unwrap();
        });
        assert!(result.is_err(), "invalid frame {length}");
    }
    assert!(CaptureBytes::new(vec![]).is_err());
    assert!(CaptureBytes::new(vec![0; MAX_FRAME + 1]).is_err());
    let listener = CaptureListener::bind(Duration::from_secs(2)).await.unwrap();
    let address = listener.binding.address;
    let mut client = binding(&listener);
    client.address.set_ip("192.0.2.1".parse().unwrap());
    assert!(matches!(
        client.exchange(bytes(b"x"), Duration::from_secs(1)).await,
        Err(CaptureError::Rejected)
    ));
    drop(listener);
    assert!(TcpStream::connect(address).await.is_err());
}
#[tokio::test(start_paused = true)]
async fn it_bounds_stalled_input_and_expired_delivery() {
    let lifetime = Duration::from_secs(2);
    assert!(CaptureListener::bind(Duration::ZERO).await.is_err());
    let listener = CaptureListener::bind(lifetime).await.unwrap();
    let _idle = TcpStream::connect(listener.binding.address).await.unwrap();
    let (accepted, ()) = tokio::join!(listener.accept(), async {
        tokio::task::yield_now().await;
        tokio::time::advance(lifetime + Duration::from_secs(1)).await;
    });
    assert!(matches!(accepted, Err(CaptureError::Timeout)));
    let (request, _peer) = tokio::join!(listener.accept(), async {
        let (mut peer, mut channel) = connect(&listener).await;
        send(&mut peer, &mut channel, bytes(b"metadata"))
            .await
            .unwrap();
        peer
    });
    tokio::time::advance(lifetime + Duration::from_secs(1)).await;
    assert!(matches!(
        request.unwrap().reply(bytes(b"secret")).await,
        Err(CaptureError::Timeout)
    ));
}
