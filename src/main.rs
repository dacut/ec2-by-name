use {
    async_std_resolver::resolver_from_system_conf,
    aws_config::{self, profile::ProfileFileCredentialsProvider},
    aws_sdk_ec2::{self, model::{Filter as Ec2Filter, InstanceState, InstanceStateChange}},
    aws_types::region::Region,
    futures::stream::{FuturesOrdered, StreamExt},
    getopts::Options,
    log::debug,
    std::{
        collections::HashSet,
        env,
        error::Error,
        future::Future,
        io::{stderr, stdout, Write},
        net::IpAddr,
        pin::Pin,
        process::ExitCode,
    },
    tokio,
};

const INVALID_USAGE: u8 = 2;

enum Operation {
    Print,
    Reboot,
    Start,
    Stop,
    Terminate,
}

impl Operation {
    fn get_flag(&self) -> &'static str {
        match self {
            Operation::Print => "print",
            Operation::Reboot => "reboot",
            Operation::Start => "start",
            Operation::Stop => "stop",
            Operation::Terminate => "terminate",
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optopt(
        "p",
        "profile",
        "Use AWS credentials from the specified profile in ~/.aws/credentials",
        "<profile>",
    );
    opts.optflag("h", "help", "Print this help menu");
    opts.optopt("r", "region", "Use specified AWS region", "<region>");
    opts.optflag("", "start", "Start instances");
    opts.optflag("", "stop", "Stop instances");
    opts.optflag("", "reboot", "Reboot instances");
    opts.optflag("", "terminate", "Terminate instances");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            let mut e = stderr();
            writeln!(e, "{}", f).unwrap();
            print_usage(&program, opts, e);
            return ExitCode::from(INVALID_USAGE);
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, opts, stdout());
        return ExitCode::SUCCESS;
    }

    if matches.free.len() == 0 {
        let mut e = stderr();
        writeln!(e, "No instance names specified").unwrap();
        print_usage(&program, opts, e);
        return ExitCode::from(INVALID_USAGE);
    }

    let mut config = aws_config::from_env();
    if let Some(region) = matches.opt_str("r") {
        config = config.region(Region::new(region));
    }

    if let Some(profile) = matches.opt_str("p") {
        let creds = ProfileFileCredentialsProvider::builder()
            .profile_name(profile)
            .build();
        config = config.credentials_provider(creds);
    }

    let mut op = Operation::Print;
    if matches.opt_present("reboot") {
        op = match op {
            Operation::Print | Operation::Reboot => Operation::Reboot,
            other => {
                eprintln!("--reboot and {} are mutually exclusive", other.get_flag());
                print_usage(&program, opts, stderr());
                return ExitCode::from(INVALID_USAGE);
            }
        }
    }

    if matches.opt_present("start") {
        op = match op {
            Operation::Print | Operation::Start => Operation::Start,
            other => {
                eprintln!("--start and {} are mutually exclusive", other.get_flag());
                print_usage(&program, opts, stderr());
                return ExitCode::from(INVALID_USAGE);
            }
        }
    }

    if matches.opt_present("stop") {
        op = match op {
            Operation::Print | Operation::Stop => Operation::Stop,
            other => {
                eprintln!("--stop and {} are mutually exclusive", other.get_flag());
                print_usage(&program, opts, stderr());
                return ExitCode::from(INVALID_USAGE);
            }
        }
    }

    if matches.opt_present("terminate") {
        op = match op {
            Operation::Print | Operation::Terminate => Operation::Terminate,
            other => {
                eprintln!("--terminate and {} are mutually exclusive", other.get_flag());
                print_usage(&program, opts, stderr());
                return ExitCode::from(INVALID_USAGE);
            }
        }
    }

    let sdk_config = config.load().await;
    let ec2_config = aws_sdk_ec2::config::Builder::from(&sdk_config).build();
    let ec2 = aws_sdk_ec2::Client::from_conf(ec2_config);

    let mut futures = FuturesOrdered::new();

    for name in matches.free {
        debug!("Dispatching find_instances {}", name);
        let future = find_instances(ec2.clone(), name);
        futures.push(future);
    }

    let mut all_instance_ids = HashSet::new();
    while let Some(result) = futures.next().await {
        match result {
            Ok(instance_ids) => for instance_id in instance_ids {
                all_instance_ids.insert(instance_id);
            },
            
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::FAILURE;
            }
        };
    }

    let mut all_instance_ids: Vec<String> = all_instance_ids.into_iter().collect();
    all_instance_ids.sort();

    match op {
        Operation::Print => println!("{}", all_instance_ids.join(" ")),

        Operation::Reboot => {
            println!("Rebooting instances: {}", all_instance_ids.join(" "));
            match ec2.reboot_instances().set_instance_ids(Some(all_instance_ids.clone())).send().await {
                Ok(_) => {
                    println!("Rebooted instances: {}", all_instance_ids.join(" "))
                }
                Err(e) => {
                    eprintln!("Failed to start instances: {}", e);
                    return ExitCode::FAILURE;
                }
            }
        }

        Operation::Start => {
            println!("Starting instances: {}", all_instance_ids.join(" "));
            match ec2.start_instances().set_instance_ids(Some(all_instance_ids)).send().await {
                Ok(start_instances_output) => print_instance_state_changes(
                    start_instances_output.starting_instances
                ),
                Err(e) => {
                    eprintln!("Failed to start instances: {}", e);
                    return ExitCode::FAILURE;
                }
            }
        }

        Operation::Stop => {
            println!("Stopping instances: {}", all_instance_ids.join(" "));
            match ec2.stop_instances().set_instance_ids(Some(all_instance_ids)).send().await {
                Ok(stop_instances_output) => print_instance_state_changes(
                    stop_instances_output.stopping_instances
                ),
                Err(e) => {
                    eprintln!("Failed to start instances: {}", e);
                    return ExitCode::FAILURE;
                }
            }
        }

        Operation::Terminate => {
            println!("Terminating instances: {}", all_instance_ids.join(" "));
            match ec2.terminate_instances().set_instance_ids(Some(all_instance_ids)).send().await {
                Ok(terminate_instances_output) => print_instance_state_changes(
                    terminate_instances_output.terminating_instances
                ),
                Err(e) => {
                    eprintln!("Failed to start instances: {}", e);
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    ExitCode::SUCCESS
}

fn print_instance_state_changes(changes: Option<Vec<InstanceStateChange>>) {
    for change in changes.unwrap_or(vec![]) {
        let instance_id = change.instance_id.unwrap_or("".to_string());
        let previous_state = instance_state_to_string(change.previous_state);
        let current_state = instance_state_to_string(change.current_state);
        println!("{}: {} -> {}", instance_id, previous_state, current_state);
    }
}

fn instance_state_to_string(instance_state: Option<InstanceState>) -> String {
    if let Some(instance_state) = instance_state {
        if let Some(name) = instance_state.name {
            name.as_str().to_string()
        } else {
            "".to_string()
        }
    } else {
        "".to_string()
    }
}

async fn find_instances(ec2: aws_sdk_ec2::Client, name: String) -> Result<HashSet<String>, Box<dyn Error + 'static>> {
    let resolver = resolver_from_system_conf().await?;
    let mut futures = FuturesOrdered::new();
    for ip_addr in resolver.lookup_ip(name.clone()).await? {
        debug!("Found IP address {} for {}", ip_addr, name);
        let future = find_instances_by_ip(ec2.clone(), ip_addr);
        futures.push(future);
    }

    let mut all_instance_ids = HashSet::new();

    while let Some(result) = futures.next().await {
        let result = result?;
        for instance_id in result {
            all_instance_ids.insert(instance_id);
        }
    }

    Ok(all_instance_ids)
}

async fn find_instances_by_ip(ec2: aws_sdk_ec2::Client, address: IpAddr) -> Result<HashSet<String>, Box<dyn Error + 'static>> {
    debug!("Finding instances with IP address {}", address);
    let mut futures = FuturesOrdered::<Pin<Box<dyn Future<Output=Result<HashSet<String>, Box<dyn Error + 'static>>>>>>::new();
    futures.push(Box::pin(find_instances_by_public_ipv4(ec2.clone(), address)));
    futures.push(Box::pin(find_instances_by_public_eip_ipv4(ec2.clone(), address)));
    futures.push(Box::pin(find_instances_by_private_ipv4(ec2.clone(), address)));
    futures.push(Box::pin(find_instances_by_private_netif_ipv4(ec2.clone(), address)));
    futures.push(Box::pin(find_instances_by_netif_ipv6(ec2.clone(), address)));

    let mut all_instance_ids = HashSet::new();

    while let Some(result) = futures.next().await {
        match result {
            Ok(instance_ids) => for instance_id in instance_ids {
                all_instance_ids.insert(instance_id);
            },
            Err(e) => return Err(e),
        }
    }

    Ok(all_instance_ids)
}

async fn find_instances_by_public_ipv4(ec2: aws_sdk_ec2::Client, address: IpAddr) -> Result<HashSet<String>, Box<dyn Error + 'static>> {
    let filter = match address {
        IpAddr::V4(addr) => Ec2Filter::builder().name("ip-address").values(addr.to_string()).build(),
        IpAddr::V6(_) => return Ok(HashSet::new()),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

async fn find_instances_by_public_eip_ipv4(ec2: aws_sdk_ec2::Client, address: IpAddr) -> Result<HashSet<String>, Box<dyn Error + 'static>> {
    let filter = match address {
        IpAddr::V4(addr) => Ec2Filter::builder().name("network-interface.addresses.association.public-ip").values(addr.to_string()).build(),
        IpAddr::V6(_) => return Ok(HashSet::new()),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

async fn find_instances_by_private_ipv4(ec2: aws_sdk_ec2::Client, address: IpAddr) -> Result<HashSet<String>, Box<dyn Error + 'static>> {
    let filter = match address {
        IpAddr::V4(addr) => Ec2Filter::builder().name("private-ip-address").values(addr.to_string()).build(),
        IpAddr::V6(_) => return Ok(HashSet::new()),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

async fn find_instances_by_private_netif_ipv4(ec2: aws_sdk_ec2::Client, address: IpAddr) -> Result<HashSet<String>, Box<dyn Error + 'static>> {
    let filter = match address {
        IpAddr::V4(addr) => Ec2Filter::builder().name("network-interface.addresses.private-ip-address").values(addr.to_string()).build(),
        IpAddr::V6(_) => return Ok(HashSet::new()),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

async fn find_instances_by_netif_ipv6(ec2: aws_sdk_ec2::Client, address: IpAddr) -> Result<HashSet<String>, Box<dyn Error + 'static>> {
    debug!("Finding instances with IPv6 address {}", address);

    let filter = match address {
        IpAddr::V4(_) => return Ok(HashSet::new()),
        IpAddr::V6(addr) => Ec2Filter::builder().name("network-interface.ipv6-addresses.ipv6-address").values(addr.to_string()).build(),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

async fn get_instance_ids_by_filter(ec2: aws_sdk_ec2::Client, filter: Ec2Filter) -> Result<HashSet<String>, Box<dyn Error + 'static>> {
    debug!("Describing instances with filter {:?}", filter);

    let mut results = HashSet::new();
    let mut stream = ec2.describe_instances().filters(filter).into_paginator().send();

    while let Some(describe_instances_result) = stream.next().await {
        debug!("Received instances: {:?}", describe_instances_result);
        let desribe_instances_output = describe_instances_result?;
        for reservation in desribe_instances_output.reservations.unwrap_or(vec![]) {
            debug!("Found reservation: {:?}", reservation.reservation_id);
            for instance in reservation.instances.unwrap_or(vec![]) {
                debug!("Found instance: {:?}", instance.instance_id);
                if let Some(instance_id) = instance.instance_id {
                    results.insert(instance_id);
                }
            }
        }
    }

    debug!("Done describing instances; results={:?}", results);

    Ok(results)
}

fn print_usage<W: Write>(program: &str, opts: Options, mut out: W) {
    let brief = format!("Usage: {} [options] <instance-name>...", program);
    let usage = opts.usage(&brief);
    out.write_all(usage.as_bytes()).unwrap();
}
