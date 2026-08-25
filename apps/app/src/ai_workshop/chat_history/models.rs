use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Conversation {
	pub id: String,
	pub title: String,
	pub instance_id: Option<String>,
	pub created_at: i64,
	pub updated_at: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Message {
	pub id: String,
	pub conversation_id: String,
	pub role: String,
	pub content: String,
	pub tool_calls: Option<String>,
	pub tool_call_id: Option<String>,
	pub created_at: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct NewMessage {
	pub conversation_id: String,
	pub role: String,
	pub content: String,
	pub tool_calls: Option<String>,
	pub tool_call_id: Option<String>,
}
