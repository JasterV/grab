use granc_core::{
    client::Descriptor,
    prost_reflect::{EnumDescriptor, Kind, MessageDescriptor, ServiceDescriptor},
};
use std::collections::{HashMap, hash_map::Keys};

/// Represents a single protobuffer package.
///
/// It contains all the services, messages and enums described in the package.
pub(crate) struct Package {
    pub name: String,
    pub services: Vec<ServiceDescriptor>,
    pub messages: Vec<MessageDescriptor>,
    pub enums: Vec<EnumDescriptor>,
}

impl Package {
    fn new(name: String) -> Self {
        Package {
            name,
            services: vec![],
            messages: vec![],
            enums: vec![],
        }
    }

    fn push_descriptor(&mut self, descriptor: Descriptor) {
        match descriptor {
            Descriptor::MessageDescriptor(v) => self.messages.push(v),
            Descriptor::ServiceDescriptor(v) => self.services.push(v),
            Descriptor::EnumDescriptor(v) => self.enums.push(v),
        }
    }
}

impl From<Descriptor> for Package {
    fn from(value: Descriptor) -> Self {
        let package_name = value.package_name().to_string();
        let mut package = Package::new(package_name);
        package.push_descriptor(value);
        package
    }
}

/// Represents a collection of protobuffer packages, each containing the corresponding file descriptors
pub(crate) struct Packages(HashMap<String, Package>);

impl Packages {
    pub fn values(&self) -> std::collections::hash_map::Values<'_, String, Package> {
        self.0.values()
    }

    pub fn names(&self) -> Keys<'_, String, Package> {
        self.0.keys()
    }
}

impl From<ServiceDescriptor> for Packages {
    fn from(value: ServiceDescriptor) -> Self {
        let mut descriptors = collect_service_dependencies(&value);

        descriptors.insert(
            value.full_name().to_string(),
            Descriptor::ServiceDescriptor(value),
        );

        let packages = group_descriptors_by_package(descriptors.into_values());
        Packages(packages)
    }
}

fn group_descriptors_by_package(
    descriptors: impl IntoIterator<Item = Descriptor>,
) -> HashMap<String, Package> {
    descriptors
        .into_iter()
        .fold(HashMap::new(), |mut acc, descriptor| {
            let package_name = descriptor.package_name();

            match acc.get_mut(package_name) {
                Some(package) => package.push_descriptor(descriptor),
                None => {
                    let _ = acc.insert(package_name.to_string(), Package::from(descriptor));
                }
            }

            acc
        })
}

fn collect_service_dependencies(service: &ServiceDescriptor) -> HashMap<String, Descriptor> {
    service
        .methods()
        .flat_map(|m| [m.input(), m.output()])
        .fold(HashMap::new(), |mut acc, d| {
            let message_name = d.full_name().to_string();

            if acc.contains_key(&message_name) {
                return acc;
            }

            acc.insert(message_name, Descriptor::MessageDescriptor(d.clone()));

            collect_message_dependencies(acc, &d)
        })
}

fn collect_message_dependencies(
    descriptors: HashMap<String, Descriptor>,
    message: &MessageDescriptor,
) -> HashMap<String, Descriptor> {
    message
        .fields()
        .fold(descriptors, |mut acc, field| match field.kind() {
            Kind::Message(m) => {
                let message_name = m.full_name().to_string();

                if acc.contains_key(&message_name) {
                    return acc;
                }

                acc.insert(message_name, Descriptor::MessageDescriptor(m.clone()));

                collect_message_dependencies(acc, &m)
            }
            Kind::Enum(e) => {
                acc.insert(e.full_name().to_string(), Descriptor::EnumDescriptor(e));
                acc
            }
            _ => acc,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use granc_core::prost_reflect::DescriptorPool;
    use granc_test_support::compiler;

    fn compile_protos(files: &[(&str, &str)]) -> DescriptorPool {
        let file_descriptor_set = compiler::compile_protos(files);
        DescriptorPool::from_file_descriptor_set(file_descriptor_set)
            .expect("Failed to decode descriptor pool")
    }

    #[test]
    fn test_package_collection_with_deduplication() {
        let proto = r#"
            syntax = "proto3";
            package test;

            enum Status {
                UNKNOWN = 0;
                OK = 1;
            }

            message Request {
                Status status = 1;
            }

            message Response {
                Status status = 1;
            }

            service MyService {
                rpc DoSomething(Request) returns (Response);
            }
        "#;

        let pool = compile_protos(&[("test.proto", proto)]);
        let service = pool
            .get_service_by_name("test.MyService")
            .expect("Service not found");

        // --- Act ---
        let packages = Packages::from(service);

        // --- Assert ---
        let test_package = packages.0.get("test").expect("Package 'test' missing");

        // Verify Services
        assert_eq!(test_package.services.len(), 1);
        assert_eq!(test_package.services[0].name(), "MyService");

        // Verify Messages
        assert_eq!(test_package.messages.len(), 2);
        let msg_names: Vec<_> = test_package.messages.iter().map(|m| m.name()).collect();
        assert!(msg_names.contains(&"Request"));
        assert!(msg_names.contains(&"Response"));

        // Verify Enums (Deduplication Check)
        assert_eq!(
            test_package.enums.len(),
            1,
            "Enum should appear exactly once"
        );
        assert_eq!(test_package.enums[0].name(), "Status");
    }

    #[test]
    fn test_circular_dependency_handling() {
        let proto = r#"
            syntax = "proto3";
            package cycle;

            message NodeA {
                NodeB child = 1;
            }

            message NodeB {
                NodeA parent = 1;
            }

            service Cycler {
                rpc Cycle(NodeA) returns (NodeA);
            }
        "#;

        let pool = compile_protos(&[("cycle.proto", proto)]);
        let service = pool
            .get_service_by_name("cycle.Cycler")
            .expect("Service not found");

        // --- Act ---
        let packages = Packages::from(service);

        // --- Assert ---
        let pkg = packages.0.get("cycle").expect("Package 'cycle' missing");

        assert_eq!(pkg.messages.len(), 2);
        let names: Vec<_> = pkg.messages.iter().map(|m| m.name()).collect();
        assert!(names.contains(&"NodeA"));
        assert!(names.contains(&"NodeB"));
    }

    #[test]
    fn test_multi_file_imports() {
        let common_proto = r#"
            syntax = "proto3";
            package common;
            
            message Shared {
                string id = 1;
            }
        "#;

        let app_proto = r#"
            syntax = "proto3";
            package app;
            
            import "common.proto";
            
            service AppService {
                rpc Get(common.Shared) returns (common.Shared);
            }
        "#;

        let pool = compile_protos(&[("common.proto", common_proto), ("app.proto", app_proto)]);

        let service = pool
            .get_service_by_name("app.AppService")
            .expect("Service not found");
        let packages = Packages::from(service);

        // Assert Package 'app'
        let app_pkg = packages.0.get("app").expect("Package 'app' missing");
        assert_eq!(app_pkg.services.len(), 1);

        // Assert Package 'common' (Ensures traversal follows imports)
        let common_pkg = packages.0.get("common").expect("Package 'common' missing");
        assert_eq!(common_pkg.messages.len(), 1);
        assert_eq!(common_pkg.messages[0].name(), "Shared");
    }
}
