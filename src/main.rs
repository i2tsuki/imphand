extern crate notify_rust;
extern crate imap;
extern crate openssl;
extern crate regex;
// #[macro_use]
// extern crate serde;

use imap::client::IMAPStream;
use notify_rust::Notification;
use notify_rust::NotificationHint as Hint;
use openssl::ssl::{SslContext, SslMethod};
use regex::Regex;

use std::process;
use std::str::FromStr;

// mod serde_email;
// use serde_email::de;

const SUMMARY: &'static str = "Category:email";
const ICON: &'static str = "thunderbird-bin-icon";
const APPNAME: &'static str = "imphand";

struct Filter {
    subject: String,
}

struct ReFilter {
    subject: Regex,
}

fn main() {
    let imap = ImapServer {
        server: "example.com".to_string(),
        port: 993,
        username: "example@example.com".to_string(),
        password: "example".to_string(),
    };

    subscribe(imap);
}

fn notification(body: &str) {
    Notification::new()
        .summary(SUMMARY)
        .body(body)
        .icon(ICON)
        .appname(APPNAME)
        .hint(Hint::Category("email".to_owned()))
        .timeout(0)
        .show()
        .unwrap();
}

struct ImapServer {
    server: String,
    port: u16,
    username: String,
    password: String,
}

fn subscribe(imap: ImapServer) {
    let mut imap_socket = IMAPStream::connect((imap.server.as_ref(), imap.port),
                                              Some(SslContext::new(SslMethod::Sslv23).unwrap()))
        .unwrap();
    imap_socket.login(imap.username.as_ref(), imap.password.as_ref()).unwrap();

    // match imap_socket.capability() {
    //     Ok(capabilities) => {
    //         for capability in capabilities.iter() {
    //             println!("{}", capability);
    //         }
    //     }
    //     Err(e) => println!("Error parsing capability: {}", e),
    // };

    // match imap_socket.run_command("LIST \"\" \"*\"") {
    //     Ok(response) => {
    //         for line in response {
    //             print!("{}", line);
    //         }
    //     }
    //     Err(e) => print!("Error run command: {}", e),
    // }

    match imap_socket.run_command("SELECT Inbox") {
        Ok(response) => {
            for line in response {
                print!("{}", line);
            }
        }
        Err(e) => print!("Error run command: {}", e),
    }

    let mut unseen: Vec<u32> = vec![];
    match imap_socket.run_command("FETCH 1:* flags") {
        Ok(response) => {
            for line in response {
                let re = Regex::new(r"\* (\d+) FETCH \(FLAGS \(\)\)").unwrap();
                match re.captures(line.as_ref()) {
                    Some(caps) => {
                        unseen.push(u32::from_str(caps.get(1).unwrap().as_str()).unwrap());
                    }
                    None => (),
                };
            }
        }
        Err(e) => print!("Error run command: {}", e),
    }

    println!("unseen messages: {:?}", unseen);

    let mut filters: Vec<Filter> = vec![];
    let mut refilters: Vec<ReFilter> = vec![];
    filters.push(Filter { subject: r".*に資料が追加されました。".to_string() });
    filters.push(Filter { subject: r".*さんが.*に参加登録しました。".to_string() });
    for filter in filters {
        let re_subject = match Regex::new(filter.subject.as_ref()) {
            Ok(re) => re,
            Err(e) => panic!("failed to compile regex: {}", e),
        };
        refilters.push(ReFilter { subject: re_subject });
    }

    for message_id in unseen {
        notify_matching_email(&mut imap_socket, message_id, &refilters);
    }

    // match imap_socket.run_command("SEARCH all") {
    //     Ok(response) => {
    //         for line in response {
    //             print!("{}", line);
    //         }
    //     }
    //     Err(e) => print!("Error run command: {}", e),
    // }

    // match imap_socket.select("Inbox") {
    //     Ok(mailbox) => {
    //         println!("{:?}", mailbox.unseen);
    //     }
    //     Err(e) => println!("Error selecting INBOX: {}", e),
    // };
}


fn notify_matching_email(imap_socket: &mut IMAPStream, message_id: u32, refilters: &Vec<ReFilter>) {
    let command = format!("FETCH {} rfc822.header", message_id);
    match imap_socket.run_command(command.as_ref()) {
        Ok(response) => {
            let mut u8response: Vec<u8> = vec![];
            for line in response {
                let re = Regex::new(r"\* (\d+) FETCH .*").unwrap();
                if !re.is_match(line.as_ref()) {
                    for u in line.as_bytes() {
                        u8response.push(*u);
                    }
                }
            }
            let s = String::from_utf8(u8response.clone()).unwrap();
            // Debug message
            // println!("u8response: {}", s);

            // Using serialize implementation
            // use std::collections::HashMap;
            // type RFC822 = HashMap<String, String>;
            // let r: RFC822 = match de::from_slice(u8response.as_slice()) {
            //     Ok(rfc822) => rfc822,
            //     Err(e) => {
            //         print!("failed to deserialize: {}", e);
            //         process::exit(1);
            //     }
            // };
            // println!("{:?}", r);

            // FixMe: Using serialize implementation
            // Using regex implementation
            use std::io::{Read, Write};

            let re = match Regex::new(r"Subject: (?P<value>\S+(\r\n\s+\S+)*)") {
                Ok(re) => re,
                Err(e) => panic!("failed to compile regex: {}", e),
            };
            let value = match re.captures(s.as_ref()) {
                Some(caps) => {
                    match caps.name("value") {
                        Some(cap) => cap.as_str(),
                        None => "",
                    }
                }
                None => "",
            };
            let value = value.to_string().replace("\r\n", "");
            let value = value.to_string().replace(" ", "");
            let child = match process::Command::new("nkf")
                .args(&["-w"])
                .stdin(process::Stdio::piped())
                .stdout(process::Stdio::piped())
                .spawn() {
                Ok(child) => child,
                Err(e) => panic!("failed to spawn process: {}", e),
            };
            match child.stdin {
                Some(mut stdin) => {
                    match stdin.write_all(&value.as_bytes()) {
                        Ok(_) => (),
                        Err(e) => panic!("failed to write child.stdin: {}", e),
                    }
                }
                None => panic!("child.stdin is None"),
            }
            let mut subject = String::new();
            match child.stdout {
                Some(mut stdout) => {
                    match stdout.read_to_string(&mut subject) {
                        Ok(_) => (),
                        Err(e) => panic!("failed to read stdout: {}", e),
                    }
                    ()
                }
                None => panic!("child.stdout is None"),
            }
            for refilter in refilters {
                if refilter.subject.is_match(subject.as_ref()) {
                    notification(subject.as_ref());
                    let command = format!("STORE {} +FLAGS (\\Seen)", message_id);
                    match imap_socket.run_command(command.as_ref()) {
                        Ok(_) => (),
                        Err(e) => panic!("failed to run command: {}", e),
                    }
                    break
                }
            }
        }
        Err(e) => panic!("failed to run command: {}", e),
    }
}
