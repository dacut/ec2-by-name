mod ec2;
mod error;
mod ops;

use {
    crate::error::Error,
    aws_config::{self, profile::ProfileFileCredentialsProvider},
    aws_types::region::Region,
    getopts::{Options, ParsingStyle},
    std::{
        env,
        io::{stderr, stdout, Write},
        process::ExitCode,
    },
};

const INVALID_USAGE: u8 = 2;

#[tokio::main]
async fn main() -> ExitCode {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();
    opts.parsing_style(ParsingStyle::StopAtFirstFree);
    opts.optopt("p", "profile", "Use AWS credentials from the specified profile in ~/.aws/credentials", "<profile>");

    opts.optflag("h", "help", "Print this help menu");
    opts.optopt("r", "region", "Use specified AWS region", "<region>");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            let mut e = stderr();
            writeln!(e, "{}", f).unwrap();
            print_usage(opts, e);
            return ExitCode::from(INVALID_USAGE);
        }
    };

    if matches.opt_present("h") {
        print_usage(opts, stdout());
        return ExitCode::SUCCESS;
    }

    if matches.free.is_empty() {
        let mut e = stderr();
        writeln!(e, "No operation specified").unwrap();
        print_usage(opts, e);
        return ExitCode::from(INVALID_USAGE);
    }

    let mut config = aws_config::from_env();
    if let Some(region) = matches.opt_str("r") {
        config = config.region(Region::new(region));
    }

    if let Some(profile) = matches.opt_str("p") {
        let creds = ProfileFileCredentialsProvider::builder().profile_name(profile).build();
        config = config.credentials_provider(creds);
    }

    let sdk_config = config.load().await;
    let ec2_config = aws_sdk_ec2::config::Builder::from(&sdk_config).build();
    let ec2 = aws_sdk_ec2::Client::from_conf(ec2_config);

    let (op_name, op_args) = matches.free.split_first().unwrap();
    let op_args = op_args.to_vec();

    let result = match op_name.as_str() {
        "print" => ops::print_instances::print_instances(ec2, op_args).await,
        "reboot" => ops::reboot_instances(ec2, op_args).await,
        "set-no-stop-before" => ops::set_no_stop::set_no_stop_before(ec2, op_args).await,
        "start" => ops::start_instances(ec2, op_args).await,
        "stop" => ops::stop_instances(ec2, op_args).await,
        "terminate" => ops::terminate_instances(ec2, op_args).await,
        _ => {
            eprintln!("Unknown operation {}", op_name);
            print_usage(opts, stderr());
            return ExitCode::from(INVALID_USAGE);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::ShowUsage) => {
            print_usage(opts, stdout());
            ExitCode::SUCCESS
        }
        Err(Error::InvalidUsage(msg)) => {
            eprintln!("Invalid usage: {}", msg);
            print_usage(opts, stderr());
            ExitCode::from(INVALID_USAGE)
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage<W: Write>(opts: Options, mut out: W) {
    let brief = "Usage: ec2-by-name [options] <operation> <instance-name>...";
    let usage = opts.usage(brief);
    out.write_all(usage.as_bytes()).unwrap();

    out.write_all(
        r#"Operations:
    info <name>...         Print instance information
    print <name>...        Print instance ids
    reboot <name>...       Reboot instances
    set-no-stop-before --time <time> | --duration <duration>
                           Set the NoStopBefore tag to the time or duration
    start <name>...        Start instances
    stop <name>...         Stop instances
    terminate <name>...    Terminate instances
"#
        .as_bytes(),
    )
    .unwrap();
}
