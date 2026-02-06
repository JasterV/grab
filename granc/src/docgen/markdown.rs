use super::package::{Package, Packages};
use crate::formatter::FormattedString;
use granc_core::prost_reflect::{EnumDescriptor, Kind, MessageDescriptor, ServiceDescriptor};
use std::fs;
use std::path::PathBuf;

pub fn generate(output_dir: PathBuf, service: ServiceDescriptor) -> std::io::Result<()> {
    // Disable colors for plain text generation
    colored::control::set_override(false);

    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)?;
    }

    let packages = Packages::from(service.clone());

    for package in packages.values() {
        let filename = format!("{}.md", package.name);
        let path = output_dir.join(&filename);

        let out = generate_package_file(package)?;

        fs::write(path, out)?;
        println!("Generated: {}", filename);
    }

    let path = output_dir.join("index.md");
    let out = generate_index(&service, &packages)?;
    fs::write(path, out)?;
    println!("Generated: index.md");

    // Restore colors
    colored::control::unset_override();
    Ok(())
}

fn generate_index(
    entry_service: &ServiceDescriptor,
    packages: &Packages,
) -> std::io::Result<String> {
    let mut out = String::new();

    out.push_str("# Documentation Index\n\n\n");

    let svc_package = entry_service.package_name();
    let svc_link = format!("{}.md#{}", svc_package, entry_service.name());

    out.push_str("## Service\n\n");
    out.push_str(&format!("- [**{}**]({})\n", entry_service.name(), svc_link));

    out.push_str("\n## Packages\n\n");

    // Collect package names (Google packages included)
    let mut package_names: Vec<_> = packages.names().collect();
    package_names.sort();

    if package_names.is_empty() {
        out.push_str("*None*\n");
    } else {
        for name in package_names {
            out.push_str(&format!("- [{}]({}.md)\n", name, name));
        }
    }

    Ok(out)
}

fn generate_package_file(package: &Package) -> std::io::Result<String> {
    let mut out = String::new();

    let mut services = package.services.clone();
    services.sort_by(|a, b| a.name().cmp(b.name()));

    for service in services {
        write_anchor(&mut out, service.name());
        out.push_str(&format!("## {}\n\n", service.name()));
        write_service_content(&mut out, &service);
        out.push_str("---\n\n");
    }

    let mut messages = package.messages.clone();
    messages.sort_by(|a, b| a.name().cmp(b.name()));

    for message in messages {
        write_anchor(&mut out, message.name());
        out.push_str(&format!("## {}\n\n", message.name()));
        write_message_content(&mut out, &message);
        out.push_str("---\n\n");
    }

    let mut enums = package.enums.clone();
    enums.sort_by(|a, b| a.name().cmp(b.name()));

    for enum_desc in enums {
        write_anchor(&mut out, enum_desc.name());
        out.push_str(&format!("## {}\n\n", enum_desc.name()));
        write_enum_content(&mut out, &enum_desc);
        out.push_str("---\n\n");
    }

    Ok(out)
}

fn write_anchor(out: &mut String, name: &str) {
    out.push_str(&format!("<a id=\"{name}\"></a>\n"));
}

fn write_service_content(out: &mut String, service: &ServiceDescriptor) {
    out.push_str("### Definition\n\n```protobuf\n");
    out.push_str(&format!("package {};\n\n", service.package_name()));
    out.push_str(&FormattedString::from(service.clone()).0);
    out.push_str("\n```\n\n");

    out.push_str("### Methods\n\n");
    for method in service.methods() {
        out.push_str(&format!("#### `{}`\n\n", method.name()));

        let input = method.input();
        let output = method.output();

        let input_link = resolve_link(input.package_name(), input.name());
        let output_link = resolve_link(output.package_name(), output.name());

        out.push_str(&format!("- Request: [{}]({})\n", input.name(), input_link));
        out.push_str(&format!(
            "- Response: [{}]({})\n",
            output.name(),
            output_link
        ));
        out.push('\n');
    }
}

fn write_message_content(out: &mut String, message: &MessageDescriptor) {
    out.push_str("### Definition\n\n```protobuf\n");
    out.push_str(&format!("package {};\n\n", message.package_name()));
    out.push_str(&FormattedString::from(message.clone()).0);
    out.push_str("\n```\n\n");

    out.push_str("### Dependencies\n\n");
    let mut has_deps = false;

    for field in message.fields() {
        match field.kind() {
            Kind::Message(m) => {
                has_deps = true;
                let link = resolve_link(m.package_name(), m.name());
                out.push_str(&format!(
                    "- Field `{}`: [{}]({})\n",
                    field.name(),
                    m.name(),
                    link
                ));
            }
            Kind::Enum(e) => {
                has_deps = true;
                let link = resolve_link(e.package_name(), e.name());
                out.push_str(&format!(
                    "- Field `{}`: [{}]({})\n",
                    field.name(),
                    e.name(),
                    link
                ));
            }
            _ => {}
        }
    }

    if !has_deps {
        out.push_str("*None*\n");
    }
    out.push('\n');
}

fn write_enum_content(out: &mut String, enum_desc: &EnumDescriptor) {
    out.push_str("### Definition\n\n```protobuf\n");
    out.push_str(&format!("package {};\n\n", enum_desc.package_name()));
    out.push_str(&FormattedString::from(enum_desc.clone()).0);
    out.push_str("\n```\n\n");
}

fn resolve_link(package: &str, name: &str) -> String {
    // Always link to local file + anchor
    format!("{}.md#{}", package, name)
}
