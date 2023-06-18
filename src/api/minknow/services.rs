pub(crate) mod analysis_configuration {
    tonic::include_proto!("/proto/analysis_configuration");
}
pub(crate) mod acquisition {
    tonic::include_proto!("proto.acquisition");
}
pub(crate) mod basecaller {
    tonic::include_proto!("proto.basecaller");
}
pub(crate) mod data {
    tonic::include_proto!("proto.data");
}
pub(crate) mod device {
    tonic::include_proto!("proto.device");
}
pub(crate) mod instance {
    tonic::include_proto!("proto.instance");
}
pub(crate) mod keystore {
    tonic::include_proto!("proto.keystore");
}
pub(crate) mod log {
    tonic::include_proto!("proto.log");
}
pub(crate) mod manager {
    tonic::include_proto!("proto.manager");
}
pub(crate) mod minion_device {
    tonic::include_proto!("proto.minion_device");
}
pub(crate) mod promethion_device {
    tonic::include_proto!("proto.promethion_device");
}
pub(crate) mod protocol_settings {
    tonic::include_proto!("proto.protocol_settings");
}
pub(crate) mod protocol {
    tonic::include_proto!("proto.protocol");
}
pub(crate) mod statistics {
    tonic::include_proto!("proto.statistics");
}