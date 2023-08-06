pub trait DecodeToString {
	fn string(&self) -> String;
}

impl DecodeToString for Vec<u8> {
	fn string(&self) -> String {
		match String::from_utf8(self.clone()) {
			Ok(s) => s,
			Err(_) => format!("{:?}", self),
		}
	}
}