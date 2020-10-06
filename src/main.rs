#[macro_use]
extern crate log;
extern crate log4rs;

use std::path::Path;
use std::process::exit;

use clap::{App, Arg};
use regex::Regex;
use reqwest::blocking::Client;

use crate::auth::auth::login_to_zabbix_api;
use crate::config::config::{load_config_from_file, ZabbixConfig};
use crate::errors::errors::OperationError;
use crate::hosts::hosts::{find_hosts, ZabbixHost};
use crate::items::items::{find_zabbix_items, ZabbixItem};
use crate::logging::logging::get_logging_config;
use crate::triggers::triggers::create_trigger;
use crate::types::types::{EmptyResult, OperationResult};
use crate::webscenarios::webscenarios::{create_web_scenario, find_web_scenarios, ZabbixWebScenario};

mod types;

mod config;
mod config_tests;

mod zabbix;
mod auth;

mod items;
mod webscenarios;
mod triggers;
mod hosts;
mod logging;
mod errors;
mod http;

const LOG_LEVEL_ARGUMENT: &str = "log-level";
const LOG_LEVEL_DEFAULT_VALUE: &str = "info";

const ERROR_EXIT_CODE: i32 = 1;

fn main() {
    let matches = App::new("WSZL tool")
        .version("0.3.0")
        .author("Eugene Lebedev <duke.tougu@gmail.com>")
        .about("Add Web scenarios support for Zabbix Low Level Discovery")
        .arg(
            Arg::with_name(LOG_LEVEL_ARGUMENT)
                .help("set logging level. possible values: debug, info, error, warn, trace")
                .long(LOG_LEVEL_ARGUMENT)
                .case_insensitive(true)
                .takes_value(true).required(false)
                .default_value(LOG_LEVEL_DEFAULT_VALUE)
        )
        .get_matches();

    let logging_level: &str = if matches.is_present(LOG_LEVEL_ARGUMENT) {
        matches.value_of(LOG_LEVEL_ARGUMENT).unwrap()
    } else { LOG_LEVEL_DEFAULT_VALUE };

    let logging_config = get_logging_config(logging_level);
    log4rs::init_config(logging_config).unwrap();

    let config_file_path = Path::new("wszl.yml");

    match load_config_from_file(config_file_path) {
        Ok(config) => {
            let client = reqwest::blocking::Client::new();

            match create_web_scenarios_and_triggers(&client, &config.zabbix) {
                Ok(_) => info!("web scenarios and triggers have been created"),
                Err(_) => exit(ERROR_EXIT_CODE)
            }
        }
        Err(_) => error!("unable to load config from file")
    }
}

fn create_web_scenarios_and_triggers(client: &Client, zabbix_config: &ZabbixConfig) -> EmptyResult {
    match login_to_zabbix_api(&client, &zabbix_config.api_endpoint,
                              &zabbix_config.username, &zabbix_config.password) {
        Ok(auth_token) => {
            debug!("login success: token '{}'", auth_token);

            match find_zabbix_objects(client, zabbix_config, &auth_token) {
                Ok(zabbix_objects) => {
                    let url_pattern = Regex::new("^vhost.item\\[(.*)\\]$").unwrap();

                    let mut has_errors = false;

                    for item in &zabbix_objects.items {

                        match create_scenario_and_trigger_for_item(zabbix_config, &auth_token,
                                        client, &url_pattern, &zabbix_objects, item) {
                            Ok(_) => {}
                            Err(_) => has_errors = true
                        }
                    }

                    if has_errors {
                        Err(OperationError::Error)

                    } else {
                        Ok(())
                    }
                }
                Err(_) => {
                    error!("unable to get zabbix objects");
                    Err(OperationError::Error)
                }
            }

        },
        Err(_) => {
            error!("unable to login");
            Err(OperationError::Error)
        }
    }
}

fn find_zabbix_objects(client: &Client, zabbix_config: &ZabbixConfig, auth_token: &str) ->
                                                                    OperationResult<ZabbixObjects> {
    match find_zabbix_items(&client, &zabbix_config.api_endpoint, &auth_token) {
        Ok(items) => {
            debug!("received items:");

            match find_web_scenarios(&client, &zabbix_config.api_endpoint, &auth_token) {
                Ok(web_scenarios) => {
                    debug!("web scenarios have been obtained");

                    let host_ids: Vec<String> = items.iter()
                                    .map(|item| item.hostid.to_string()).collect();

                    match find_hosts(&client, &zabbix_config.api_endpoint, &auth_token, host_ids) {
                        Ok(hosts) => {

                            Ok(
                                ZabbixObjects {
                                    items,
                                    web_scenarios,
                                    hosts
                                }
                            )

                        }
                        Err(_) => {
                            error!("unable to get zabbix hosts by ids");
                            Err(OperationError::Error)
                        }
                    }
                }
                Err(_) => {
                    error!("unable to get zabbix web scenarios");
                    Err(OperationError::Error)
                }
            }
        }
        Err(_) => {
            error!("unable to get zabbix items");
            Err(OperationError::Error)
        }
    }
}

fn create_scenario_and_trigger_for_item(zabbix_config: &ZabbixConfig,
                                        auth_token: &str, client: &Client,
                                        url_pattern: &Regex, zabbix_objects: &ZabbixObjects,
                                        zabbix_item: &ZabbixItem) -> EmptyResult {
    let mut has_errors = false;

    debug!("---------------------------");
    debug!("item: {}", zabbix_item.name);

    if url_pattern.is_match(&zabbix_item.key_) {
        let groups = url_pattern.captures_iter(&zabbix_item.key_).next().unwrap();
        let url = String::from(&groups[1]);
        debug!("- url '{}'", url);

        let scenario_name = format!("Check index page '{}'", url);

        match zabbix_objects.web_scenarios.iter().find(|entity| entity.name == scenario_name) {
            Some(_) => debug!("web scenario has been found for url '{}', skip", url),
            None => {
                debug!("web scenario wasn't found for url '{}', creating..", url);

                match zabbix_objects.hosts.iter().find(|host| host.hostid == zabbix_item.hostid) {
                    Some(host) => {
                        match create_web_scenario(&client, &zabbix_config.api_endpoint, &auth_token, &url, &host.hostid) {
                            Ok(_) => {
                                info!("web scenario has been created for '{}'", url);

                                match create_trigger(&client, &zabbix_config.api_endpoint, &auth_token, &host.host, &url) {
                                    Ok(_) => info!("trigger has been created"),
                                    Err(_) => {
                                        error!("unable to create trigger for url '{}'", url);
                                        has_errors = true;
                                    }
                                }
                            },
                            Err(_) => {
                                error!("unable to create web scenario for url '{}'", url);
                                has_errors = true;
                            }
                        }
                    }
                    None => {
                        error!("host wasn't found by id {}", zabbix_item.hostid);
                        has_errors = true;
                    }
                }
            }
        }

    } else {
        error!("unsupported item format");
        has_errors = true;
    }

    if has_errors {
        Err(OperationError::Error)

    } else {
        Ok(())
    }
}

struct ZabbixObjects {
    items: Vec<ZabbixItem>,
    web_scenarios: Vec<ZabbixWebScenario>,
    hosts: Vec<ZabbixHost>
}
