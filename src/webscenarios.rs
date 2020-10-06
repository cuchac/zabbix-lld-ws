pub mod webscenarios {
    use std::collections::HashMap;

    use serde::Deserialize;
    use serde::Serialize;

    use crate::errors::errors::OperationError;
    use crate::http::http::send_post_request;
    use crate::types::types::{EmptyResult, OperationResult};
    use crate::zabbix::zabbix;
    use crate::zabbix::zabbix::ZabbixRequest;

    #[derive(Deserialize)]
    pub struct ZabbixWebScenario {
        pub name: String
    }

    #[derive(Serialize)]
    struct GetWebScenariosRequest {
        jsonrpc: String,
        method: String,
        params: GetWebScenariosRequestParams,
        auth: String,
        id: u8
    }

    #[derive(Serialize)]
    struct GetWebScenariosRequestParams {
        search: HashMap<String, String>
    }

    #[derive(Deserialize)]
    struct WebScenariosResponse {
        result: Vec<ZabbixWebScenario>
    }

    #[derive(Serialize)]
    struct CreateRequest {
        jsonrpc: String,
        method: String,
        params: CreateRequestParams,
        auth: String,
        id: u8
    }

    #[derive(Serialize)]
    struct CreateRequestParams {
        name: String,
        hostid: String,
        steps: Vec<WebScenarioStep>
    }

    #[derive(Serialize)]
    struct WebScenarioStep {
        name: String,
        url: String,
        status_codes: String,
        no: u8
    }

    pub fn find_web_scenarios(client: &reqwest::blocking::Client,
                              api_endpoint: &str, auth_token: &str) ->
                                                        OperationResult<Vec<ZabbixWebScenario>> {
        println!("searching web scenarios..");

        let mut search_params = HashMap::new();
        search_params.insert("key_".to_string(), "Check index page '".to_string());

        let params = GetWebScenariosRequestParams {
            search: search_params
        };

        let request: ZabbixRequest<GetWebScenariosRequestParams> = ZabbixRequest::new(
            "httptest.get", params, auth_token
        );

        match send_post_request(client, api_endpoint, request) {
            Ok(response) => {
                let search_response: WebScenariosResponse = serde_json::from_str(&response)
                                            .expect(zabbix::UNSUPPORTED_RESPONSE_MESSAGE);
                Ok(search_response.result)
            }
            Err(_) => {
                error!("unable to find zabbix items");
                Err(OperationError::Error)
            }
        }
    }

    pub fn create_web_scenario(client: &reqwest::blocking::Client,
                               api_endpoint: &str, auth_token: &str,
                               item_url: &str, host_id: &str) -> EmptyResult {
        println!("creating web scenario for '{}'", item_url);

        let mut search_params = HashMap::new();
        search_params.insert("key_".to_string(), "Check index page '".to_string());

        let scenario_name = format!("Check index page '{}'", item_url);

        let step = WebScenarioStep {
            name: "Get page".to_string(),
            url: item_url.to_string(),
            status_codes: "200".to_string(),
            no: 1
        };

        let params = CreateRequestParams {
            name: scenario_name,
            hostid: host_id.to_string(),
            steps: vec![step]
        };

        let request: ZabbixRequest<CreateRequestParams> = ZabbixRequest::new(
            "httptest.create", params, auth_token
        );

        match send_post_request(client, api_endpoint, request) {
            Ok(_) => {
                info!("web scenario has been created for '{}'", item_url);
                Ok(())
            }
            Err(_) => {
                error!("unable to create web scenario for '{}'", item_url);
                Err(OperationError::Error)
            }
        }
    }
}
