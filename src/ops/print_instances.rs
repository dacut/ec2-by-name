use crate::{ec2::find_instances_then, error::Error};

pub(crate) async fn print_instances(ec2: aws_sdk_ec2::Client, args: Vec<String>) -> Result<(), Error> {
    find_instances_then(ec2, args, |instance_ids| async move {
        println!("{}", instance_ids.join(" "));
        Ok(())
    })
    .await
}
