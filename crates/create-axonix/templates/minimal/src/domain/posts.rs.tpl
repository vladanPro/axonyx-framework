pub fn validate_post_title(title: &str) -> Result<(), &'static str> {
    if title.trim().is_empty() {
        return Err("title cannot be empty");
    }

    if title.len() > 120 {
        return Err("title is too long");
    }

    Ok(())
}
