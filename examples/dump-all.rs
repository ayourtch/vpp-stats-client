use vpp_stat_client::*;

fn main() {
    let c = VppStatClient::connect("/tmp/stats.sock").unwrap();

    let mut patterns = VppStringVec::new();
    patterns.push("main");
    patterns.push(".*");
    patterns.push("/err/ikev2-ip4/ike_auth_req");
    patterns.push("/if/names");
    patterns.push("/bfd/udp4/sessions");
    println!("Patterns: {:?}", &patterns);
    let dir = c.ls(Some(&patterns));
    for name in dir.names() {
        // println!("{}", name);
    }

    println!("running dump");
    let data = dir.dump();

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
            _ => unimplemented!(),
        }
    }
}
