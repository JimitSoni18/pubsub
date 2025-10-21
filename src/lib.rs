pub mod broker;
pub mod cmd_parse;

#[derive(Clone)]
pub struct Message {
	channel_name: String,
	message: String,
}

pub enum SomeMsg {
    SubscriptionResponse(String),
    PublishedMessage {
        channel_name: String,
        message: String,
    }
}

