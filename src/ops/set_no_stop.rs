use {
    crate::{
        ec2::find_instances_then,
        error::{Error, NResult},
    },
    aws_sdk_ec2::{self, model::Tag},
    chrono::{DateTime, Duration, Utc},
    getopts::Options,
    humantime::{parse_duration, parse_rfc3339_weak},
    std::time::UNIX_EPOCH,
};

pub(crate) async fn set_no_stop_before(ec2: aws_sdk_ec2::Client, args: Vec<String>) -> NResult {
    let mut opts = Options::new();
    opts.optopt("d", "duration", "Duration for no-stop-before", "<duration>");
    opts.optopt("t", "time", "Time for no-stop-before", "<time>");
    opts.optflag("h", "help", "Print this help menu");

    let matches = opts.parse(args)?;
    if matches.opt_present("h") {
        return Err(Error::ShowUsage);
    }

    if matches.opt_present("d") && matches.opt_present("t") {
        eprintln!("Cannot specify both duration and time");
        return Err(Error::InvalidUsage("Cannot specify both duration and time".to_string()));
    }

    let duration = if let Some(duration_str) = matches.opt_str("d") {
        parse_duration(&duration_str)?
    } else if let Some(time_str) = matches.opt_str("t") {
        parse_rfc3339_weak(&time_str)?.duration_since(UNIX_EPOCH).expect("Time cannot be represented since Unix epoch")
    } else {
        return Err(Error::InvalidUsage("Must specify either duration or time".to_string()));
    };

    let duration = Duration::from_std(duration).expect("Failed to convert system duration to Chrono duration");
    let timestamp: DateTime<Utc> = Utc::now() + duration;
    let timestamp_str: String = timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let tag = Tag::builder().key("NoStopBefore").value(timestamp_str.clone()).build();

    find_instances_then(ec2.clone(), matches.free, |instance_ids| async move {
        println!("Setting NoStopBefore for instances: {}", instance_ids.join(" "));
        ec2.create_tags().set_resources(Some(instance_ids.clone())).tags(tag).send().await?;
        println!("Set NoStopBefore to {} for instances: {}", timestamp_str, instance_ids.join(" "));
        Ok(())
    })
    .await
}
