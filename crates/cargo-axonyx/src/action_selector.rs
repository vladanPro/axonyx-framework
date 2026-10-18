// Shared verbatim with compiled hosts. Keep this module independent of CLI crates.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ActionSelector {
    pub(crate) path: String,
    pub(crate) name: String,
}

pub(crate) fn parse(target: &str) -> Option<ActionSelector> {
    let (endpoint, query) = target.split_once('?')?;
    if endpoint != "/__axonyx/action" || query.contains('#') {
        return None;
    }
    let mut path = None;
    let mut name = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let decoded_key = decode(key)?;
        let value = decode(value)?;
        match decoded_key.as_str() {
            "path" if key == "path" && path.is_none() => path = Some(value),
            "name" if key == "name" && name.is_none() => name = Some(value),
            // Do not let other request parsers interpret encoded aliases or
            // duplicate selectors differently from the budget resolver.
            "path" | "name" => return None,
            _ => {}
        }
    }
    let path = path?;
    let name = name?;
    let route = path
        .split_once('?')
        .map_or(path.as_str(), |(route, _)| route);
    if !route.starts_with('/')
        || route.starts_with("//")
        || route.contains('\\')
        || route.trim() != route
        || path.contains('#')
        || path.chars().any(char::is_control)
        || route.split('/').any(|part| matches!(part, "." | ".."))
        || name.is_empty()
        || name.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return None;
    }
    Some(ActionSelector { path, name })
}

fn decode(value: &str) -> Option<String> {
    let mut decoded = Vec::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'+' => decoded.push(b' '),
            b'%' => {
                let high = char::from(bytes.next()?).to_digit(16)?;
                let low = char::from(bytes.next()?).to_digit(16)?;
                decoded.push((high * 16 + low) as u8);
            }
            byte => decoded.push(byte),
        }
    }
    String::from_utf8(decoded).ok()
}
