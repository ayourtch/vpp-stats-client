use clap::Parser as ClapParser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, SystemTime};
use vpp_stat_client::*;

#[derive(Clone, Debug, Serialize, Deserialize, ClapParser)]
enum Operation {
    OpLs,
    OpDump,
    OpPoll,
    OpTightPoll,
}

impl FromStr for Operation {
    type Err = &'static str;

    fn from_str(input: &str) -> Result<Operation, Self::Err> {
        match input {
            "ls" => Ok(Operation::OpLs),
            "dump" => Ok(Operation::OpDump),
            "poll" => Ok(Operation::OpPoll),
            "tightpoll" => Ok(Operation::OpTightPoll),
            _ => Err("Could not parse operation"),
        }
    }
}

/// This program does something useful, but its author needs to edit this.
/// Else it will be just hanging around forever
#[derive(Debug, Clone, ClapParser, Serialize, Deserialize)]
#[clap(version = "0.0", author = "Andrew Yourtchenko <ayourtch@gmail.com>")]
struct Opts {
    /// Target hostname to do things on
    #[clap(short, long, default_value = "/tmp/stats.sock")]
    socket: String,

    /// Operation to run: ls, dump, poll, tightpoll
    #[clap(short, long)]
    operation: Operation,

    /// Pattern to match
    #[clap(short, long, default_value = ".*")]
    pattern: Vec<String>,

    /// A level of verbosity, and can be used multiple times
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,
}

fn print_stat_data(data: &VppStatData<'_>) {
    for item in data.iter() {
        use vpp_stat_client::StatValue::*;
        match item.value {
            ScalarIndex(val) => {
                println!("{}: {}", item.name, val);
            }
            CounterVectorSimple(cvs) => {
                for i in 0..cvs.len() {
                    println!("{}[{}]: {:?}", item.name, i, &cvs[i]);
                }
            }
            CounterVectorCombined(cvc) => {
                for i in 0..cvc.len() {
                    print!("{}[{}]: ", item.name, i);
                    for v in &cvc[i] {
                        print!("({} pkt, {} bytes)", v.packets, v.bytes);
                    }
                    println!("");
                }
            }
            NameVector(nv) => {
                for i in 0..nv.len() {
                    println!("{}[{}]: {:?}", item.name, i, &nv[i]);
                }
            }
            Empty => {}

            x => {
                println!("ERR: {:?}", &x);
            }
            _ => unimplemented!(),
        }
    }
}

fn main() {
    let opts: Opts = Opts::parse();

    let c = VppStatClient::connect(&opts.socket).unwrap();

    let mut patterns = VppStringVec::new();
    for s in &opts.pattern {
        patterns.push(&s);
    }
    /*
    patterns.push("main");
    patterns.push(".*");
    patterns.push("/err/ikev2-ip4/ike_auth_req");
    patterns.push("/if/names");
    patterns.push("/bfd/udp4/sessions");
    */
    println!("Patterns: {:?}", &patterns);
    let mut dir = c.ls(Some(&patterns));

    match opts.operation {
        Operation::OpLs => {
            for name in dir.names() {
                println!("{}", name);
            }
        }
        Operation::OpDump => {
            let data = dir.dump().unwrap();
            print_stat_data(&data);
        }
        Operation::OpPoll => loop {
            let data = if let Ok(d) = dir.dump() {
                d
            } else {
                dir = c.ls(Some(&patterns));
                continue;
            };
            print_stat_data(&data);
            std::thread::sleep(std::time::Duration::from_secs(5));
        },
        Operation::OpTightPoll => loop {
            let data = if let Ok(d) = dir.dump() {
                d
            } else {
                dir = c.ls(Some(&patterns));
                continue;
            };
        },
        _ => unimplemented!(),
    }
}
