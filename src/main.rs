use std::net::Ipv4Addr;

use pubsub::broker::Broker;

#[tokio::main]
async fn main() {
	let tcp_listener = tokio::net::TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), 8008))
		.await
		.expect("failed to connect to socket!");

	let broker = Broker::default();
	let client = broker.new_client();

	// let broker: &'static _ = Box::leak(Default::default());

	// loop {
	// 	if let Ok((stream, _ /* addr */)) = tcp_listener.accept().await {
	// 		tokio::spawn(Subscriber::listen(broker, stream));
	// 	}
	// }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	pub async fn test_subscribe_publish_unsubscribe_all() {
		let broker = Broker::default();
		let mut client = broker.new_client();
		let client_id = client.id();

		let fx_new_channel_name = "test-chan-1".to_string();
		let fx_new_channel_name_clone = "test-chan-1".to_string();

		let fx_message = "abc 123".to_string();
		let fx_message_clone = fx_message.clone();

		broker.new_channel(fx_new_channel_name.clone()).unwrap();

		broker
			.subscribe(fx_new_channel_name_clone, client_id)
			.unwrap();

		let jh = tokio::spawn(async move {
			let Some(msg) = client.receiver().recv().await else {
				panic!("did not receive any message!");
			};

			if fx_message_clone != msg {
				panic!("sent message did not match");
			}
		});

		let count = broker
			.publish(&fx_new_channel_name, fx_message)
			.await
			.unwrap();

		assert_eq!(count, 1);

		jh.await.unwrap();
	}
}
