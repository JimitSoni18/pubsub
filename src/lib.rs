pub mod broker;
pub mod cmd_parse;


pub type Message = String;

// #[derive(Clone, Eq, PartialEq, Debug)]
// pub struct Message {
// 	channel_name: String,
// 	message: String,
// }

// impl Message {
// 	pub fn new(channel_name: String, message: String) -> Self {
// 		Self {
// 			channel_name,
// 			message,
// 		}
// 	}
// }

// pub enum SomeMsg {
// 	SubscriptionResponse(String),
// 	PublishedMessage {
// 		channel_name: String,
// 		message: String,
// 	},
// }

