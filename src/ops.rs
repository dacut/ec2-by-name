pub(crate) mod print_instances;
pub(crate) mod set_no_stop;

use {
    crate::{ec2::find_instances_then, error::NResult},
    aws_sdk_ec2::{
        self,
    },
    aws_sdk_ec2::{
        model::{InstanceState, InstanceStateChange},
    },
};

pub(crate) async fn reboot_instances(ec2: aws_sdk_ec2::Client, args: Vec<String>) -> NResult {
    find_instances_then(ec2.clone(), args, |instance_ids| async move {
        println!("Rebooting instances: {}", instance_ids.join(" "));
        ec2.reboot_instances().set_instance_ids(Some(instance_ids.clone())).send().await?;
        println!("Rebooted instances: {}", instance_ids.join(" "));
        Ok(())
    })
    .await
}

pub(crate) async fn start_instances(ec2: aws_sdk_ec2::Client, args: Vec<String>) -> NResult {
    find_instances_then(ec2.clone(), args, |instance_ids| async move {
        println!("Starting instances: {}", instance_ids.join(" "));
        let output = ec2.start_instances().set_instance_ids(Some(instance_ids)).send().await?;
        print_instance_state_changes(output.starting_instances);
        Ok(())
    })
    .await
}

pub(crate) async fn stop_instances(ec2: aws_sdk_ec2::Client, args: Vec<String>) -> NResult {
    find_instances_then(ec2.clone(), args, |instance_ids| async move {
        println!("Stopping instances: {}", instance_ids.join(" "));
        let output = ec2.stop_instances().set_instance_ids(Some(instance_ids)).send().await?;
        print_instance_state_changes(output.stopping_instances);
        Ok(())
    })
    .await
}

pub(crate) async fn terminate_instances(ec2: aws_sdk_ec2::Client, args: Vec<String>) -> NResult {
    find_instances_then(ec2.clone(), args, |instance_ids| async move {
        println!("Terminating instances: {}", instance_ids.join(" "));
        let output = ec2.terminate_instances().set_instance_ids(Some(instance_ids)).send().await?;
        print_instance_state_changes(output.terminating_instances);
        Ok(())
    })
    .await
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
