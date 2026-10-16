use std::{fs, path::Path};

use axonyx_core::ax_formatter_prelude::format_ax_source;

fn check_templates(directory: &Path, checked: &mut usize) {
    for entry in fs::read_dir(directory).expect("template directory should be readable") {
        let path = entry.expect("template entry should be readable").path();
        if path.is_dir() {
            check_templates(&path, checked);
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.ends_with(".asx.tpl") && !name.ends_with(".ax.tpl") {
            continue;
        }
        let source = fs::read_to_string(&path)
            .expect("template should be UTF-8")
            .replace("\r\n", "\n");
        assert_eq!(
            source,
            format_ax_source(&source),
            "Unformatted Axonyx template: {}. Use the template formatting script.",
            path.display()
        );
        *checked += 1;
    }
}

#[test]
fn scaffold_sources_use_the_shared_formatter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut checked = 0;
    check_templates(&root.join("templates"), &mut checked);
    assert!(checked >= 32, "expected all four scaffold source templates");
}
