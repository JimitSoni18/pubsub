use std::net::Ipv4Addr;

#[tokio::main]
async fn main() {
	let tcp_listener = tokio::net::TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), 8008))
		.await
		.expect("failed to connect to socket!");

	// let broker: &'static _ = Box::leak(Default::default());

	// loop {
	// 	if let Ok((stream, _ /* addr */)) = tcp_listener.accept().await {
	// 		tokio::spawn(Subscriber::listen(broker, stream));
	// 	}
	// }
}
