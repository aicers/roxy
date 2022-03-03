use anyhow::{anyhow, Result};
use roxy::{run_roxy, NicOutput, Node, NodeRequest, SubCommand};

#[allow(clippy::too_many_lines)]
fn main() {
    let host = "hostname_A";
    let process = "Roxy";

    println!("get uptime:");
    if let Ok(req) = NodeRequest::new::<Option<String>>(host, process, Node::Uptime, None) {
        match send_request::<String>(&req) {
            Ok(r) => println!("Response: {}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nget version:");
    if let Ok(req) =
        NodeRequest::new::<Option<String>>(host, process, Node::Version(SubCommand::Get), None)
    {
        match send_request::<(String, String)>(&req) {
            Ok(r) => println!("Response: {:?}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nset os version:");
    if let Ok(req) = NodeRequest::new::<String>(
        host,
        process,
        Node::Version(SubCommand::SetOsVersion),
        "AICE OS v1.0.27".to_string(),
    ) {
        match send_request::<String>(&req) {
            Ok(r) => println!("Response: {}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nset product version:");
    if let Ok(req) = NodeRequest::new::<String>(
        host,
        process,
        Node::Version(SubCommand::SetProductVersion),
        "AICE Security v1.1.101".to_string(),
    ) {
        match send_request::<String>(&req) {
            Ok(r) => println!("Response: {}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nget hostname:");
    if let Ok(req) =
        NodeRequest::new::<Option<String>>(host, process, Node::Hostname(SubCommand::Get), None)
    {
        match send_request::<String>(&req) {
            Ok(r) => println!("Response: {}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    // println!("\nset hostname:");
    // if let Ok(req) = NodeRequest::new::<String>(
    //     host,
    //     process,
    //     NodeCommand::Hostname(SubCommand::Set),
    //     "new_hostname".to_string(),
    // ) {
    //     match send_request::<String>(&req) {
    //         Ok(r) => println!("Response: {}", r),
    //         Err(e) => eprintln!("Error: {}", e),
    //     }
    // }

    println!("\nget diskusage:");
    if let Ok(req) = NodeRequest::new::<Option<String>>(host, process, Node::DiskUsage, None) {
        match send_request::<(String, String, String, String)>(&req) {
            Ok(r) => println!("Response: {:?}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nset syslog remote servers:");
    if let Ok(req) = NodeRequest::new::<Vec<String>>(
        host,
        process,
        Node::Syslog(SubCommand::Set),
        vec![
            "@@192.168.0.205:7500".to_string(),
            "@192.168.0.205:500".to_string(),
        ],
    ) {
        match send_request::<String>(&req) {
            Ok(r) => println!("Response: {}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nget syslog remote servers:");
    if let Ok(req) =
        NodeRequest::new::<Option<String>>(host, process, Node::Syslog(SubCommand::Get), None)
    {
        match send_request::<Option<Vec<(String, String, String)>>>(&req) {
            Ok(r) => println!("Response: {:?}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\ninit syslog remote servers:");
    if let Ok(req) =
        NodeRequest::new::<Option<String>>(host, process, Node::Syslog(SubCommand::Init), None)
    {
        match send_request::<String>(&req) {
            Ok(r) => println!("Response: {}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nget interface name list:");
    if let Ok(req) = NodeRequest::new::<Option<String>>(
        host,
        process,
        Node::Interface(SubCommand::List),
        Some("en".to_string()),
    ) {
        match send_request::<Vec<String>>(&req) {
            Ok(r) => println!("Response: {:?}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nget interface setting for eno1:");
    if let Ok(req) = NodeRequest::new::<Option<String>>(
        host,
        process,
        Node::Interface(SubCommand::Get),
        Some("eno1".to_string()),
    ) {
        match send_request::<Option<Vec<(String, NicOutput)>>>(&req) {
            Ok(r) => println!("Response: {:?}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    println!("\nget all interface setting:");
    if let Ok(req) =
        NodeRequest::new::<Option<String>>(host, process, Node::Interface(SubCommand::Get), None)
    {
        match send_request::<Option<Vec<(String, NicOutput)>>>(&req) {
            Ok(r) => println!("Response: {:?}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    // println!("\nset(overwrite) interface setting for eno4:");
    // let new_nic = NicOutput::new(
    //     Some(vec![
    //         "192.168.4.17/24".to_string(),
    //         "192.168.13.17/24".to_string(),
    //     ]),
    //     None,
    //     None,
    //     Some(vec!["164.124.101.2".to_string()]),
    // );
    // if let Ok(req) = NodeRequest::new::<(String, NicOutput)>(
    //     host,
    //     process,
    //     Node::Interface(SubCommand::Set),
    //     ("eno4".to_string(), new_nic),
    // ) {
    //     //req.debug::<(String, NicOutput)>();
    //     match send_request::<String>(&req) {
    //         Ok(r) => println!("Response: {:?}", r),
    //         Err(e) => eprintln!("Error: {}", e),
    //     }
    // }

    // println!("\ndelete interface setting for eno4:");
    // let new_nic = NicOutput::new(Some(vec!["192.168.13.17/24".to_string()]), None, None, None);
    // if let Ok(req) = NodeRequest::new::<(String, NicOutput)>(
    //     host,
    //     process,
    //     Node::Interface(SubCommand::Delete),
    //     ("eno4".to_string(), new_nic),
    // ) {
    //     //req.debug::<(String, NicOutput)>();
    //     match send_request::<String>(&req) {
    //         Ok(r) => println!("Response: {:?}", r),
    //         Err(e) => eprintln!("Error: {}", e),
    //     }
    // }

    // println!("\ndelete interface setting for eno4:");
    // let new_nic = NicOutput::new(Some(vec!["192.168.4.17/24".to_string()]), None, None, None);
    // if let Ok(req) = NodeRequest::new::<(String, NicOutput)>(
    //     host,
    //     process,
    //     Node::Interface(SubCommand::Delete),
    //     ("eno4".to_string(), new_nic),
    // ) {
    //     match send_request::<String>(&req) {
    //         Ok(r) => println!("Response: {:?}", r),
    //         Err(e) => eprintln!("Error: {}", e),
    //     }
    // }

    // println!("\ndelete interface setting for eno4:");
    // let new_nic = NicOutput::new(None, None, None, Some(vec!["164.124.101.2".to_string()]));
    // if let Ok(req) = NodeRequest::new::<(String, NicOutput)>(
    //     host,
    //     process,
    //     Node::Interface(SubCommand::Delete),
    //     ("eno4".to_string(), new_nic),
    // ) {
    //     match send_request::<String>(&req) {
    //         Ok(r) => println!("Response: {:?}", r),
    //         Err(e) => eprintln!("Error: {}", e),
    //     }
    // }

    println!("\nset(overwrite) interface setting for eno4:");
    let new_nic = NicOutput::new(Some(vec!["192.168.4.7/24".to_string()]), None, None, None);
    if let Ok(req) = NodeRequest::new::<(String, NicOutput)>(
        host,
        process,
        Node::Interface(SubCommand::Set),
        ("eno4".to_string(), new_nic),
    ) {
        //req.debug::<(String, NicOutput)>();
        match send_request::<String>(&req) {
            Ok(r) => println!("Response: {:?}", r),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    // println!("\ninit interface setting for eno3:");
    // if let Ok(req) = NodeRequest::new::<String>(
    //     host,
    //     process,
    //     Node::Interface(SubCommand::Init),
    //     "eno3".to_string(),
    // ) {
    //     match send_request::<Option<String>>(&req) {
    //         Ok(r) => println!("Response: {:?}", r),
    //         Err(e) => eprintln!("Error: {}", e),
    //     }
    // }
}

// Here is node. I got a new request!!!
// T: the type of return value
fn send_request<T>(request: &NodeRequest) -> Result<T>
where
    T: serde::de::DeserializeOwned + std::fmt::Debug,
{
    let host = "hostname_A";
    if request.host == *host && request.process == "Roxy" {
        let json = request.roxy_task()?;
        //println!("DEBUG: json = {}", json);
        run_roxy::<T>(&json)
    } else if request.host == *host && request.process == "Hog" {
        // if I am Hog
        unimplemented!();
    } else {
        // forward this request to the destination host
        Err(anyhow!("not me"))
    }
}
