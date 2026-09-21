//! House rules the type system does not enforce by itself.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn us_clean_01_functions_do_not_take_boolean_arguments() {
        let mut offenders = Vec::new();
        each_src_line(|path, index, line| {
            if line_has_bool_argument(line) {
                offenders.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
            }
        });
        assert!(
            offenders.is_empty(),
            "boolean function arguments are forbidden; use a named enum or two functions:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn us_perf_01_src_does_not_clone_collections_to_reiterate() {
        let mut offenders = Vec::new();
        each_src_line(|path, index, line| {
            if line_clones_a_collection(path, line) {
                offenders.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
            }
        });
        assert!(
            offenders.is_empty(),
            "iterate or move; do not clone a collection or rebuild a Need from a card:\n{}",
            offenders.join("\n")
        );
    }

    fn each_src_line(mut visit: impl FnMut(&Path, usize, &str)) {
        walk(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut visit,
        );
    }

    fn walk(path: &Path, visit: &mut impl FnMut(&Path, usize, &str)) {
        if path.is_dir() {
            for entry in fs::read_dir(path).expect("read dir") {
                walk(&entry.expect("dir entry").path(), visit);
            }
            return;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            return;
        }
        let src = fs::read_to_string(path).expect("read rust");
        for (index, line) in src.lines().enumerate() {
            visit(path, index, line);
        }
    }

    fn line_has_bool_argument(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            return false;
        }
        named_bool_argument(trimmed)
    }

    fn line_clones_a_collection(path: &Path, line: &str) -> bool {
        if path.ends_with("style.rs") {
            return false;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            return false;
        }
        trimmed.contains(".cloned().collect(")
            || trimmed.contains(".into_iter().cloned()")
            || trimmed.contains(".to_vec()")
            || trimmed.contains("Need::from_card")
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
