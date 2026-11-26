use crate::config::{ConfigError, GlabConfig, HostAuth};

pub fn parse(text: &str) -> Result<GlabConfig, ConfigError> {
    let mut default_host = None;
    let mut hosts = Vec::new();
    let mut in_hosts = false;
    let mut current: Option<HostAuth> = None;

    for raw in text.lines() {
        let indent = raw.chars().take_while(|c| *c == ' ').count();
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !in_hosts {
            if let Some(value) = key_value(line, "host") {
                if !value.is_empty() {
                    default_host = Some(value);
                }
            }
            if line == "hosts:" {
                in_hosts = true;
            }
            continue;
        }
        if indent == 0 && line != "hosts:" && !line.starts_with('#') {
            flush_host(&mut hosts, current.take());
            break;
        }
        if indent <= 4 && line.ends_with(':') && !line.contains(' ') {
            flush_host(&mut hosts, current.take());
            let hostname = line.trim_end_matches(':').to_string();
            current = Some(HostAuth {
                api_host: hostname.clone(),
                hostname,
                api_protocol: "https".into(),
                token: String::new(),
                user: None,
            });
            continue;
        }
        if let Some(host) = current.as_mut() {
            apply_host_field(host, line);
        }
    }
    flush_host(&mut hosts, current.take());
    if hosts.is_empty() {
        return Err(ConfigError::NoHosts);
    }
    Ok(GlabConfig {
        default_host,
        hosts,
    })
}

fn flush_host(hosts: &mut Vec<HostAuth>, host: Option<HostAuth>) {
    if let Some(host) = host {
        if !host.token.is_empty() {
            hosts.push(host);
        }
    }
}

fn apply_host_field(host: &mut HostAuth, line: &str) {
    if let Some(value) = key_value(line, "token") {
        host.token = value;
    } else if let Some(value) = key_value(line, "api_host") {
        if !value.is_empty() {
            host.api_host = value;
        }
    } else if let Some(value) = key_value(line, "api_protocol") {
        if !value.is_empty() {
            host.api_protocol = value;
        }
    } else if let Some(value) = key_value(line, "user") {
        if !value.is_empty() && value != "null" {
            host.user = Some(value);
        }
    }
}

fn key_value(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    let rest = line.strip_prefix(&prefix)?.trim();
    Some(unquote(rest))
}

fn unquote(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    if let Some(rest) = trimmed.strip_prefix("!!null ") {
        return rest.trim().to_string();
    }
    if trimmed == "!!null" {
        return String::new();
    }
    trimmed.to_string()
}
