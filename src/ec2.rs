use {
    async_std_resolver::resolver_from_system_conf,
    aws_sdk_ec2::{
        self,
        model::{Filter as Ec2Filter},
    },
    crate::error::{NResult, Result},
    futures::stream::{FuturesOrdered, StreamExt},
    log::{debug, error},
    std::{
        collections::HashSet,
        future::Future,
        net::IpAddr,
        pin::Pin,
    },
};

pub(crate) async fn find_instances_then<F, Ret>(ec2: aws_sdk_ec2::Client, names: Vec<String>, then: F) -> NResult
where
    F: FnOnce(Vec<String>) -> Ret,
    Ret: Future<Output = NResult>,
{
    let mut futures = FuturesOrdered::new();

    for name in names {
        debug!("Dispatching find_instances {}", name);
        let future = find_instances(ec2.clone(), name);
        futures.push_back(future);
    }

    let mut all_instance_ids = HashSet::new();
    let mut first_error = None;
    while let Some(result) = futures.next().await {
        match result {
            Ok(instance_ids) => {
                for instance_id in instance_ids {
                    all_instance_ids.insert(instance_id);
                }
            }

            Err(e) => {
                error!("Error finding instances: {}", e);
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        };
    }

    if let Some(e) = first_error {
        return Err(e);
    }

    let mut all_instance_ids: Vec<String> = all_instance_ids.into_iter().collect();
    all_instance_ids.sort();

    then(all_instance_ids).await
}

pub(crate) async fn find_instances(ec2: aws_sdk_ec2::Client, name: String) -> Result<HashSet<String>> {
    let resolver = resolver_from_system_conf().await?;
    let mut futures = FuturesOrdered::new();
    for ip_addr in resolver.lookup_ip(name.clone()).await? {
        debug!("Found IP address {} for {}", ip_addr, name);
        let future = find_instances_by_ip(ec2.clone(), ip_addr);
        futures.push_back(future);
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

pub(crate) async fn find_instances_by_ip(
    ec2: aws_sdk_ec2::Client,
    address: IpAddr,
) -> Result<HashSet<String>> {
    debug!("Finding instances with IP address {}", address);
    let mut futures =
        FuturesOrdered::<Pin<Box<dyn Future<Output = Result<HashSet<String>>>>>>::new();
    futures.push_back(Box::pin(find_instances_by_public_ipv4(ec2.clone(), address)));
    futures.push_back(Box::pin(find_instances_by_public_eip_ipv4(ec2.clone(), address)));
    futures.push_back(Box::pin(find_instances_by_private_ipv4(ec2.clone(), address)));
    futures.push_back(Box::pin(find_instances_by_private_netif_ipv4(ec2.clone(), address)));
    futures.push_back(Box::pin(find_instances_by_netif_ipv6(ec2.clone(), address)));

    let mut all_instance_ids = HashSet::new();

    while let Some(result) = futures.next().await {
        match result {
            Ok(instance_ids) => {
                for instance_id in instance_ids {
                    all_instance_ids.insert(instance_id);
                }
            }
            Err(e) => return Err(e),
        }
    }

    Ok(all_instance_ids)
}

pub(crate) async fn find_instances_by_public_ipv4(
    ec2: aws_sdk_ec2::Client,
    address: IpAddr,
) -> Result<HashSet<String>> {
    let filter = match address {
        IpAddr::V4(addr) => Ec2Filter::builder().name("ip-address").values(addr.to_string()).build(),
        IpAddr::V6(_) => return Ok(HashSet::new()),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

pub(crate) async fn find_instances_by_public_eip_ipv4(
    ec2: aws_sdk_ec2::Client,
    address: IpAddr,
) -> Result<HashSet<String>> {
    let filter = match address {
        IpAddr::V4(addr) => Ec2Filter::builder()
            .name("network-interface.addresses.association.public-ip")
            .values(addr.to_string())
            .build(),
        IpAddr::V6(_) => return Ok(HashSet::new()),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

pub(crate) async fn find_instances_by_private_ipv4(
    ec2: aws_sdk_ec2::Client,
    address: IpAddr,
) -> Result<HashSet<String>> {
    let filter = match address {
        IpAddr::V4(addr) => Ec2Filter::builder().name("private-ip-address").values(addr.to_string()).build(),
        IpAddr::V6(_) => return Ok(HashSet::new()),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

pub(crate) async fn find_instances_by_private_netif_ipv4(
    ec2: aws_sdk_ec2::Client,
    address: IpAddr,
) -> Result<HashSet<String>> {
    let filter = match address {
        IpAddr::V4(addr) => {
            Ec2Filter::builder().name("network-interface.addresses.private-ip-address").values(addr.to_string()).build()
        }
        IpAddr::V6(_) => return Ok(HashSet::new()),
    };

    get_instance_ids_by_filter(ec2, filter).await
}

pub(crate) async fn find_instances_by_netif_ipv6(
    ec2: aws_sdk_ec2::Client,
    address: IpAddr,
) -> Result<HashSet<String>> {
    debug!("Finding instances with IPv6 address {}", address);

    let filter = match address {
        IpAddr::V4(_) => return Ok(HashSet::new()),
        IpAddr::V6(addr) => {
            Ec2Filter::builder().name("network-interface.ipv6-addresses.ipv6-address").values(addr.to_string()).build()
        }
    };

    get_instance_ids_by_filter(ec2, filter).await
}

pub(crate) async fn get_instance_ids_by_filter(
    ec2: aws_sdk_ec2::Client,
    filter: Ec2Filter,
) -> Result<HashSet<String>> {
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

