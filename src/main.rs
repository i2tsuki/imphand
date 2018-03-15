extern crate imap;
extern crate mailparse;
extern crate notify_rust;
extern crate openssl;
extern crate regex;

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

use imap::client::IMAPStream;
use mailparse::*;
use notify_rust::Notification;
use notify_rust::NotificationHint as Hint;
use openssl::ssl::{SslContext, SslMethod};
use regex::Regex;
use std::collections::HashMap;
use std::process;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

// mod serde_email;
// use serde_email::de;

mod app;
mod config;

const SUMMARY: &'static str = "Category:email";
const ICON: &'static str = "thunderbird-bin-icon";
const APPNAME: &'static str = "imphand";

#[derive(Debug)]
struct ReFilter {
    mark: Vec<String>,
    subject: Option<Regex>,
    from: Option<Regex>,
    notification: bool,
    label: Option<String>,
}

fn main() {
    let app = app::new();
    let config = config::new(app.config_file).unwrap();
    for (_name, account) in config.account {
        subscribe(account);
    }
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

fn subscribe(account: config::Account) {
    let mut count: HashMap<String, i64> = HashMap::new();

    let server: Vec<&str> = account.server.split(':').collect();
    let server: (&str, u16) = (server[0], server[1].parse().unwrap());

    let mut socket = match IMAPStream::connect(server, Some(SslContext::new(SslMethod::Sslv23).unwrap())) {
        Ok(socket) => socket,
        Err(e) => panic!("failed to connect to the server: {}", e),
    };

    socket
        .login(&*account.username, &*account.password)
        .unwrap();


    for folder in account.folder {
        let mut refilter: Vec<ReFilter> = vec![];
        match socket.run_command(&format!("SELECT {}", folder.name)) {
            Ok(_) => (),
            Err(e) => print!("Error run command: {}", e),
        }

        for filter in &account.filter[&folder.key] {
            match filter.label.clone() {
                Some(label) => {
                    if !count.contains_key(&label) {
                        count.insert(label, 0);
                    }
                }
                None => (),
            }

            let filter = ReFilter {
                mark: filter.mark.clone(),
                subject: Some(Regex::new(&*filter.clone().subject.unwrap()).unwrap()),
                from: match filter.clone().from {
                    Some(from) => Some(Regex::new(from.as_ref()).unwrap()),
                    None => None,
                },
                notification: filter.notification,
                label: filter.label.clone(),
            };
            refilter.push(filter);
        }

        let re = Regex::new(
            r"\* (\d+) FETCH \(FLAGS \((\s?|NonJunk|\\Seen|NonJunk \\Seen)\)\)",
        ).unwrap();
        match socket.run_command(&format!("FETCH {} {}", "1:*", "FLAGS")) {
            Ok(resp) => {
                info!("{:?}", resp);
                for line in resp {
                    match re.captures(&line) {
                        Some(caps) => {
                            let id = u16::from_str(caps.get(1).unwrap().as_str()).unwrap();
                            count = notify_matching_email(&mut socket, id, &refilter, count);
                        }
                        None => {
                            info!("unmatch: {}", line.replace("\n", ""));
                        }
                    };
                }
            }
            Err(e) => print!("Error run command: {}", e),
        }
    }
    let epoch = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(now) => now.as_secs(),
        Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    };
    for (key, value) in &count {
        println!("{}\t{}\t{}", key, value, epoch);
    }
}

fn mark_email(socket: &mut IMAPStream, id: u16, mark: &Vec<String>) -> bool {
    if mark.contains(&"seen".to_string()) {
        match socket.run_command(&*format!("STORE {} {}", id, "+FLAGS (\\Seen)")) {
            Ok(_) => (),
            Err(e) => panic!("failed to run command: {}", e),
        }
    }
    if mark.contains(&"delete".to_string()) {
        match socket.run_command(&*format!("STORE {} {}", id, "+FLAGS (\\Deleted)")) {
            Ok(_) => (),
            Err(e) => panic!("failed to run command: {}", e),
        }
        return true;
    }
    false
}

fn notify_matching_email<'a>(
    socket: &mut IMAPStream,
    id: u16,
    filter: &Vec<ReFilter>,
    mut count: HashMap<String, i64>,
) -> (HashMap<String, i64>) {
    match socket.run_command(&format!("FETCH {} {}", id, "rfc822.header")) {
        Ok(resp) => {
            let body: String = resp.into_iter().collect();
            let mut subject = "".to_string();
            let mut from = "".to_string();
            let index = body.find('\n').unwrap();
            let (_, body) = body.split_at(index + 1);
            let index = body.rfind(')').unwrap();
            let (body, _) = body.split_at(index + 1);
            let parsed = parse_mail(body.as_bytes()).unwrap();
            for header in parsed.headers {
                let key = header.get_key().unwrap();
                match &*key {
                    "From" => from = header.get_value().unwrap(),
                    "Subject" => subject = header.get_value().unwrap(),
                    _ => {}

                }
            }
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

            let mut flag = false;
            for f in filter {
                for obj in vec![(subject.as_ref(), &f.subject), (from.as_ref(), &f.from)] {
                    match *obj.1 {
                        Some(ref m) => {
                            if m.is_match(obj.0) {
                                flag = true
                            } else {
                                flag = false;
                                break;
                            }
                        }
                        None => (),
                    }
                }
                if flag {
                    info!("subject: {}, from: {}", subject, from);
                    match f.label {
                        Some(ref label) => {
                            if let Some(x) = count.get_mut(label) {
                                *x = *x + 1;
                            }
                        }
                        None => (),
                    }
                    if f.notification {
                        notification(subject.as_ref());
                    }
                    if mark_email(socket, id, &f.mark) {
                        break;
                    }
                }
            }

            // for (_, filter) in filter.iter().enumerate() {
            //     match filter.subject {
            //         Some(f_subject) => match filter.from {
            //             Some(f_from) => {
            //                 if f_subject.is_match(&subject) && f_from.is_match(&from) {
            //                     println!("subject: {}, from: {}", subject, from);
            //                     if filter.notification {
            //                         notification(subject.as_ref());
            //                     }
            //                     if mark_email(socket, id, &filter.mark) {
            //                         break;
            //                     }
            //                 }
            //             },
            //             None => {
            //             }
            //         }
            //     }

            //     }

            //     match filter.from {
            //         Some(from) => {
            //             if
            //         }
            //     }

            // }
        }
        Err(e) => panic!("failed to run command: {}", e),
    }
    return count;
}
