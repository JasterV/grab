use echo_service::FILE_DESCRIPTOR_SET;
use granc_core::client::{Descriptor, GrancClient};

#[test]
fn test_offline_list_services() {
    let client = GrancClient::offline(FILE_DESCRIPTOR_SET.to_vec())
        .expect("Failed to load file descriptor set");

    let mut services = client.list_services();
    services.sort();

    assert_eq!(services.as_slice(), ["echo.EchoService"]);
}

#[test]
fn test_offline_describe_descriptors() {
    let client = GrancClient::offline(FILE_DESCRIPTOR_SET.to_vec())
        .expect("Failed to load file descriptor set");

    // 1. Success: Service
    let desc = client
        .get_descriptor_by_symbol("echo.EchoService")
        .expect("Service not found");

    assert!(matches!(
        desc,
        Descriptor::ServiceDescriptor(s) if s.name() == "EchoService"
    ));

    // 2. Success: Message
    let desc = client
        .get_descriptor_by_symbol("echo.EchoRequest")
        .expect("Message not found");

    assert!(matches!(
        desc,
        Descriptor::MessageDescriptor(m) if m.name() == "EchoRequest"
    ));

    // 3. Error: Symbol Not Found
    let desc = client.get_descriptor_by_symbol("echo.Ghost");
    assert!(desc.is_none());
}

#[test]
fn test_offline_creation_error() {
    let result = GrancClient::offline(vec![0, 1, 2, 3]);
    assert!(result.is_err());
}
