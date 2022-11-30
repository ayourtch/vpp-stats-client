use clap::Parser as ClapParser;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::str::FromStr;
use tiny_http::{Response, Server, StatusCode};
use vpp_stat_client::*;

/// This program does something useful, but its author needs to edit this.
/// Else it will be just hanging around forever
#[derive(Debug, Clone, ClapParser, Serialize, Deserialize)]
#[clap(version = "0.0", author = "Andrew Yourtchenko <ayourtch@gmail.com>")]
struct Opts {
    /// VPP stats socket as supplied in the "statseg { socket-name /path/to/socket }" config
    #[clap(short, long, default_value = "/tmp/stats.sock")]
    socket: String,

    /// Pattern to match
    #[clap(short, long, default_value = ".*")]
    pattern: Vec<String>,

    /// A level of verbosity, and can be used multiple times
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,
}

use lazy_static::lazy_static;

use regex::Regex;
use std::borrow::Cow;

fn prom_str(s: &str) -> Cow<str> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"[^0-9a-zA-Z]").unwrap();
    }
    RE.replace_all(s, "_")
}

fn write_stat_data(data: &VppStatData<'_>) -> String {
    let mut out = "".to_string();
    for item in data.iter() {
        use vpp_stat_client::StatValue::*;
        let name = prom_str(item.name);
        match item.value {
            ScalarIndex(val) => {
                writeln!(out, "# TYPE {} counter", name);
                writeln!(out, "{} {:.2}", name, val);
            }
            CounterVectorSimple(cvs) => {
                writeln!(out, "# TYPE {} counter", name);
                for k in 0..cvs.len() {
                    let cvs_k = &cvs[k];
                    for j in 0..cvs_k.len() {
                        writeln!(
                            out,
                            "{}{{thread=\"{}\",interface=\"{}\"}} {}",
                            name, k, j, cvs_k[j]
                        );
                    }
                }
            }
            CounterVectorCombined(cvc) => {
                writeln!(out, "# TYPE {}_packets counter", name);
                writeln!(out, "# TYPE {}_bytes counter", name);
                for k in 0..cvc.len() {
                    let cvc_k = &cvc[k];
                    for j in 0..cvc_k.len() {
                        writeln!(
                            out,
                            "{}_packets{{thread=\"{}\",interface=\"{}\"}} {}",
                            name, k, j, cvc_k[j].packets
                        );
                        writeln!(
                            out,
                            "{}_bytes{{thread=\"{}\",interface=\"{}\"}} {}",
                            name, k, j, cvc_k[j].bytes
                        );
                    }
                }
            }
            NameVector(nv) => {
                writeln!(out, "# TYPE {}_info gauge", name);
                for k in 0..nv.len() {
                    writeln!(
                        out,
                        "{}_info{{index=\"{}\",name=\"{}\"}} 1",
                        name, k, &nv[k]
                    );
                }
            }
            Empty => {}
            _ => unimplemented!(),
        }
    }
    out
}

static root_page_str: &str = "<html><head><title>Metrics exporter</title></head><body><ul><li><a href=\"/metrics\">metrics</a></li></ul></body></html>\n";
static not_found_str: &str = "<html><head><title>Document not found</title></head><body><h1>404 - Document not found</h1></body></html>\n";

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

    use ascii::AsciiString;
    use std::fs::File;
    use std::str::FromStr;

    let endpoint = "0.0.0.0:8000";

    let server = Server::http(endpoint).unwrap();
    println!("Listening on {}", endpoint);

    let content_type_html = tiny_http::Header {
        field: "Content-Type".parse().unwrap(),
        value: AsciiString::from_ascii("text/html; charset=utf8").unwrap(),
    };

    for request in server.incoming_requests() {
        let url = request.url();
        let method = request.method();
        let _headers = request.headers();
        match url {
            "/" => {
                let file = std::fs::File::open("page.txt");
                let response = Response::from_string(root_page_str);
                let response = response.with_header(content_type_html.clone());
                // Response::from_file("test.txt"),
                request.respond(response);
            }
            "/metrics" => {
                let mut out: Option<String> = None;

                'refresh_layout: for i in 1..10 {
                    let data = if let Ok(d) = dir.dump() {
                        d
                    } else {
                        dir = c.ls(Some(&patterns));
                        continue 'refresh_layout;
                    };
                    out = Some(write_stat_data(&data));
                    break 'refresh_layout;
                }
                let response = if let Some(out) = out {
                    Response::from_string(out)
                } else {
                    panic!("Could not acquire soft lock!");
                };
                request.respond(response);
            }
            _ => {
                let response = Response::from_string(not_found_str);
                let response = response.with_header(content_type_html.clone());
                let response = response.with_status_code(StatusCode(404));
                request.respond(response);
            }
        }
    }
}
