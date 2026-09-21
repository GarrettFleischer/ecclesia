//! House rules the type system does not enforce by itself.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn us_clean_01_functions_do_not_take_boolean_arguments() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        visit(&root, &mut offenders);
        assert!(
            offenders.is_empty(),
            "boolean function arguments are forbidden; use a named enum or two functions:\n{}",
            offenders.join("\n")
        );
    }

    fn visit(path: &Path, offenders: &mut Vec<String>) {
        if path.is_dir() {
            for entry in fs::read_dir(path).expect("read dir") {
                visit(&entry.expect("dir entry").path(), offenders);
            }
            return;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            return;
        }
        collect_bool_args(
            path,
            &fs::read_to_string(path).expect("read rust"),
            offenders,
        );
    }

    fn collect_bool_args(path: &Path, src: &str, offenders: &mut Vec<String>) {
        for (index, line) in src.lines().enumerate() {
            if line_has_bool_argument(line) {
                offenders.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
            }
        }
    }

    fn line_has_bool_argument(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            return false;
        }
        named_bool_argument(trimmed)
    }

    fn named_bool_argument(line: &str) -> bool {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if !is_ident_start(bytes[i]) {
                i += 1;
                continue;
            }
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let rest = line[i..].trim_start();
            if rest.starts_with(':') {
                let ty = rest[1..].trim_start();
                if type_is_bool(ty) {
                    return !line[..start].contains("->");
                }
            }
        }
        false
    }

    fn type_is_bool(ty: &str) -> bool {
        ty.starts_with("bool") && !is_ident_continue(ty.as_bytes().get(4).copied().unwrap_or(0))
    }

    fn is_ident_start(b: u8) -> bool {
        b.is_ascii_alphabetic() || b == b'_'
    }

    fn is_ident_continue(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }
}
