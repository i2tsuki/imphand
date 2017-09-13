extern crate notify_rust;
extern crate imap;
extern crate openssl;
extern crate regex;
extern crate mailparse;

// #[macro_use]
// extern crate serde;

use imap::client::IMAPStream;
use mailparse::*;
use notify_rust::Notification;
use notify_rust::NotificationHint as Hint;
use openssl::ssl::{SslContext, SslMethod};
use regex::Regex;

use std::io::{self, Read};
use std::process;
use std::str::FromStr;

// mod serde_email;
// use serde_email::de;

const SUMMARY: &'static str = "Category:email";
const ICON: &'static str = "thunderbird-bin-icon";
const APPNAME: &'static str = "imphand";

struct Filter {
    subject: String,
    from: String,
}

struct ReFilter {
    subject: Regex,
    from: Regex,
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
    let mut imap_socket = match IMAPStream::connect(
        (imap.server.as_ref(), imap.port),
        Some(SslContext::new(SslMethod::Sslv23).unwrap()),
    ) {
        Ok(socket) => socket,
        Err(e) => panic!("failed to connect to the server: {}", e),
    };

    imap_socket
        .login(imap.username.as_ref(), imap.password.as_ref())
        .unwrap();

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
    filters.push(Filter {
        subject: r".*に資料が追加されました。".to_string(),
        from: r"no-reply@connpass.com".to_string(),
    });
    filters.push(Filter {
        subject: r".*さんが.*に参加登録しました。".to_string(),
        from: r"no-reply@connpass.com".to_string(),
    });
    filters.push(Filter {
        subject: r".*が.*を公開しました".to_string(),
        from: r"no-reply@connpass.com".to_string(),
    });
    for filter in filters {
        let re_subject = match Regex::new(filter.subject.as_ref()) {
            Ok(re) => re,
            Err(e) => panic!("failed to compile regex: {}", e),
        };
        let re_from = match Regex::new(filter.from.as_ref()) {
            Ok(re) => re,
            Err(e) => panic!("failed to compile regex: {}", e),
        };
        refilters.push(ReFilter {
            subject: re_subject,
            from: re_from,
        });
    }

    let mut delete_targets: Vec<u32> = vec![];
    for message_id in unseen {
        notify_matching_email(
            &mut imap_socket,
            message_id,
            &refilters,
            &mut delete_targets,
        );
    }

    // Deleting messages
    loop {
        let mut input: [u8; 1] = [0];
        println!("confirm delete message(Y/n): ");
        match io::stdin().read(&mut input) {
            Ok(_) => (),
            Err(e) => panic!("failed to read input: {}", e),
        }
        println!("input: {:?}", input);
        if input[0] == 89 || input[0] == 121 {
            println!("delete message: {:?}", delete_targets);
            for message_id in delete_targets {
                let command = format!("STORE {} +FLAGS (\\Deleted)", message_id);
                match imap_socket.run_command(command.as_ref()) {
                    Ok(_) => (),
                    Err(e) => panic!("failed to run command: {}", e),
                }
            }
            break;
        } else if input[0] == 110 {
            break;
        }
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


fn notify_matching_email(
    imap_socket: &mut IMAPStream,
    message_id: u32,
    refilters: &Vec<ReFilter>,
    delete_targets: &mut Vec<u32>,
) {
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
            let mut subject = "".to_string();
            let mut from = "".to_string();
            let parsed = parse_mail(&u8response[..]).unwrap();
            for header in parsed.headers {
                let key = header.get_key().unwrap();
                match &*key {
                    "From" => {
                        from = header.get_value().unwrap().replace(" ", "");
                    }
                    "Subject" => {
                        subject = header.get_value().unwrap().replace(" ", "");
                    }
                    _ => {}

                }
                if header.get_key().unwrap() == "Subject" {}
            }
            println!("{}, {}", subject, from);

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

            for (_, refilter) in refilters.iter().enumerate() {
                if refilter.subject.is_match(subject.as_ref()) && refilter.from.is_match(from.as_str()) {
                    notification(subject.as_ref());
                    let command = format!("STORE {} +FLAGS (\\Seen)", message_id);
                    match imap_socket.run_command(command.as_ref()) {
                        Ok(_) => (),
                        Err(e) => panic!("failed to run command: {}", e),
                    }
                    delete_targets.push(message_id);
                    break;
                }
            }
        }
        Err(e) => panic!("failed to run command: {}", e),
    }
}
