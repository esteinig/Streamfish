
use crate::services::minknow_api::manager::manager_service_client::ManagerServiceClient;

use crate::services::minknow_api::instance::{
    GetVersionInfoResponse
};
use crate::services::minknow_api::manager::{
    GetVersionInfoRequest, 
    FlowCellPositionsRequest, 
    FlowCellPositionsResponse,
    AddSimulatedDeviceRequest,
    RemoveSimulatedDeviceRequest,
    SimulatedDeviceType,
    FlowCellPosition,
    flow_cell_position::State
};
use crate::services::minknow_api::device::get_device_info_response::DeviceType;


use tonic::Request;
use tonic::transport::Channel;
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::InterceptedService;

use crate::client::auth::AuthInterceptor;


// A wrapper around the MangerServiceClient, which requests 
// data and transforms responses for custom applications
pub struct ManagerClient {
    // A client instance with an active channel
    pub client: ManagerServiceClient<InterceptedService<Channel, AuthInterceptor>>
}
impl ManagerClient {
    pub fn new(channel: Channel, token: String) -> Self {

        let token: MetadataValue<Ascii> = token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = ManagerServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Self { client }
    }    
    // Get the current version information
    pub async fn get_version_info(&mut self) -> Result<GetVersionInfoResponse, Box<dyn std::error::Error>>  {

        let request = Request::new(GetVersionInfoRequest {});
        let response = self.client.get_version_info(request).await?.into_inner();

        Ok(response)
    }
    // Get the current flow cell positions 
    //
    // Provides a snapshot of places where users can insert flow cells. It has a streamed response
    // in case there are too many positions to fit into a single response, but normally there should
    // only be a single response.
    pub async fn get_flow_cell_positions(&mut self) -> Result<FlowCellPositionsResponse, Box<dyn std::error::Error>>  {

        let request = Request::new(FlowCellPositionsRequest {});
        let mut stream = self.client.flow_cell_positions(request).await?.into_inner();

        let mut responses = Vec::new();
        while let Some(position_response) = stream.message().await? {
            responses.push(position_response)
        }

        if responses.len() > 1 {
            log::debug!("Streamfish::ManagerClient: too many flow cell positions were returned to fit into a single response");
            log::debug!("Streamfish::ManagerClient: only the first flow cell position response is returned");
        }

        match responses.get(0) {
            Some(position_response) => Ok(position_response.to_owned()),
            None => panic!("Streamfish::ManagerClient: could not obtain the required position response")
        }
    }
    // Add a simulated device
    //
    // The name of the position, this must be unique and the correct format:
    //
    // For MinION Mk1B and Mk1C: "MS" followed by five digits, eg: "MS12345".
    // For GridION: "GS" followed by five digits, eg: "GS12345".
    // For P2 Solo: "P2S_" followed by five digits, and then "-A" or "-B" eg: "P2S_12345-A".
    //
    // PromethION and P2 Solo position-names have no format restriction, but must be unique. It is
    // strongly recommended to follow standard naming conventions:
    //
    // For PromethION: start with "1A" and then increase the number and/or the letter as you add
    // more positions.
    // For P2 Solo: use "P2S_00000-A" and "P2S_00000-B" (these fit the format of real P2 Solo devices,
    // but do not correspond to any real device).
    //
    // Future versions might constrain PromethION and P2 Solo device names. Using the above
    // suggestions should ensure that your code will continue to work.
    //
    // Note that MinKNOW Core 5.5 and earlier required the P2 Solo device name to be "P2S" followed
    // by four digits. This is no longer recommended.
    pub async fn add_simulated_device(&mut self, device_name: &str, device_type: SimulatedDeviceType) -> Result<(), Box<dyn std::error::Error>>  {

        let request = Request::new(AddSimulatedDeviceRequest { 
            name: device_name.to_string(), r#type: device_type.into()
        });
        self.client.add_simulated_device(request).await?.into_inner();

        Ok(())
    }
    // Remove a simulated device [AUTHENTICATION REQUIRED]
    pub async fn remove_simulated_device(&mut self, device_name: &str) -> Result<(), Box<dyn std::error::Error>>  {

        let request = Request::new(RemoveSimulatedDeviceRequest { 
            name: device_name.to_string()
        });
        self.client.remove_simulated_device(request).await?.into_inner();

        Ok(())
    }

}

// Display formatting of responses and data contained in responses for logging

impl std::fmt::Display for GetVersionInfoResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let minknow_version = match &self.minknow {
            Some(minknow_version) => minknow_version.full.as_str(),
            None => "not detected"
        };
        write!(f, "{}", minknow_version)
    }
}


impl std::fmt::Display for FlowCellPositionsResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.total_count)
    }
}


impl std::fmt::Display for FlowCellPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {

        let s = format!(
            "{} - {}{} - {}{}",
            self.name,
            match DeviceType::from_i32(self.device_type) { Some(device_type) => device_type.as_str_name(), None => "UNKNOWN DEVICE" },
            match self.is_simulated { true => " (SIMULATED)", false => "" }, 
            match State::from_i32(self.state) { Some(state) => state.as_str_name().trim_start_matches("STATE_"), None => "STATELESS" },
            match &self.rpc_ports { Some(rpc_ports) => format!(" @ PORTS {}|{}", rpc_ports.secure, rpc_ports.secure_grpc_web), None => "".to_string()}
        );

        write!(f, "{}", s)
    }
}

impl SimulatedDeviceType {
    pub fn from_cli(value: &str) -> Self {
        match value {
            "minion" => SimulatedDeviceType::SimulatedMinion,
            "promethion" => SimulatedDeviceType::SimulatedPromethion,
            "p2" => SimulatedDeviceType::SimulatedP2,
            _ => SimulatedDeviceType::SimulatedMinion
        }
    }
}