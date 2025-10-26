use std::{net::Ipv4Addr, sync::Arc};

use futures::{SinkExt, StreamExt};
use pubsub::{Message, broker::Broker};
use tokio_util::{
	bytes::Bytes,
	codec::{Framed, LengthDelimitedCodec},
};

mod cmd_parse;

use cmd_parse::parse_cmd;

use crate::cmd_parse::{ChanList, Command};

#[tokio::main]
async fn main() {
	let tcp_listener = tokio::net::TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), 8008))
		.await
		.expect("failed to connect to socket!");

	let broker = Arc::new(Broker::<Message>::default());

	loop {
		let (stream, addr) = tcp_listener.accept().await.unwrap();
		let broker = broker.clone();
		let (tx, client) = broker.new_client();
		let client_id = client.id();
		let mut framed = Framed::new(stream, LengthDelimitedCodec::default());

		// let jh0 = tokio::spawn(async {
		// 	loop {
		// 		while let Some(msg) = client.receiver().recv().await {
		// 			framed.send(Bytes::from(msg.as_ref()));
		// 		}
		// 	}
		// });

		let jh1 = tokio::spawn(async move {
			while let Some(message) = framed.next().await {
				if let Ok(message) = message {
					if let Ok(message) = str::from_utf8(&message) {
						if let Ok(msg) = parse_cmd(message) {
							match msg {
								Command::Pub { channel, message } => {
									// FIXME: send using tx instead of framed.send
									// framed.send(Bytes::from("wtf")).await.unwrap();
								}
								Command::Sub { channels } => {
									for channel in channels {
										// FIXME: channel not created: add CreateChannel variant to Command
										broker.subscribe(channel.to_string(), client_id).unwrap();
									}
								}
								Command::UnSub { channels } => match channels {
									ChanList::All => {
										broker.unsubscribe_all(client_id);
									}
									ChanList::Many(channels) => {
										for channel in channels {
											broker.unsubscribe(channel, client_id).unwrap();
										}
									}
								},
								Command::PSub { channel_patterns } => todo!(),
								Command::PUnSub { channel_patterns } => todo!(),
								Command::PubSub(pub_sub_sub_cmd) => todo!(),
								_ => unreachable!(),
							}
						} else {
							framed
								.send(Bytes::from("unable to parse command from message"))
								.await
								.unwrap();
						}
					} else if framed
						.send(Bytes::from("invalid string; only utf8 allowed"))
						.await
						.is_err()
					{
						framed.close().await.unwrap();
					};
				}
			}
		});
	}

	// let broker: &'static _ = Box::leak(Default::default());

	// loop {
	// 	if let Ok((stream, _ /* addr */)) = tcp_listener.accept().await {
	// 		tokio::spawn(Subscriber::listen(broker, stream));
	// 	}
	// }
}

#[cfg(test)]
mod tests {
	use std::sync::{Arc, LazyLock};

	use super::*;
	static BROKER: LazyLock<Broker<Message>> = LazyLock::new(Broker::default);

	#[tokio::test]
	pub async fn test_static_broker() {
		let (tx, mut client) = BROKER.new_client();
		let client_id = client.id();

		let fx_new_channel_name = "test-chan-1";
		let fx_new_channel_name_clone = "test-chan-1".to_string();

		let fx_message: Arc<str> = Arc::from("abc 123");
		let fx_message_clone = fx_message.clone();

		BROKER.create_channel(fx_new_channel_name).unwrap();

		BROKER
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

		let count = BROKER
			.publish(fx_new_channel_name, fx_message)
			.await
			.unwrap();

		assert_eq!(count, 1);

		jh.await.unwrap();
	}

	#[tokio::test]
	pub async fn test_subscribe_publish_unsubscribe_all() {
		let broker = Broker::default();
		let (tx, mut client) = broker.new_client();
		let client_id = client.id();

		let fx_new_channel_name = "test-chan-1";
		let fx_new_channel_name_clone = "test-chan-1".to_string();

		let fx_message: Arc<str> = Arc::from("abc 123");
		let fx_message_clone = fx_message.clone();

		broker.create_channel(fx_new_channel_name).unwrap();

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
			.publish(fx_new_channel_name, fx_message)
			.await
			.unwrap();

		assert_eq!(count, 1);

		jh.await.unwrap();
	}
}
