
use crate::config::MinknowConfig;
use crate::clients::manager::ManagerClient;

use tonic::transport::{ClientTlsConfig, Certificate, Channel};

pub struct ActiveChannels {
    pub manager: Channel
}

pub struct ActiveClients {
    pub manager: ManagerClient
}

// Main client for the MinKnow API
pub struct MinknowClient {
    pub host: String,
    pub port: i32,
    pub token: String,
    pub clients: ActiveClients,
    pub channels: ActiveChannels
}

impl MinknowClient {

    pub async fn connect(config: &MinknowConfig) -> Result<Self, Box<dyn std::error::Error>> {

        // When connecting to MinKnow we require a secure channel (TLS). However, we were getting 
        // an error through the underlying TLS certificate library, solution is documented here.
        //
        // Error: InvalidCertificate(Other(UnsupportedCertVersion)) 
        // 
        // The error code from the `webpki` library states:
        //
        //      The certificate is not a v3 X.509 certificate.
        //
        // Looks like MinKnow is using outdated certificate versions. This issue is relevant and has
        // a solution:
        //
        //      https://github.com/rustls/rustls/issues/127
        //
        // However, we need to modify the ClientTlsConfig in `tonic` to disable certificate verifiction.
        //
        // The insane thing is that this actually worked thanks to the commenter in the issue... holy fuck,
        // this is such a hack. It now requires a modified version of `tonic` which is available as a fork
        // on my GitHub (https://github.com/esteinig/tonic @ v0.9.2-r1)

        let cert = std::fs::read_to_string(&config.tls_cert_path).expect(
            &format!("Failed to read certificate from path: {}", &config.tls_cert_path.display()) 
        );
        let tls_ca = Certificate::from_pem(cert);
        let tls_config = ClientTlsConfig::new().domain_name("localhost").ca_certificate(tls_ca);

        // Establish a secure channel to the MinKnow manager service, that will be 
        // available throughout to request data from the manager service
        let manager_uri = format!("https://{}:{}", config.host, config.port);
        let manager_channel = Channel::from_shared(manager_uri)?
            .tls_config(tls_config)?
            .connect()
            .await?;
        
        // Test the connection by getting MinKnow version - we clone the established channel
        // (cheap, see below) and instantiate a new manger service client, that allows us tpo
        // send the implemented requests.

        log::info!("Requesting version from ManagerService");
        let mut manager_client = ManagerClient::new(manager_channel.clone());

        // Get the version information to test the connection and print the version of MinKnow
        let response = manager_client.get_version_info().await?;
        log::info!("MinKnow version is: v{}", response);

        // Get the flow cell positions and print their total count
        let positions = manager_client.get_flow_cell_positions().await?;
        log::info!("Flow cell positions: {}", positions);

        // # Multiplexing requests [from Tonic]
        //
        // Sending a request on a channel requires a `&mut self` and thus can only send
        // one request in flight. This is intentional and is required to follow the `Service`
        // contract from the `tower` library which this channel implementation is built on
        // top of.
        //
        // `tower` itself has a concept of `poll_ready` which is the main mechanism to apply
        // back pressure. `poll_ready` takes a `&mut self` and when it returns `Poll::Ready`
        // we know the `Service` is able to accept only one request before we must `poll_ready`
        // again. Due to this fact any `async fn` that wants to poll for readiness and submit
        // the request must have a `&mut self` reference.
        //
        // To work around this and to ease the use of the channel, `Channel` provides a
        // `Clone` implementation that is _cheap_. This is because at the very top level
        // the channel is backed by a `tower_buffer::Buffer` which runs the connection
        // in a background task and provides a `mpsc` channel interface. Due to this
        // cloning the `Channel` type is cheap and encouraged.
        //
        // Performance wise we have found you can get a decent amount of throughput by just
        // clonling a single channel. Though the best comes when you load balance a few 
        // channels and also clone them around (https://github.com/hyperium/tonic/issues/285)

        // I may have to revisit this as the requests get more complex, e.g. on streaming 
        // acquisition data or sending off reads for basecalling.
    
        let minknow_channels = ActiveChannels {
            manager: manager_channel
        };

        let minknow_clients = ActiveClients {
            manager: manager_client
        };

        Ok(Self {
            host: config.host.to_owned(),
            port: config.port.to_owned(),
            token: config.token.to_owned(),
            clients: minknow_clients,
            channels: minknow_channels
        })
    }
}




pub struct ReadUntilClient {

}