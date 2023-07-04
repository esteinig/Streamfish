//! Tonic builder

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &[
                "proto/dori_api/adaptive.proto",
                
                "proto/minknow_api/minion_device.proto",
                "proto/minknow_api/data.proto",
                "proto/minknow_api/protocol.proto",
                "proto/minknow_api/statistics.proto",
                "proto/minknow_api/acquisition.proto",
                "proto/minknow_api/manager.proto",
                "proto/minknow_api/protocol_settings.proto",
                "proto/minknow_api/basecaller.proto",
                "proto/minknow_api/analysis_configuration.proto",
                "proto/minknow_api/promethion_device.proto",
                "proto/minknow_api/instance.proto",
                "proto/minknow_api/log.proto",
                "proto/minknow_api/keystore.proto",
                "proto/minknow_api/rpc_options.proto",
                "proto/minknow_api/device.proto",
            ],
            &["proto/"],
        )?;
    Ok(())
}