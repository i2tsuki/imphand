pub struct ImapServer {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

pub fn imap_server() -> ImapServer {
    let imap = ImapServer {
        server: "example.com".to_string(),
        port: 993,
        username: "example@example.com".to_string(),
        password: "example".to_string(),
    };
    imap
}
