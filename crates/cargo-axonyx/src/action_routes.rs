// Shared verbatim with compiled hosts; only compiler-owned patterns belong here.
pub(crate) type Route<'a> = (&'a str, &'a [&'a str]);

pub(crate) fn resolve(routes: &[Route<'_>], path: &str) -> Option<usize> {
    let path = path.split_once('?').map_or(path, |(path, _)| path);
    let segments = parts(path);
    let mut selected = None;
    let mut selected_key = None;
    let mut ambiguous = false;
    for (index, (pattern, _)) in routes.iter().enumerate() {
        let pattern = parts(pattern);
        if pattern.len() != segments.len()
            || !pattern
                .iter()
                .zip(&segments)
                .all(|(expected, actual)| expected.starts_with(':') || expected == actual)
        {
            continue;
        }
        // Match filesystem routing: exact directory first, then dynamic siblings
        // in parameter-name order, before considering the next segment.
        let key = pattern
            .iter()
            .map(|segment| (segment.starts_with(':'), segment.trim_start_matches(':')))
            .collect::<Vec<_>>();
        match selected_key.as_ref().map(|selected| key.cmp(selected)) {
            None | Some(std::cmp::Ordering::Less) => {
                selected = Some(index);
                selected_key = Some(key);
                ambiguous = false;
            }
            Some(std::cmp::Ordering::Equal) => ambiguous = true,
            Some(std::cmp::Ordering::Greater) => {}
        }
    }
    if ambiguous {
        None
    } else {
        selected
    }
}

pub(crate) fn allows(routes: &[Route<'_>], path: &str, action: &str) -> bool {
    resolve(routes, path).is_some_and(|index| routes[index].1.contains(&action))
}

pub(crate) fn body_limit(
    routes: &[Route<'_>],
    limits: &[(&str, &str, usize)],
    method: &str,
    target: &str,
) -> Option<usize> {
    if method != "POST" {
        return None;
    }
    let selector = crate::action_selector::parse(target)?;
    let index = resolve(routes, &selector.path)?;
    if !allows(routes, &selector.path, &selector.name) {
        return None;
    }
    limits.iter().find_map(|(route, action, bytes)| {
        (*route == routes[index].0 && *action == selector.name).then_some(*bytes)
    })
}

fn parts(path: &str) -> Vec<&str> {
    path.split('/').filter(|part| !part.is_empty()).collect()
}
