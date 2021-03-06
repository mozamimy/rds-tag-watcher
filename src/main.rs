extern crate aws_lambda as lambda;
extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate rusoto_core;
extern crate rusoto_rds;
extern crate rusoto_sns;

use rusoto_rds::Rds;
use rusoto_sns::Sns;

fn main() {
    let _ = env_logger::try_init();

    lambda::start(|_: lambda::event::cloudwatch_events::CloudWatchEvent| {
        let rds = rusoto_rds::RdsClient::new(rusoto_core::Region::default());

        let mut marker: Option<String> = None;
        let mut db_instances = Vec::<rusoto_rds::DBInstance>::new();
        loop {
            let mut rds_input: rusoto_rds::DescribeDBInstancesMessage = Default::default();
            rds_input.marker = marker.clone();

            match rds.describe_db_instances(rds_input).sync() {
                Err(e) => return Err(format_err!("{:?}", e)),
                Ok(r) => {
                    match r.db_instances {
                        Some(i) => db_instances.append(&mut i.clone()),
                        _ => { /* do nothing */ }
                    }
                    marker = r.marker.clone();
                }
            }

            if marker == None {
                break;
            }
        }

        let mut db_instance_arns = Vec::new();
        let tag = std::env::var("TAG")?;
        for db_instance in &db_instances {
            match rds
                .list_tags_for_resource(rusoto_rds::ListTagsForResourceMessage {
                    resource_name: db_instance.clone().db_instance_arn.unwrap(),
                    filters: None,
                })
                .sync()
            {
                Err(e) => return Err(format_err!("{:?}", e)),
                Ok(r) => {
                    if r.tag_list.unwrap().iter().find(|&t| t.clone().key.unwrap() == tag) == None {
                        db_instance_arns.push(db_instance.clone().db_instance_arn.unwrap());
                    }
                }
            }
        }

        if db_instance_arns.len() == 0 {
            info!("There are no instances to notify.");
            return Ok("ok")
        } else {
            let sns = rusoto_sns::SnsClient::new(rusoto_core::Region::default());
            let mut publish_input: rusoto_sns::PublishInput = Default::default();
            publish_input.topic_arn = Some(std::env::var("SNS_TOPIC_ARN")?);
            publish_input.subject = Some(std::env::var("SNS_SUBJECT").unwrap_or(format!("*Attach {} tag please!*", tag)));
            publish_input.message = db_instance_arns.join("\n");

            match sns.publish(publish_input).sync() {
                Err(e) => return Err(format_err!("{:?}", e)),
                Ok(_) => return Ok("ok"),
            }
        }
    })
}
