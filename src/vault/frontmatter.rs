use serde_yaml_ng::Value;

pub fn split(raw: &str) -> (Option<Value>, String) {
    let mut lines = raw.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return (None, raw.to_string());
    }
    let mut yaml = String::new();
    let mut body = String::new();
    let mut in_body = false;
    let mut closed = false;
    for line in lines {
        if !in_body && line.trim_end() == "---" {
            in_body = true;
            closed = true;
            continue;
        }
        if in_body {
            body.push_str(line);
            body.push('\n');
        } else {
            yaml.push_str(line);
            yaml.push('\n');
        }
    }
    if !closed {
        return (None, raw.to_string());
    }
    match serde_yaml_ng::from_str::<Value>(&yaml) {
        Ok(value) => (Some(value), body),
        Err(_) => (None, raw.to_string()),
    }
}

pub fn string_or_list(value: &Value, key: &str) -> Vec<String> {
    match value.get(key) {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}
