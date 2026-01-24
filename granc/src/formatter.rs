use colored::*;
use granc_core::{
    client::{ClientConnectError, DynamicCallError, GetDescriptorError, ListServicesError},
    prost_reflect::{EnumDescriptor, Kind, MessageDescriptor, MethodDescriptor, ServiceDescriptor},
};
use tonic::Status;

/// A wrapper struct for a formatted, colored string.
///
/// Implements `Display` so it can be printed directly.
pub struct FormattedString(pub String);

pub struct ServiceList(pub Vec<String>);

impl std::fmt::Display for FormattedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f)?;
        writeln!(f, "{}", self.0)?;
        Ok(())
    }
}

impl From<serde_json::Value> for FormattedString {
    fn from(value: serde_json::Value) -> Self {
        FormattedString(serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()))
    }
}

impl From<Status> for FormattedString {
    fn from(status: Status) -> Self {
        FormattedString(format!(
            "{} code={:?} message={:?}",
            "gRPC Failed:".red().bold(),
            status.code(),
            status.message()
        ))
    }
}

impl From<std::io::Error> for FormattedString {
    fn from(err: std::io::Error) -> Self {
        FormattedString(format!(
            "{}\n\n'{}'",
            "Failed to read file:".red().bold(),
            err
        ))
    }
}

impl From<ClientConnectError> for FormattedString {
    fn from(err: ClientConnectError) -> Self {
        FormattedString(format!("{}\n\n'{}'", "Connection Error:".red().bold(), err))
    }
}

impl From<ListServicesError> for FormattedString {
    fn from(err: ListServicesError) -> Self {
        FormattedString(format!(
            "{}\n\n'{}'",
            "Failed to list services:".red().bold(),
            err
        ))
    }
}

impl From<GetDescriptorError> for FormattedString {
    fn from(err: GetDescriptorError) -> Self {
        FormattedString(format!(
            "{}\n\n'{}'",
            "Symbol Lookup Failed:".red().bold(),
            err
        ))
    }
}

impl From<DynamicCallError> for FormattedString {
    fn from(err: DynamicCallError) -> Self {
        FormattedString(format!("{}\n\n'{}'", "Call Failed:".red().bold(), err))
    }
}

impl From<ServiceList> for FormattedString {
    fn from(ServiceList(services): ServiceList) -> Self {
        if services.is_empty() {
            return FormattedString("No services found.".yellow().to_string());
        }

        let mut out = String::new();
        out.push_str("Available Services:\n");
        for svc in services {
            out.push_str(&format!("  - {}\n", svc.green()));
        }
        FormattedString(out.trim_end().to_string())
    }
}

impl From<ServiceDescriptor> for FormattedString {
    fn from(service: ServiceDescriptor) -> Self {
        let mut out = String::new();
        out.push_str(&format!(
            "{} {} {{\n",
            "service".cyan(),
            service.name().green()
        ));

        for method in service.methods() {
            out.push_str("  ");
            // Reuse the From<MethodDescriptor> implementation
            let method_fmt = FormattedString::from(method);
            out.push_str(&method_fmt.0);
            out.push_str("\n\n");
        }
        out.push('}');
        FormattedString(out)
    }
}

impl From<MethodDescriptor> for FormattedString {
    fn from(method: MethodDescriptor) -> Self {
        let input_stream = if method.is_client_streaming() {
            format!("{} ", "stream".cyan())
        } else {
            "".to_string()
        };
        let output_stream = if method.is_server_streaming() {
            format!("{} ", "stream".cyan())
        } else {
            "".to_string()
        };

        FormattedString(format!(
            "{} {}({}{}) {} ({}{});",
            "rpc".cyan(),
            method.name().green(),
            input_stream,
            method.input().full_name().yellow(),
            "returns".cyan(),
            output_stream,
            method.output().full_name().yellow()
        ))
    }
}

impl From<MessageDescriptor> for FormattedString {
    fn from(message: MessageDescriptor) -> Self {
        let mut out = String::new();
        out.push_str(&format!(
            "{} {} {{\n",
            "message".cyan(),
            message.name().green()
        ));

        for field in message.fields() {
            let label = if field.is_map() {
                "".to_string()
            } else if field.is_list() {
                format!("{} ", "repeated".cyan())
            } else {
                "".to_string()
            };

            let type_name = match field.kind() {
                Kind::Double => "double".yellow(),
                Kind::Float => "float".yellow(),
                Kind::Int32 => "int32".yellow(),
                Kind::Int64 => "int64".yellow(),
                Kind::Uint32 => "uint32".yellow(),
                Kind::Uint64 => "uint64".yellow(),
                Kind::Sint32 => "sint32".yellow(),
                Kind::Sint64 => "sint64".yellow(),
                Kind::Fixed32 => "fixed32".yellow(),
                Kind::Fixed64 => "fixed64".yellow(),
                Kind::Sfixed32 => "sfixed32".yellow(),
                Kind::Sfixed64 => "sfixed64".yellow(),
                Kind::Bool => "bool".yellow(),
                Kind::String => "string".yellow(),
                Kind::Bytes => "bytes".yellow(),
                Kind::Message(m) => m.full_name().yellow(),
                Kind::Enum(e) => e.full_name().yellow(),
            };

            if field.is_map() {
                out.push_str(&format!(
                    "  // map entry: {} {} = {};\n",
                    type_name,
                    field.name(),
                    field.number()
                ));
            } else {
                out.push_str(&format!(
                    "  {}{}{} {} = {};\n",
                    label,
                    type_name,
                    " ".normal(), // Reset color
                    field.name(),
                    field.number()
                ));
            }
        }
        out.push('}');
        FormattedString(out)
    }
}

impl From<EnumDescriptor> for FormattedString {
    fn from(enum_desc: EnumDescriptor) -> Self {
        let mut out = String::new();
        out.push_str(&format!(
            "{} {} {{\n",
            "enum".cyan(),
            enum_desc.name().green()
        ));

        for val in enum_desc.values() {
            out.push_str(&format!(
                "  {} = {};\n",
                val.name(),
                val.number().to_string().purple()
            ));
        }
        out.push('}');

        FormattedString(out)
    }
}
